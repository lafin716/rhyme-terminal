# Scope the command gate to this PTY. User argv is data, never shell source.
function global:Invoke-RhymeLoopAgent {
    param([string]$Provider, [object[]]$CommandArgs)
    $json = if ($CommandArgs.Count -eq 0) { '[]' } else { ConvertTo-Json -InputObject @($CommandArgs) -Compress }
    $encoded = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($json))
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = $env:RHYME_LOOP_BRIDGE
    $info.WorkingDirectory = $ExecutionContext.SessionState.Path.CurrentFileSystemLocation.ProviderPath
    $info.Arguments = '--rhyme-loop-agent ' + $Provider + ' --encoded-args ' + $encoded
    $info.UseShellExecute = $false
    # GUI-subsystem release helpers must be explicitly awaited. No redirected
    # text pipeline: the Agent inherits the original interactive terminal.
    $process = [Diagnostics.Process]::Start($info)
    try {
        $process.WaitForExit()
        $global:LASTEXITCODE = $process.ExitCode
    } finally { $process.Dispose() }
}
function global:codex { Invoke-RhymeLoopAgent -Provider 'codex' -CommandArgs $args }
function global:claude { Invoke-RhymeLoopAgent -Provider 'claude' -CommandArgs $args }
