param(
  [ValidateSet("auto", "cuda", "cpu", "both")]
  [string]$Device = "both",
  [int]$Count = 1,
  [string]$Prompt = "a small glowing dungeon lantern, dark fantasy item icon",
  [int]$TimeoutSeconds = 1200,
  [int]$PauseSeconds = 3,
  [switch]$KeepImageServer,
  [switch]$DiagnoseOnSuccess
)

$ErrorActionPreference = "Stop"
$Repo = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $Repo
$LogDir = Join-Path $Repo "logs"
New-Item -ItemType Directory -Path $LogDir -Force | Out-Null
$RunStamp = Get-Date -Format "yyyyMMdd-HHmmss"
$LoopLog = Join-Path $LogDir ("windows-image-loop-{0}.txt" -f $RunStamp)

function Write-LoopLine($Message = "") {
  $text = [string]$Message
  Write-Host $text
  Add-Content -LiteralPath $LoopLog -Value $text -Encoding UTF8
}

function Write-LoopStep($Message) {
  Write-LoopLine ""
  Write-LoopLine ("==> {0}" -f $Message)
}

function Test-LoopCommand($Name) {
  return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Stop-ImageWorker {
  if (-not (Test-LoopCommand "Get-NetTCPConnection")) {
    Write-LoopLine "Get-NetTCPConnection is unavailable; skipping image worker port cleanup."
    return
  }

  $connections = @(
    Get-NetTCPConnection -LocalPort 7869 -State Listen -ErrorAction SilentlyContinue
  )
  $processIds = @(
    $connections |
      Select-Object -ExpandProperty OwningProcess -Unique |
      Where-Object { $_ -and $_ -gt 0 }
  )

  if (-not $processIds.Count) {
    Write-LoopLine "No existing image worker is listening on port 7869."
    return
  }

  foreach ($processId in $processIds) {
    $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
    if (-not $process) {
      continue
    }
    Write-LoopLine ("Stopping image worker before next attempt: {0} ({1})" -f $process.ProcessName, $processId)
    Stop-Process -Id $processId -Force
  }
}

function Invoke-LoopChildPowerShell([string[]]$CommandArgs) {
  if (-not (Test-LoopCommand "powershell")) {
    throw "Windows PowerShell was not found."
  }

  & powershell @CommandArgs 2>&1 | ForEach-Object { Write-LoopLine $_ }
  $exitCode = $LASTEXITCODE
  return $exitCode
}

function Invoke-Diagnostics($Reason) {
  Write-LoopStep ("Collecting diagnostics: {0}" -f $Reason)
  $diagnostics = Join-Path $Repo "scripts\diagnose-windows.ps1"
  $diagnosticArgs = @(
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    $diagnostics,
    "-IncludeLogTails"
  )
  $diagnosticExit = Invoke-LoopChildPowerShell $diagnosticArgs
  Write-LoopLine ("Diagnostics exit code: {0}" -f $diagnosticExit)
}

function Get-DevicesToRun {
  if ($Device -eq "both") {
    if (Test-LoopCommand "nvidia-smi") {
      return @("cpu", "cuda")
    }

    Write-LoopLine "No nvidia-smi command was found; running the CPU proof only."
    return @("cpu")
  }

  return @($Device)
}

if ($Count -lt 1) {
  throw "Count must be at least 1."
}
if ($TimeoutSeconds -lt 60) {
  throw "TimeoutSeconds must be at least 60."
}
if ($PauseSeconds -lt 0) {
  throw "PauseSeconds cannot be negative."
}

$devicesToRun = @(Get-DevicesToRun)
$totalAttempts = $devicesToRun.Count * $Count
$failureCount = 0
$successCount = 0

Write-LoopLine "Open Dungeon Windows image loop"
Write-LoopLine ("timestamp={0}" -f (Get-Date -Format o))
Write-LoopLine ("repo={0}" -f $Repo)
Write-LoopLine ("log={0}" -f $LoopLog)
Write-LoopLine ("device={0}" -f $Device)
Write-LoopLine ("devices_to_run={0}" -f ($devicesToRun -join ","))
Write-LoopLine ("count={0}" -f $Count)
Write-LoopLine ("timeout_seconds={0}" -f $TimeoutSeconds)

for ($iteration = 1; $iteration -le $Count; $iteration++) {
  foreach ($attemptDevice in $devicesToRun) {
    $attemptLabel = "{0}/{1} {2}" -f $iteration, $Count, $attemptDevice
    $smokeLog = Join-Path $LogDir ("windows-image-loop-smoke-{0}-{1}-{2}.log" -f $RunStamp, $iteration, $attemptDevice)

    Write-LoopStep ("Image smoke attempt {0}" -f $attemptLabel)
    Write-LoopLine ("smoke_log={0}" -f $smokeLog)

    if (-not $KeepImageServer) {
      Stop-ImageWorker
      if ($PauseSeconds -gt 0) {
        Start-Sleep -Seconds $PauseSeconds
      }
    }

    $smoke = Join-Path $Repo "scripts\smoke-windows-image.ps1"
    $smokeArgs = @(
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      $smoke,
      "-Prompt",
      $Prompt,
      "-TimeoutSeconds",
      ([string]$TimeoutSeconds),
      "-LogPath",
      $smokeLog
    )

    if ($attemptDevice -eq "cpu") {
      $smokeArgs += "-CpuOnly"
    } elseif ($attemptDevice -eq "cuda") {
      $smokeArgs += @("-Device", "cuda")
    }

    $exitCode = 1
    try {
      $exitCode = Invoke-LoopChildPowerShell $smokeArgs
    } catch {
      Write-LoopLine ("smoke_failed_to_run={0}" -f $_.Exception.Message)
      $exitCode = 1
    }

    if ($exitCode -eq 0) {
      $successCount += 1
      Write-LoopLine ("PASS {0}" -f $attemptLabel)
      if ($DiagnoseOnSuccess) {
        Invoke-Diagnostics ("success after {0}" -f $attemptLabel)
      }
    } else {
      $failureCount += 1
      Write-LoopLine ("FAIL {0} exit_code={1}" -f $attemptLabel, $exitCode)
      Invoke-Diagnostics ("failure after {0}" -f $attemptLabel)
    }
  }
}

Write-LoopStep "Image loop summary"
Write-LoopLine ("attempts={0}" -f $totalAttempts)
Write-LoopLine ("passed={0}" -f $successCount)
Write-LoopLine ("failed={0}" -f $failureCount)
Write-LoopLine ("loop_log={0}" -f $LoopLog)

if ($failureCount -gt 0) {
  exit 1
}

exit 0
