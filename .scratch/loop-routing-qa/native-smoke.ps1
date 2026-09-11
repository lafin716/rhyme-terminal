param(
  [Parameter(Mandatory=$true)][string]$Executable,
  [Parameter(Mandatory=$true)][string]$ControlExecutable,
  [Parameter(Mandatory=$true)][ValidatePattern('^com\.user\.winmux\.loop-smoke-[a-zA-Z0-9-]+$')][string]$SmokeIdentifier,
  [Parameter(Mandatory=$true)][string]$SmokeConfig
)
$ErrorActionPreference='Stop'
$nativeNonce=[Guid]::NewGuid().ToString('N').Substring(0,12)
$nativeRoot=Join-Path 'C:\Users\user\orca\winmux\.scratch\loop-routing-qa' ('native-'+$nativeNonce)
$nativeUser='loop_smoke_'+$nativeNonce
$config=Get-Content -LiteralPath $SmokeConfig -Raw | ConvertFrom-Json
if($config.identifier -ne $SmokeIdentifier){throw 'Smoke identifier mismatch; refusing production app launch.'}
$nativeExe=(Resolve-Path -LiteralPath $Executable).Path
$nativeCtl=(Resolve-Path -LiteralPath $ControlExecutable).Path
$nativeKnownData=Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) $SmokeIdentifier
if(Test-Path -LiteralPath $nativeKnownData){throw 'Smoke identifier already has data; choose a fresh identifier.'}
$nativeDirs=@{}
foreach($name in @('local','roaming','profile','webview','claude','codex','temp','cwd')){
  $nativeDirs[$name]=Join-Path $nativeRoot $name
  New-Item -ItemType Directory -Path $nativeDirs[$name] -Force | Out-Null
}
$report=[ordered]@{identifier=$SmokeIdentifier;username=$nativeUser;root=$nativeRoot;appData=$nativeKnownData;executable=$nativeExe;sha256=(Get-FileHash -LiteralPath $nativeExe).Hash;windows=@();alive=$false;startedAt=(Get-Date).ToString('o')}
function New-IsolatedInfo([string]$File,[string]$Arguments){
  $info=New-Object Diagnostics.ProcessStartInfo
  $info.FileName=$File;$info.Arguments=$Arguments;$info.WorkingDirectory=$nativeDirs.cwd
  $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.WindowStyle=[Diagnostics.ProcessWindowStyle]::Hidden
  $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
  foreach($key in @($info.EnvironmentVariables.Keys)){
    if($key -match '(?i)(TOKEN|API_KEY|SECRET|PASSWORD|CREDENTIAL|AUTHORIZATION|SSH_AUTH_SOCK)'){$info.EnvironmentVariables.Remove($key)}
  }
  $environment=@{USERNAME=$nativeUser;LOCALAPPDATA=$nativeDirs.local;APPDATA=$nativeDirs.roaming;USERPROFILE=$nativeDirs.profile;HOME=$nativeDirs.profile;HOMEDRIVE=[IO.Path]::GetPathRoot($nativeDirs.profile).TrimEnd('\');HOMEPATH=$nativeDirs.profile.Substring(2);WEBVIEW2_USER_DATA_FOLDER=$nativeDirs.webview;CLAUDE_CONFIG_DIR=$nativeDirs.claude;CODEX_HOME=$nativeDirs.codex;TEMP=$nativeDirs.temp;TMP=$nativeDirs.temp}
  foreach($key in $environment.Keys){$info.EnvironmentVariables[$key]=$environment[$key]}
  return $info
}
function Invoke-IsolatedControl([string]$Arguments){
  $child=New-Object Diagnostics.Process
  $child.StartInfo=New-IsolatedInfo $nativeCtl $Arguments
  [void]$child.Start()
  $outputTask=$child.StandardOutput.ReadToEndAsync();$errorTask=$child.StandardError.ReadToEndAsync()
  if(!$child.WaitForExit(15000)){$child.Kill();throw 'Isolated control timed out.'}
  $result=@{exitCode=$child.ExitCode;output=$(if($outputTask.Wait(1000)){$outputTask.Result}else{'stdout pipe remains open'});error=$(if($errorTask.Wait(1000)){$errorTask.Result}else{'stderr pipe remains open'})}
  $child.Dispose();return $result
}
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class NativeLoopSmoke {
  public delegate bool Callback(IntPtr window,IntPtr param);
  [DllImport("user32.dll")] public static extern bool EnumWindows(Callback callback,IntPtr param);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window,out uint pid);
  [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr window,StringBuilder text,int max);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window,out RECT rect);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr window,IntPtr dc,uint flags);
  [StructLayout(LayoutKind.Sequential)] public struct RECT {public int Left,Top,Right,Bottom;}
  public static IntPtr[] WindowsFor(int pid){var list=new List<IntPtr>();EnumWindows((window,param)=>{uint owner;GetWindowThreadProcessId(window,out owner);if(owner==pid)list.Add(window);return true;},IntPtr.Zero);return list.ToArray();}
  public static string Title(IntPtr window){var text=new StringBuilder(512);GetWindowText(window,text,text.Capacity);return text.ToString();}
}
'@
$baselineProcesses=@(Get-Process -Name 'rhyme-terminal','winmuxd','winmux' -ErrorAction SilentlyContinue|Select-Object Id,ProcessName)
$report.baselineProcesses=$baselineProcesses
$app=$null
try{
  # Seed cmd /D so a fresh UI cannot launch the user's PowerShell profile.
  $seed=Invoke-IsolatedControl ('new --name native-smoke --workspace 1 --shell cmd.exe --shell-arg /D --cwd "'+$nativeDirs.cwd+'"')
  $report.seedExitCode=$seed.exitCode
  if($seed.exitCode -ne 0){throw ('Isolated seed failed: '+$seed.error)}
  $app=New-Object Diagnostics.Process
  $app.StartInfo=New-IsolatedInfo $nativeExe ''
  [void]$app.Start();$report.appPid=$app.Id
  $appOutput=$app.StandardOutput.ReadToEndAsync();$appError=$app.StandardError.ReadToEndAsync()
  $handles=@()
  for($attempt=0;$attempt -lt 60;$attempt++){
    if($app.HasExited){throw ('GUI exited: '+$app.ExitCode)}
    $handles=@([NativeLoopSmoke]::WindowsFor($app.Id)|Where-Object{[NativeLoopSmoke]::Title($_)-like '*rhyme*'})
    if($handles.Count){break}
    Start-Sleep -Milliseconds 250
  }
  Start-Sleep -Seconds 3
  $report.alive=!$app.HasExited
  foreach($handle in $handles){
    $rect=New-Object NativeLoopSmoke+RECT
    [void][NativeLoopSmoke]::GetWindowRect($handle,[ref]$rect)
    $report.windows+=@{handle=$handle.ToInt64();title=[NativeLoopSmoke]::Title($handle);visible=[NativeLoopSmoke]::IsWindowVisible($handle);width=$rect.Right-$rect.Left;height=$rect.Bottom-$rect.Top}
  }
  if(!$handles.Count){throw 'GUI alive but no titled main window found.'}
  $main=$handles[0]
  try{
    $element=[Windows.Automation.AutomationElement]::FromHandle($main)
    $children=$element.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
    $report.uiNames=@($children|ForEach-Object{$_.Current.Name}|Where-Object{$_}|Select-Object -First 100)
    $settings=@($children|Where-Object{$_.Current.Name -in @('Settings','설정') -and $_.Current.ControlType -eq [Windows.Automation.ControlType]::Button})|Select-Object -First 1
    if($settings){
      ([Windows.Automation.InvokePattern]$settings.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)).Invoke()
      Start-Sleep -Milliseconds 600
      $children=$element.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
      $loop=@($children|Where-Object{$_.Current.Name -like '*에이전트 루프*' -and $_.Current.ControlType -eq [Windows.Automation.ControlType]::Button})|Select-Object -First 1
      if($loop){
        ([Windows.Automation.InvokePattern]$loop.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)).Invoke()
        Start-Sleep -Milliseconds 600
        $report.loopSettingsOpened=$true
        $children=$element.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
        $report.loopUiNames=@($children|ForEach-Object{$_.Current.Name}|Where-Object{$_}|Select-Object -First 100)
      }
    }
  }catch{$report.uiError=$_.Exception.Message}
  $rect=New-Object NativeLoopSmoke+RECT
  [void][NativeLoopSmoke]::GetWindowRect($main,[ref]$rect)
  $bitmap=New-Object Drawing.Bitmap([Math]::Max(1,$rect.Right-$rect.Left),[Math]::Max(1,$rect.Bottom-$rect.Top))
  $graphics=[Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
  try{$report.printWindow=[NativeLoopSmoke]::PrintWindow($main,$dc,2)}finally{$graphics.ReleaseHdc($dc);$graphics.Dispose()}
  $report.screenshot=Join-Path $nativeRoot 'native-window.png'
  $bitmap.Save($report.screenshot,[Drawing.Imaging.ImageFormat]::Png);$bitmap.Dispose()
  $report.isolatedSessions=(Invoke-IsolatedControl '--json ls').output|ConvertFrom-Json
}catch{$report.error=$_.Exception.Message}
finally{
  try{$report.cleanup=Invoke-IsolatedControl 'kill-server'}catch{$report.cleanupError=$_.Exception.Message}
  if($app){
    if(!$app.HasExited){$app.Kill();[void]$app.WaitForExit(5000)}
    $report.appStopped=$app.HasExited
    if($appError -and $appError.Wait(1000)){$report.appError=$appError.Result}
    $app.Dispose()
  }
  $report.baselineProcessesStillAlive=@($baselineProcesses|Where-Object {Get-Process -Id $_.Id -ErrorAction SilentlyContinue}|Select-Object Id,ProcessName)
  $report.finishedAt=(Get-Date).ToString('o')
  $reportPath=Join-Path $nativeRoot 'report.json'
  $report|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $reportPath -Encoding UTF8
  Write-Output ('REPORT='+$reportPath)
  $report|ConvertTo-Json -Depth 8
}
if($report.error){exit 1}
