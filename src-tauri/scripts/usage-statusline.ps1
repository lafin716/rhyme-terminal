# Installed by Winmux in one isolated Claude profile. Preserve stdout for Claude.
$ErrorActionPreference = 'Stop'
$OutputEncoding = New-Object Text.UTF8Encoding($false)
[Console]::OutputEncoding = $OutputEncoding
[Console]::InputEncoding = $OutputEncoding
$inputJson = [Console]::In.ReadToEnd()
try {
    $payload = $inputJson | ConvertFrom-Json
    $windows = @{}
    foreach ($name in @('five_hour', 'seven_day')) {
        $window = $payload.rate_limits.$name
        if ($null -ne $window -and $null -ne $window.used_percentage) {
            $percent = 0.0
            if ([double]::TryParse([string]$window.used_percentage, [Globalization.NumberStyles]::Float, [Globalization.CultureInfo]::InvariantCulture, [ref]$percent) -and -not [double]::IsNaN($percent) -and -not [double]::IsInfinity($percent)) {
                $windows[$name] = @{ used_percentage = $percent; resets_at = $window.resets_at }
            }
        }
    }
    $matchesProfile = $true
    $tokenFile = Join-Path $PSScriptRoot 'oauth-token.txt'
    if ($env:CLAUDE_CODE_OAUTH_TOKEN -and [IO.File]::Exists($tokenFile)) {
        $matchesProfile = [string]::Equals($env:CLAUDE_CODE_OAUTH_TOKEN.Trim(), [IO.File]::ReadAllText($tokenFile).Trim(), [StringComparison]::Ordinal)
    }
    if ($windows.Count -gt 0 -and $matchesProfile) {
        $sample = @{ version = 1; receivedAt = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds(); rate_limits = $windows }
        $cache = Join-Path $PSScriptRoot 'winmux-usage.json'
        $temporary = Join-Path $PSScriptRoot ('winmux-usage-' + [guid]::NewGuid().ToString('N') + '.tmp')
        try {
            [IO.File]::WriteAllText($temporary, ($sample | ConvertTo-Json -Depth 5 -Compress), (New-Object Text.UTF8Encoding($false)))
            if ([IO.File]::Exists($cache)) { [IO.File]::Replace($temporary, $cache, [NullString]::Value) }
            else { [IO.File]::Move($temporary, $cache) }
        } finally { if ([IO.File]::Exists($temporary)) { [IO.File]::Delete($temporary) } }
    }
} catch { } # Collection must never disrupt the user's statusline.
try {
    $bridge = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot 'winmux-statusline.json') | ConvertFrom-Json
    $original = $bridge.original.command
    if ($original) {
        $bash = $env:CLAUDE_CODE_GIT_BASH_PATH
        if (-not $bash) {
            $git = (Get-Command git.exe -ErrorAction SilentlyContinue).Source
            if ($git) {
                $candidate = Join-Path (Split-Path (Split-Path $git -Parent) -Parent) 'bin/bash.exe'
                if (Test-Path -LiteralPath $candidate) { $bash = $candidate }
            }
        }
        if ($bash) { $inputJson | & $bash -c $original }
        else { $inputJson | & $env:ComSpec /d /s /c $original }
    }
} catch { }
