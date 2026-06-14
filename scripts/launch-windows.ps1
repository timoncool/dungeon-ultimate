param(
  [switch]$CpuOnly,
  [switch]$SetupImages,
  [switch]$SetupOllama
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Repo = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $Repo

$AppUrl = "http://localhost:3000"
$HealthUrl = "http://127.0.0.1:3000/api/health"
$NonInteractive = $env:CI -eq "true"

function Write-Step($Message) {
  Write-Host ""
  Write-Host "==> $Message" -ForegroundColor Cyan
}

function Test-Http($Url) {
  try {
    Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 $Url | Out-Null
    return $true
  } catch {
    return $false
  }
}

function Test-PortListener($Port) {
  if (-not (Get-Command "Get-NetTCPConnection" -ErrorAction SilentlyContinue)) {
    return $false
  }

  $connections = @(
    Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
  )
  return $connections.Count -gt 0
}

function Open-App {
  Start-Process $AppUrl
}

function Invoke-Setup {
  $setupArgs = @()
  if ($CpuOnly) {
    $setupArgs += "-CpuOnly"
  }
  if ($SetupImages) {
    $setupArgs += "-SetupImages"
  }
  if ($SetupOllama) {
    $setupArgs += "-SetupOllama"
  }

  & (Join-Path $Repo "scripts\setup-windows.ps1") @setupArgs
  if (-not $?) {
    exit 1
  }
  $lastExitCode = Get-Variable -Name LASTEXITCODE -ValueOnly -ErrorAction SilentlyContinue
  if ($null -eq $lastExitCode) {
    exit 0
  }
  exit $lastExitCode
}

function Invoke-Stop {
  & (Join-Path $Repo "scripts\stop-windows.ps1")
  if (-not $?) {
    exit 1
  }
}

Write-Step "Open Dungeon Windows launcher"

$AppResponding = Test-Http $HealthUrl
$PortListening = Test-PortListener 3000

if ($AppResponding -or $PortListening) {
  if ($AppResponding) {
    Write-Host "Open Dungeon is already running at $AppUrl." -ForegroundColor Green
  } else {
    Write-Host "Something is already listening on port 3000." -ForegroundColor Yellow
  }

  if ($NonInteractive) {
    Open-App
    exit 0
  }

  Write-Host ""
  Write-Host "Press Enter or type O to open it in your browser."
  Write-Host "Type K to stop Open Dungeon."
  Write-Host "Type R to restart it."
  $choice = (Read-Host "Selection").Trim().ToUpperInvariant()

  if ($choice -eq "K" -or $choice -eq "S" -or $choice -eq "STOP") {
    Invoke-Stop
    Write-Host ""
    Write-Host "Open Dungeon stopped." -ForegroundColor Green
    Read-Host "Press Enter to close"
    exit 0
  }

  if ($choice -eq "R" -or $choice -eq "RESTART") {
    Invoke-Stop
    Invoke-Setup
  }

  Open-App
  exit 0
}

Invoke-Setup
