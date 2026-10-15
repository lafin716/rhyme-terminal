param()
# Per-invocation bridge. Never write raw hook input, prompts, or tool output.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$utf8 = New-Object System.Text.UTF8Encoding($false)
try {
    $root = $env:RHYME_LOOP_ATTEMPT_DIR
    if ([string]::IsNullOrWhiteSpace($root)) { throw 'Missing attempt directory' }
    $inputEvent = [Console]::In.ReadToEnd() | ConvertFrom-Json
    $kind = [string]$inputEvent.hook_event_name
    $allowed = @('SessionStart','SessionEnd','UserPromptSubmit','PreToolUse','PostToolUse','PostToolUseFailure','PermissionRequest','Stop','StopFailure','SubagentStart','SubagentStop','Interrupt')
    if ($kind -notin $allowed) { throw 'Unsupported lifecycle event' }
    $sessionId = [string]$inputEvent.session_id
    $parsedId = [guid]::Empty
    if (-not [guid]::TryParse($sessionId, [ref]$parsedId)) { throw 'Invalid session identity' }
    # Rust canonical paths use the Windows verbatim prefix. PowerShell 5.1's
    # filesystem provider rejects it; use .NET for every path and file operation.
    $errorCode = $inputEvent.error.code
    if ($kind -eq 'StopFailure') {
        if ($inputEvent.error -is [string]) { $errorCode = [string]$inputEvent.error }
        $diagnostic = ([string]$inputEvent.error_details) + ' ' + ([string]$inputEvent.last_assistant_message)
        if ($errorCode -eq 'billing_error' -or (($errorCode -in @('rate_limit','unknown')) -and $diagnostic -match '(?i)hit your limit|usage limit|usage has been exhausted|insufficient_quota|credit balance is too low|quota exceeded')) {
            $errorCode = 'usage_limit_reached'
        }
    }
    $event = [ordered]@{
        kind = $kind
        errorCode = $errorCode
        sessionId = $sessionId
        transcriptPath = $inputEvent.transcript_path
        toolUseId = $inputEvent.tool_use_id
        subagentId = $inputEvent.agent_id
        atMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    }
    $events = [IO.Path]::Combine($root, 'events')
    [IO.Directory]::CreateDirectory($events) | Out-Null
    $name = '{0:D20}-{1}' -f [DateTime]::UtcNow.Ticks, [guid]::NewGuid().ToString('N')
    $pending = [IO.Path]::Combine($events, ($name + '.tmp'))
    $ready = [IO.Path]::Combine($events, ($name + '.json'))
    [IO.File]::WriteAllText($pending, ($event | ConvertTo-Json -Compress), $utf8)
    [IO.File]::Move($pending, $ready)
    if ($kind -eq 'SessionStart') {
        # Never let a failed native resume silently turn into a fresh prompted run.
        $started = [DateTimeOffset]::UtcNow
        $reported = $false
        while ($true) {
            try {
                $approval = [IO.File]::ReadAllText([IO.Path]::Combine($root, 'start-approved.json')) | ConvertFrom-Json
                if ([string]$approval.sessionId -eq $sessionId) { break }
            } catch { }
            if (-not $reported -and ([DateTimeOffset]::UtcNow - $started).TotalSeconds -ge 60) {
                $event.kind = 'StartupTimeout'
                $event.atMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
                $expiryName = '{0:D20}-{1}' -f [DateTime]::UtcNow.Ticks, [guid]::NewGuid().ToString('N')
                $expiryTemporary = [IO.Path]::Combine($events, ($expiryName + '.tmp'))
                [IO.File]::WriteAllText($expiryTemporary, ($event | ConvertTo-Json -Compress), $utf8)
                [IO.File]::Move($expiryTemporary, ([IO.Path]::Combine($events, ($expiryName + '.json'))))
                $reported = $true
            }
            Start-Sleep -Milliseconds 100
        }
        $startupReady = [IO.Path]::Combine($root, 'startup-ready.json')
        $startupTemporary = [IO.Path]::Combine($root, ('startup-ready-' + [guid]::NewGuid().ToString('N') + '.tmp'))
        [IO.File]::WriteAllText($startupTemporary, (@{ sessionId = $sessionId } | ConvertTo-Json -Compress), $utf8)
        if ([IO.File]::Exists($startupReady)) {
            [IO.File]::Replace($startupTemporary, $startupReady, $null)
        } else { [IO.File]::Move($startupTemporary, $startupReady) }
    }
    '{}'
} catch {
    # Errors cannot leak stdin. The runtime detects missing lifecycle evidence.
    [Console]::Error.WriteLine('Rhyme Loop lifecycle bridge failed')
    exit 2
}
