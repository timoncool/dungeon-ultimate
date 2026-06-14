param(
  [switch]$CpuOnly,
  [ValidateSet("auto", "cuda", "cpu")]
  [string]$Device = "auto",
  [string]$Prompt = "a small glowing dungeon lantern, dark fantasy item icon",
  [int]$Port = 7869,
  [int]$TimeoutSeconds = 1200,
  [string]$LogPath = ""
)

$ErrorActionPreference = "Stop"
$Repo = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $Repo
$LogDir = Join-Path $Repo "logs"
if (-not $LogPath) {
  $LogPath = Join-Path $LogDir ("windows-image-smoke-{0}.log" -f (Get-Date -Format "yyyyMMdd-HHmmss"))
}
$TranscriptStarted = $false

function Write-Step($Message) {
  Write-Host ""
  Write-Host "==> $Message" -ForegroundColor Cyan
}

function Wait-Json($Url, $Seconds) {
  $deadline = (Get-Date).AddSeconds($Seconds)
  while ((Get-Date) -lt $deadline) {
    try {
      return Invoke-RestMethod -UseBasicParsing -TimeoutSec 2 -Uri $Url
    } catch {
      Start-Sleep -Seconds 1
    }
  }
  return $null
}

function Invoke-JsonPost($Url, $Payload, $TimeoutSec) {
  $body = $Payload | ConvertTo-Json -Compress
  try {
    return Invoke-RestMethod -UseBasicParsing -Method Post -Uri $Url -ContentType "application/json" -Body $body -TimeoutSec $TimeoutSec
  } catch {
    Write-Host "Image smoke request failed." -ForegroundColor Red
    if ($_.Exception.Response) {
      try {
        $stream = $_.Exception.Response.GetResponseStream()
        $reader = New-Object System.IO.StreamReader($stream)
        Write-Host $reader.ReadToEnd()
      } catch {
        Write-Host $_.Exception.Message
      }
    }
    throw
  }
}

try {
  New-Item -ItemType Directory -Path $LogDir -Force | Out-Null
  try {
    Start-Transcript -Path $LogPath -Force | Out-Null
    $TranscriptStarted = $true
  } catch {
    Write-Host "Could not start transcript log: $($_.Exception.Message)" -ForegroundColor Yellow
  }

  Write-Step "Preparing Windows image worker"
  Write-Host "Log: $LogPath"

  if ($CpuOnly) {
    $env:IMAGE_SERVER_DEVICE = "cpu"
  } elseif ($Device -ne "auto") {
    $env:IMAGE_SERVER_DEVICE = $Device
  }

  $setup = Join-Path $Repo "scripts\setup-windows.ps1"
  $setupArgs = @("-ImageOnly")
  if ($CpuOnly) {
    $setupArgs += "-CpuOnly"
  }
  & $setup @setupArgs
  if (-not $?) {
    exit 1
  }

  $baseUrl = "http://127.0.0.1:$Port"
  Write-Step "Waiting for image server"
  $health = Wait-Json "$baseUrl/health" 45
  if (-not $health) {
    throw "Timed out waiting for $baseUrl/health. Close any stale image server windows and rerun this script."
  }

  if ($health.sdnqModel -ne "flux2-4b-sdnq") {
    throw "Expected Windows SDNQ model flux2-4b-sdnq, got '$($health.sdnqModel)'."
  }

  $expectedDevice = $null
  if ($CpuOnly) {
    $expectedDevice = "cpu"
  } elseif ($Device -ne "auto") {
    $expectedDevice = $Device
  }

  if ($expectedDevice -and $health.device -ne $expectedDevice) {
    throw "Image server is running with device '$($health.device)', expected '$expectedDevice'. Close the existing image server window and rerun this script."
  }

  Write-Host "Image server health OK: device=$($health.device), backend=$($health.defaultBackend), model=$($health.sdnqModel)"

  Write-Step "Generating real 512px smoke image"
  $payload = @{
    backend = "sdnq-hs"
    prompt = $Prompt
    mode = "fast"
    aspect = "square"
    width = 512
    height = 512
    steps = 12
    guidance = 3.5
    timeout = $TimeoutSeconds
  }

  $result = Invoke-JsonPost "$baseUrl/generate" $payload ($TimeoutSeconds + 60)
  if (-not $result.url) {
    throw "Image server response did not include a generated image URL."
  }

  $relativeUrl = ([string]$result.url).TrimStart("/")
  $relativePath = $relativeUrl -replace "/", [System.IO.Path]::DirectorySeparatorChar
  $imagePath = Join-Path (Join-Path $Repo "public") $relativePath
  if (-not (Test-Path $imagePath)) {
    throw "Image server reported $($result.url), but the file was not found at $imagePath."
  }

  $imageFile = Get-Item $imagePath
  if ($imageFile.Length -le 0) {
    throw "Generated image was empty: $imagePath"
  }

  Write-Host ""
  Write-Host "Windows real image smoke passed." -ForegroundColor Green
  Write-Host "Backend: $($result.backend)"
  Write-Host "Device: $($health.device)"
  Write-Host "Steps: $($result.steps)"
  Write-Host "Guidance: $($result.guidance)"
  Write-Host "Image: $imagePath"
  Write-Host "Log: $LogPath"
} finally {
  if ($TranscriptStarted) {
    try {
      Stop-Transcript | Out-Null
    } catch {
      Write-Host "Could not stop transcript log: $($_.Exception.Message)" -ForegroundColor Yellow
    }
  }
}
