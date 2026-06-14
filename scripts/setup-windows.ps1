param(
  [switch]$CpuOnly,
  [switch]$SkipImageSetup,
  [switch]$ValidateOnly
)

$ErrorActionPreference = "Stop"
$Repo = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $Repo
$NonInteractive = $ValidateOnly -or ($env:CI -eq "true")

function Write-Step($Message) {
  Write-Host ""
  Write-Host "==> $Message" -ForegroundColor Cyan
}

function Stop-WithHelp($Message, $Url = $null) {
  Write-Host ""
  Write-Host $Message -ForegroundColor Red
  if ($Url) {
    Start-Process $Url
  }
  Write-Host "Fix the above, then double-click Launch-Windows.bat again."
  if (-not $NonInteractive) {
    Read-Host "Press Enter to close"
  }
  exit 1
}

function Test-Command($Name) {
  return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Refresh-Path {
  $machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
  $user = [Environment]::GetEnvironmentVariable("Path", "User")
  $current = $env:Path
  $env:Path = (@($machine, $user, $current) | Where-Object { $_ } | Select-Object -Unique) -join ";"
}

function Expand-UserPath($Value) {
  if (-not $Value) {
    return $Value
  }
  $expanded = [Environment]::ExpandEnvironmentVariables($Value)
  if ($expanded -eq "~") {
    return $HOME
  }
  if ($expanded.StartsWith("~\") -or $expanded.StartsWith("~/")) {
    return Join-Path $HOME $expanded.Substring(2)
  }
  return $expanded
}

function Invoke-Checked($FailureMessage, $Exe, [string[]]$CommandArgs) {
  & $Exe @CommandArgs
  if ($LASTEXITCODE -ne 0) {
    Stop-WithHelp $FailureMessage
  }
}

function Get-LatestWriteTimeUtc([string[]]$Paths) {
  $latest = [DateTime]::MinValue
  foreach ($PathValue in $Paths) {
    if (-not (Test-Path $PathValue)) {
      continue
    }
    $Item = Get-Item $PathValue
    if ($Item.PSIsContainer) {
      $Child = Get-ChildItem $PathValue -Recurse -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
      if ($Child -and $Child.LastWriteTimeUtc -gt $latest) {
        $latest = $Child.LastWriteTimeUtc
      }
    } elseif ($Item.LastWriteTimeUtc -gt $latest) {
      $latest = $Item.LastWriteTimeUtc
    }
  }
  return $latest
}

function Install-WithWinget($Id, $Name) {
  if (-not (Test-Command "winget")) {
    return $false
  }
  Write-Step "Installing $Name with winget"
  winget install --id $Id --exact --source winget --accept-package-agreements --accept-source-agreements
  if ($LASTEXITCODE -ne 0) {
    return $false
  }
  Refresh-Path
  return $true
}

function Wait-Http($Url, $Seconds) {
  $deadline = (Get-Date).AddSeconds($Seconds)
  while ((Get-Date) -lt $deadline) {
    try {
      Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 $Url | Out-Null
      return $true
    } catch {
      Start-Sleep -Seconds 1
    }
  }
  return $false
}

function Get-PythonCommand {
  $candidates = @(
    @{ Exe = "py"; Args = @("-3.11") },
    @{ Exe = "py"; Args = @("-3.12") },
    @{ Exe = "py"; Args = @("-3.10") },
    @{ Exe = "python"; Args = @() }
  )

  foreach ($candidate in $candidates) {
    if (-not (Test-Command $candidate.Exe)) {
      continue
    }
    $probeArgs = @($candidate.Args) + @("-c", "import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)")
    & $candidate.Exe @probeArgs *> $null
    if ($LASTEXITCODE -eq 0) {
      return $candidate
    }
  }
  return $null
}

Write-Step "Open Dungeon Windows launcher"

if ($ValidateOnly) {
  Write-Step "Validating Windows launcher and image routing"
  if (-not (Test-Command "node")) {
    Stop-WithHelp "Node.js is required for validation."
  }
  $Python = Get-PythonCommand
  if (-not $Python) {
    Stop-WithHelp "Python 3.10+ is required for validation."
  }
  Invoke-Checked "start-image-server syntax check failed." "node" @("--check", "scripts/start-image-server.mjs")
  Invoke-Checked "run-python syntax check failed." "node" @("--check", "scripts/run-python.mjs")
  Invoke-Checked "image routing check failed." "npm" @("run", "check:image-routing")
  Write-Host "Windows launcher validation passed." -ForegroundColor Green
  exit 0
}

if (-not (Test-Command "node")) {
  if (-not (Install-WithWinget "OpenJS.NodeJS.LTS" "Node.js LTS")) {
    Stop-WithHelp "Node.js 20+ is required." "https://nodejs.org"
  }
}

$nodeMajor = [int](& node -p "process.versions.node.split('.')[0]")
if ($nodeMajor -lt 22) {
  Stop-WithHelp "Node.js 22+ is required; found $(node -v). Node 20 does not have Windows prebuilt binaries for this app's SQLite dependency." "https://nodejs.org"
}

if (-not (Test-Command "ollama")) {
  if (-not (Install-WithWinget "Ollama.Ollama" "Ollama")) {
    Stop-WithHelp "Ollama is required for the local narrator." "https://ollama.com/download"
  }
}

if (-not (Wait-Http "http://127.0.0.1:11434/api/version" 2)) {
  Write-Step "Starting Ollama"
  Start-Process -WindowStyle Minimized -FilePath "ollama" -ArgumentList "serve"
  if (-not (Wait-Http "http://127.0.0.1:11434/api/version" 30)) {
    Stop-WithHelp "Ollama did not start. Open Ollama manually, then relaunch."
  }
}

if (-not ((ollama list 2>$null) -match "gemma4:12b-it-qat")) {
  Write-Step "Downloading the default narrator model (gemma4:12b-it-qat, one time)"
  ollama pull gemma4:12b-it-qat
  if ($LASTEXITCODE -ne 0) {
    Stop-WithHelp "Model download failed. Check your connection and relaunch."
  }
}

$NodeInstallStamp = "node_modules\.package-lock.json"
$NpmInputs = @("package.json", "package-lock.json")
$NeedsNpmInstall = -not (Test-Path "node_modules") -or
  -not (Test-Path $NodeInstallStamp) -or
  ((Get-LatestWriteTimeUtc $NpmInputs) -gt (Get-Item $NodeInstallStamp).LastWriteTimeUtc)

if ($NeedsNpmInstall) {
  Write-Step "Installing app dependencies"
  Invoke-Checked "npm install failed." "npm" @("install")
}

$BuildStamp = ".next\BUILD_ID"
$BuildInputs = @(
  "src",
  "package.json",
  "package-lock.json",
  "next.config.ts",
  "postcss.config.mjs",
  "tsconfig.json"
)
$NeedsBuild = -not (Test-Path $BuildStamp) -or
  ((Get-LatestWriteTimeUtc $BuildInputs) -gt (Get-Item $BuildStamp).LastWriteTimeUtc)

if ($NeedsBuild) {
  Write-Step "Building Open Dungeon"
  Invoke-Checked "Build failed." "npm" @("run", "build")
}

if (-not $SkipImageSetup) {
  if (-not (Test-Command "git")) {
    if (-not (Install-WithWinget "Git.Git" "Git")) {
      Stop-WithHelp "Git is required to fetch ultra-fast-image-gen." "https://git-scm.com/download/win"
    }
  }

  $UltraDir = if ($env:ULTRA_FAST_IMAGE_GEN_DIR) {
    Expand-UserPath $env:ULTRA_FAST_IMAGE_GEN_DIR
  } else {
    Join-Path $HOME "ultra-fast-image-gen"
  }

  if (-not (Test-Path $UltraDir)) {
    Write-Step "Cloning ultra-fast-image-gen"
    git clone https://github.com/newideas99/ultra-fast-image-gen.git $UltraDir
    if ($LASTEXITCODE -ne 0) {
      Stop-WithHelp "Could not clone ultra-fast-image-gen."
    }
  }

  $VenvDir = Join-Path $UltraDir ".venv"
  $VenvPython = Join-Path $VenvDir "Scripts\python.exe"
  if (-not (Test-Path $VenvPython)) {
    $Python = Get-PythonCommand
    if (-not $Python) {
      if (-not (Install-WithWinget "Python.Python.3.11" "Python 3.11")) {
        Stop-WithHelp "Python 3.10+ is required for local image generation." "https://www.python.org/downloads/windows/"
      }
      Refresh-Path
      $Python = Get-PythonCommand
    }
    if (-not $Python) {
      Stop-WithHelp "Python 3.10+ was not found after install. Relaunch this script."
    }

    Write-Step "Creating ultra-fast-image-gen virtual environment"
    $venvArgs = @($Python.Args) + @("-m", "venv", $VenvDir)
    Invoke-Checked "Could not create the Python virtual environment." $Python.Exe $venvArgs
  }

  $ImageDevice = if ($env:IMAGE_SERVER_DEVICE) {
    $env:IMAGE_SERVER_DEVICE
  } elseif ($CpuOnly) {
    "cpu"
  } elseif (Test-Command "nvidia-smi") {
    "cuda"
  } else {
    "cpu"
  }

  $TorchIndex = if ($ImageDevice -eq "cuda") {
    "https://download.pytorch.org/whl/cu128"
  } else {
    "https://download.pytorch.org/whl/cpu"
  }

  $Requirements = Join-Path $UltraDir "requirements.txt"
  $Stamp = Join-Path $VenvDir ".open-dungeon-windows-$ImageDevice.stamp"
  if (-not (Test-Path $Stamp)) {
    Write-Step "Installing image dependencies ($ImageDevice)"
    Invoke-Checked "pip upgrade failed." $VenvPython @("-m", "pip", "install", "--upgrade", "pip", "setuptools", "wheel")
    Invoke-Checked "PyTorch install failed. Try relaunching with: powershell -File scripts\setup-windows.ps1 -CpuOnly" $VenvPython @("-m", "pip", "install", "torch", "torchvision", "--index-url", $TorchIndex)
    Invoke-Checked "ultra-fast-image-gen dependency install failed." $VenvPython @("-m", "pip", "install", "-r", $Requirements)
    Set-Content -Path $Stamp -Value "device=$ImageDevice`ninstalled=$(Get-Date -Format o)`n" -Encoding UTF8
  }

  if (-not (Wait-Http "http://127.0.0.1:7869/health" 2)) {
    Write-Step "Starting local image server ($ImageDevice)"
    $imageCommand = @"
`$env:ULTRA_FAST_IMAGE_GEN_DIR = '$UltraDir'
`$env:ULTRA_FAST_IMAGE_GEN_PYTHON = '$VenvPython'
`$env:IMAGE_SERVER_DEVICE = '$ImageDevice'
`$env:IMAGE_SERVER_DEFAULT_BACKEND = 'sdnq-hs'
Set-Location '$Repo'
npm run image:server
"@
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($imageCommand))
    Start-Process powershell -ArgumentList @("-NoExit", "-NoProfile", "-ExecutionPolicy", "Bypass", "-EncodedCommand", $encoded)
  }
}

Write-Step "Starting Open Dungeon at http://localhost:3000"
Start-Job -ScriptBlock {
  Start-Sleep -Seconds 3
  Start-Process "http://localhost:3000"
} | Out-Null

npm run start
