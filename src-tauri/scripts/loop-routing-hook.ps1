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
    $allowed = @('SessionStart','SessionEnd','UserPromptSubmit','PreToolUse','PostToolUse','PostToolUseFailure','PermissionRequest','Stop','SubagentStart','SubagentStop','Interrupt')
    if ($kind -notin $allowed) { throw 'Unsupported lifecycle event' }
    $sessionId = [string]$inputEvent.session_id
    $parsedId = [guid]::Empty
    if (-not [guid]::TryParse($sessionId, [ref]$parsedId)) { throw 'Invalid session identity' }
    # Rust canonical paths use the Windows verbatim prefix. PowerShell 5.1's
    # filesystem provider rejects it; use .NET for every path and file operation.
    $controlPath = [IO.Path]::Combine($root, 'control.json')
    $switchRequested = $false
    if ([IO.File]::Exists($controlPath)) {
        $switchRequested = ([IO.File]::ReadAllText($controlPath) | ConvertFrom-Json).switchRequested -eq $true
    }
    $boundary = $switchRequested -and ($kind -in @('PreToolUse','PostToolUse','PostToolUseFailure','Stop'))
    $toolInput = $inputEvent.tool_input
    if ($toolInput -is [string]) {
        try { $toolInput = $toolInput | ConvertFrom-Json } catch { $toolInput = $null }
    }
    $toolResponse = $inputEvent.tool_response
    $unknownBackground = ($toolInput.run_in_background -eq $true) -or ($toolResponse.backgrounded -eq $true) -or
        ($null -ne $toolResponse.backgroundTaskId) -or ($null -ne $toolResponse.background_task_id)
    if ($kind -eq 'PostToolUse' -and ([string]$inputEvent.tool_name -in @('Bash','Shell','exec_command','write_stdin','shell_command'))) {
        $responseText = if ($toolResponse -is [string]) { $toolResponse } else { $toolResponse | ConvertTo-Json -Depth 8 -Compress }
        $unknownBackground = $unknownBackground -or ($null -ne $toolResponse.session_id) -or
            ($responseText -match '(?i)process running with session id|script running with cell id|running in (the )?background')
    }
    $event = [ordered]@{
        kind = $kind
        sessionId = $sessionId
        transcriptPath = $inputEvent.transcript_path
        toolUseId = $inputEvent.tool_use_id
        subagentId = $inputEvent.agent_id
        atMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
        boundary = $boundary
        unknownBackground = [bool]$unknownBackground
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
                $event.kind = 'BoundaryExpired'
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
    if ($boundary) {
        # PostToolUse continue:false does NOT stop Codex. Hold this synchronous
        # hook until the controller terminates the old CLI and observes exit.
        # No timer releases the gate. Runtime must act well before hook timeout.
        $waitStarted = [DateTimeOffset]::UtcNow
        $expiryReported = $false
        $resumed = $false
        while ($true) {
            Start-Sleep -Milliseconds 100
            if ([IO.File]::Exists([IO.Path]::Combine($root, 'release.json'))) { break }
            try {
                if (([IO.File]::ReadAllText($controlPath) | ConvertFrom-Json).switchRequested -eq $false) {
                    $resumed = $true
                    break
                }
            } catch { } # A transient/partial control write never opens the gate.
            if (-not $expiryReported -and ([DateTimeOffset]::UtcNow - $waitStarted).TotalSeconds -ge 30) {
                $expired = [ordered]@{
                    kind = 'BoundaryExpired'
                    sessionId = $sessionId
                    transcriptPath = $inputEvent.transcript_path
                    toolUseId = $inputEvent.tool_use_id
                    subagentId = $inputEvent.agent_id
                    atMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
                    boundary = $false
                    unknownBackground = [bool]$unknownBackground
                }
                $expiryName = '{0:D20}-{1}' -f [DateTime]::UtcNow.Ticks, [guid]::NewGuid().ToString('N')
                $expiryTemporary = [IO.Path]::Combine($events, ($expiryName + '.tmp'))
                [IO.File]::WriteAllText($expiryTemporary, ($expired | ConvertTo-Json -Compress), $utf8)
                [IO.File]::Move($expiryTemporary, ([IO.Path]::Combine($events, ($expiryName + '.json'))))
                $expiryReported = $true
            }
        }
        if ($resumed) { '{}' }
        elseif ($kind -eq 'PreToolUse') {
            '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Rhyme Loop switching accounts"}}'
        } else {
            '{"continue":false,"stopReason":"Rhyme Loop switching accounts"}'
        }
    } else { '{}' }
} catch {
    # Errors cannot leak stdin. The runtime detects missing lifecycle evidence.
    [Console]::Error.WriteLine('Rhyme Loop lifecycle bridge failed')
    exit 2
}
