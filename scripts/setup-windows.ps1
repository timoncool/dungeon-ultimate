param(
  [switch]$CpuOnly,
  [switch]$SkipImageSetup,
  [switch]$SetupImages,
  [switch]$SetupOllama,
  [switch]$ImageOnly,
  [switch]$ValidateOnly
)

$ErrorActionPreference = "Stop"
$Repo = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $Repo
$NonInteractive = $ValidateOnly -or ($env:CI -eq "true")
$LogDir = Join-Path $Repo "logs"
$LatestImageServerLog = Join-Path $LogDir "windows-image-server-latest.txt"

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

function Confirm-Yes($Message) {
  if ($NonInteractive) {
    return $false
  }
  Write-Host ""
  Write-Host $Message -ForegroundColor Yellow
  $answer = Read-Host "Type Y to continue, or press Enter to skip"
  return $answer.Trim().ToUpperInvariant() -eq "Y"
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

function ConvertTo-PowerShellLiteral($Value) {
  return "'" + ([string]$Value).Replace("'", "''") + "'"
}

function Test-PowerShellFile($RelativePath, $Name) {
  $tokens = $null
  $errors = $null
  $path = Join-Path $Repo $RelativePath
  [System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors) | Out-Null
  if ($errors.Count -gt 0) {
    $message = ($errors | Select-Object -First 1).Message
    Stop-WithHelp "$Name syntax check failed: $message"
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

function Update-GitRepoIfClean($Path, $Name) {
  if (-not (Test-Path (Join-Path $Path ".git"))) {
    Write-Host "$Name already exists at $Path, but it is not a git checkout. Skipping update." -ForegroundColor Yellow
    return
  }

  Push-Location $Path
  try {
    $dirty = git status --porcelain
    if ($LASTEXITCODE -ne 0) {
      Write-Host "Could not inspect $Name git status. Continuing with the existing checkout." -ForegroundColor Yellow
      return
    }

    if ($dirty) {
      Write-Host "$Name has local changes. Skipping automatic update so your files are left alone." -ForegroundColor Yellow
      return
    }

    Write-Step "Updating $Name"
    git pull --ff-only
    if ($LASTEXITCODE -ne 0) {
      Write-Host "Could not fast-forward $Name. Continuing with the existing checkout." -ForegroundColor Yellow
    }
  } finally {
    Pop-Location
  }
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

function Get-JsonHealth($Url, $Seconds = 0) {
  $deadline = (Get-Date).AddSeconds($Seconds)
  while ($true) {
    try {
      return Invoke-RestMethod -UseBasicParsing -TimeoutSec 2 -Uri $Url
    } catch {
      if ((Get-Date) -ge $deadline) {
        return $null
      }
      Start-Sleep -Seconds 1
    }
  }
}

function Get-HealthValue($Health, $Name) {
  if (-not $Health) {
    return $null
  }
  $property = $Health.PSObject.Properties[$Name]
  if (-not $property) {
    return $null
  }
  return $property.Value
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
    try {
      & $candidate.Exe @probeArgs *> $null
      $exitCode = $LASTEXITCODE
    } catch {
      $exitCode = 1
    }
    if ($exitCode -eq 0) {
      return $candidate
    }
  }
  return $null
}

function Test-CudaAvailable($PythonExe) {
  $probe = @"
import sys
try:
    import torch
except Exception as exc:
    print(f"torch import failed: {exc}")
    sys.exit(2)

print(f"torch={torch.__version__}")
print(f"cuda_available={torch.cuda.is_available()}")
if torch.cuda.is_available():
    print(f"cuda_device={torch.cuda.get_device_name(0)}")
    sys.exit(0)
sys.exit(1)
"@
  & $PythonExe -c $probe
  $exitCode = $LASTEXITCODE
  if ($exitCode -eq 2) {
    Stop-WithHelp "PyTorch could not be imported inside ultra-fast-image-gen's virtual environment."
  }
  return $exitCode -eq 0
}

function Test-UltraFastImageGenContract($PythonExe, $GeneratePath) {
  if (-not (Test-Path $GeneratePath)) {
    Stop-WithHelp "Missing ultra-fast-image-gen generate.py at $GeneratePath."
  }

  $helpOutput = & $PythonExe $GeneratePath "--help" 2>&1
  if ($LASTEXITCODE -ne 0) {
    Write-Host ($helpOutput -join "`n") -ForegroundColor Yellow
    Stop-WithHelp "ultra-fast-image-gen generate.py --help failed."
  }
  if (($helpOutput -join "`n") -notmatch "flux2-4b-sdnq") {
    Stop-WithHelp "ultra-fast-image-gen no longer advertises flux2-4b-sdnq; Open Dungeon's Windows CUDA/CPU image route needs an update."
  }
}

Write-Step "Open Dungeon Windows launcher"

if ($ImageOnly -and $SkipImageSetup) {
  Stop-WithHelp "-ImageOnly cannot be combined with -SkipImageSetup."
}
if ($SetupImages -and $SkipImageSetup) {
  Stop-WithHelp "-SetupImages cannot be combined with -SkipImageSetup."
}

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
  Test-PowerShellFile "scripts/launch-windows.ps1" "Windows launch script"
  Test-PowerShellFile "scripts/check-windows-launchers.ps1" "Windows launcher check script"
  Test-PowerShellFile "scripts/smoke-windows-image.ps1" "image smoke script"
  Test-PowerShellFile "scripts/stop-windows.ps1" "Windows stop script"
  Test-PowerShellFile "scripts/diagnose-windows.ps1" "Windows diagnostics script"
  Test-PowerShellFile "scripts/run-windows-image-loop.ps1" "Windows image loop script"
  & (Join-Path $Repo "scripts\check-windows-launchers.ps1")
  Invoke-Checked "image routing check failed." "npm" @("run", "check:image-routing")
  Invoke-Checked "image server HTTP smoke failed." "npm" @("run", "check:image-server-http")
  Write-Host "Windows launcher validation passed." -ForegroundColor Green
  exit 0
}

if (-not (Test-Command "node")) {
  if (-not (Install-WithWinget "OpenJS.NodeJS.LTS" "Node.js LTS")) {
    Stop-WithHelp "Node.js 22+ is required." "https://nodejs.org"
  }
}

$nodeMajor = [int](& node -p "process.versions.node.split('.')[0]")
if ($nodeMajor -lt 22) {
  Stop-WithHelp "Node.js 22+ is required; found $(node -v). Node 20 does not have Windows prebuilt binaries for this app's SQLite dependency." "https://nodejs.org"
}

if (-not $ImageOnly) {
  $UseOllama = $SetupOllama
  if (-not (Test-Command "ollama")) {
    if ($SetupOllama -or (Confirm-Yes "Ollama is optional. Install it only if you want Open Dungeon to manage the default local narrator model on this machine.")) {
      if (-not (Install-WithWinget "Ollama.Ollama" "Ollama")) {
        Stop-WithHelp "Ollama install failed or winget is unavailable. Install Ollama manually or use Connect a server in Text Model settings." "https://ollama.com/download"
      }
      $UseOllama = $true
    } else {
      Write-Host "Skipping Ollama install. In the app, choose Text Model -> Connect a server to use LM Studio, llama.cpp, OpenRouter, or another OpenAI-compatible backend." -ForegroundColor Yellow
    }
  }

  if (Test-Command "ollama") {
    if (-not $UseOllama) {
      $UseOllama = Wait-Http "http://127.0.0.1:11434/api/version" 2
    }
    if (-not $UseOllama -and (Confirm-Yes "Ollama is installed but not required. Start/check Ollama and the default local narrator model now?")) {
      $UseOllama = $true
    }
  }

  if ($UseOllama) {
    if (-not (Wait-Http "http://127.0.0.1:11434/api/version" 2)) {
      Write-Step "Starting Ollama"
      Start-Process -WindowStyle Minimized -FilePath "ollama" -ArgumentList "serve"
      if (-not (Wait-Http "http://127.0.0.1:11434/api/version" 30)) {
        Stop-WithHelp "Ollama did not start. Open Ollama manually, then relaunch."
      }
    }

    if (-not ((ollama list 2>$null) -match "gemma4:12b-it-qat")) {
      if ($SetupOllama -or (Confirm-Yes "Download the default local narrator model gemma4:12b-it-qat now? This is optional if you use Connect a server.")) {
        Write-Step "Downloading the default narrator model (gemma4:12b-it-qat, one time)"
        ollama pull gemma4:12b-it-qat
        if ($LASTEXITCODE -ne 0) {
          Stop-WithHelp "Model download failed. Check your connection and relaunch."
        }
      } else {
        Write-Host "Skipping default Ollama model download. Use Text Model -> Connect a server, or pull a local model later." -ForegroundColor Yellow
      }
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
}

if ($ImageOnly) {
  $ShouldSetupImages = $true
} elseif ($SkipImageSetup) {
  $ShouldSetupImages = $false
} elseif ($SetupImages) {
  $ShouldSetupImages = $true
} else {
  $ShouldSetupImages = Confirm-Yes "Local image generation is optional and needs Git, Python, and ultra-fast-image-gen. Set it up now?"
}

if ($ShouldSetupImages) {
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
  } else {
    Update-GitRepoIfClean $UltraDir "ultra-fast-image-gen"
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
  $Generate = Join-Path $UltraDir "generate.py"
  if (-not (Test-Path $Requirements)) {
    Stop-WithHelp "Missing ultra-fast-image-gen requirements.txt at $Requirements."
  }

  $Stamp = Join-Path $VenvDir ".open-dungeon-windows-$ImageDevice.stamp"
  $ActiveDeviceStamp = Join-Path $VenvDir ".open-dungeon-windows-active-device"
  $ActiveImageDevice = if (Test-Path $ActiveDeviceStamp) {
    (Get-Content -Path $ActiveDeviceStamp -TotalCount 1).Trim()
  } else {
    ""
  }
  $NeedsImageDeps = -not (Test-Path $Stamp) -or
    ($ActiveImageDevice -ne $ImageDevice) -or
    ((Get-Item $Requirements).LastWriteTimeUtc -gt (Get-Item $Stamp).LastWriteTimeUtc)

  if ($NeedsImageDeps) {
    Write-Step "Installing image dependencies ($ImageDevice)"
    $FilteredRequirements = Join-Path ([System.IO.Path]::GetTempPath()) "open-dungeon-ultra-fast-image-gen-requirements.txt"
    Get-Content -Path $Requirements |
      Where-Object { $_ -notmatch '^\s*(torch|torchvision)(\s|[<>=~!;\[]|$)' } |
      Set-Content -Path $FilteredRequirements -Encoding UTF8
    Invoke-Checked "pip upgrade failed." $VenvPython @("-m", "pip", "install", "--upgrade", "pip", "setuptools", "wheel")
    Invoke-Checked "PyTorch install failed. Try relaunching with: powershell -File scripts\setup-windows.ps1 -SetupImages -CpuOnly" $VenvPython @("-m", "pip", "install", "torch", "torchvision", "--index-url", $TorchIndex)
    Invoke-Checked "ultra-fast-image-gen dependency install failed." $VenvPython @("-m", "pip", "install", "-r", $FilteredRequirements)
    Set-Content -Path $Stamp -Value "device=$ImageDevice`ninstalled=$(Get-Date -Format o)`n" -Encoding UTF8
    Set-Content -Path $ActiveDeviceStamp -Value $ImageDevice -Encoding UTF8
  }

  Write-Step "Checking ultra-fast-image-gen CLI contract"
  Test-UltraFastImageGenContract $VenvPython $Generate

  if ($ImageDevice -eq "cuda") {
    Write-Step "Checking PyTorch CUDA availability"
    if (-not (Test-CudaAvailable $VenvPython)) {
      Write-Host "PyTorch CUDA is not available even though an NVIDIA tool was detected. Starting the image worker in CPU mode." -ForegroundColor Yellow
      Write-Host "After updating NVIDIA drivers/CUDA support, run Launch-Windows-Image-Smoke.bat to try CUDA again." -ForegroundColor Yellow
      $ImageDevice = "cpu"
    }
  }

  $ImageServerHealthUrl = "http://127.0.0.1:7869/health"
  $ExistingImageServerHealth = Get-JsonHealth $ImageServerHealthUrl 2
  if (-not $ExistingImageServerHealth) {
    Write-Step "Starting local image server ($ImageDevice)"
    New-Item -ItemType Directory -Path $LogDir -Force | Out-Null
    $ImageServerLog = Join-Path $LogDir ("windows-image-server-{0}.log" -f (Get-Date -Format "yyyyMMdd-HHmmss"))
    Set-Content -Path $LatestImageServerLog -Value $ImageServerLog -Encoding UTF8
    Write-Host "Image server log: $ImageServerLog"

    $UltraDirLiteral = ConvertTo-PowerShellLiteral $UltraDir
    $VenvPythonLiteral = ConvertTo-PowerShellLiteral $VenvPython
    $ImageDeviceLiteral = ConvertTo-PowerShellLiteral $ImageDevice
    $RepoLiteral = ConvertTo-PowerShellLiteral $Repo
    $ImageServerLogLiteral = ConvertTo-PowerShellLiteral $ImageServerLog
    $imageCommand = @"
`$env:ULTRA_FAST_IMAGE_GEN_DIR = $UltraDirLiteral
`$env:ULTRA_FAST_IMAGE_GEN_PYTHON = $VenvPythonLiteral
`$env:IMAGE_SERVER_DEVICE = $ImageDeviceLiteral
`$env:IMAGE_SERVER_DEFAULT_BACKEND = 'sdnq-hs'
Set-Location $RepoLiteral
Write-Host ('Image server log: ' + $ImageServerLogLiteral)
npm run image:server *>&1 | Tee-Object -FilePath $ImageServerLogLiteral -Append
"@
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($imageCommand))
    Start-Process powershell -ArgumentList @("-NoExit", "-NoProfile", "-ExecutionPolicy", "Bypass", "-EncodedCommand", $encoded)
  } else {
    $ExistingDevice = Get-HealthValue $ExistingImageServerHealth "device"
    $ExistingBackend = Get-HealthValue $ExistingImageServerHealth "defaultBackend"
    $ExistingSdnqModel = Get-HealthValue $ExistingImageServerHealth "sdnqModel"

    Write-Host "Existing image server: device=$ExistingDevice, backend=$ExistingBackend, model=$ExistingSdnqModel"

    if ($ExistingDevice -and ($ExistingDevice -ne $ImageDevice)) {
      Write-Host "Existing image server is running with device '$ExistingDevice', but this launcher selected '$ImageDevice'." -ForegroundColor Yellow
      Write-Host "Close the existing image server PowerShell window and relaunch if you want to switch devices." -ForegroundColor Yellow
    }
    if ($ExistingBackend -and ($ExistingBackend -ne "sdnq-hs")) {
      Write-Host "Existing image server default backend is '$ExistingBackend'. Windows launchers expect 'sdnq-hs'." -ForegroundColor Yellow
    }
    if ($ExistingSdnqModel -and ($ExistingSdnqModel -ne "flux2-4b-sdnq")) {
      Write-Host "Existing image server SDNQ model is '$ExistingSdnqModel'. Windows launchers expect 'flux2-4b-sdnq'." -ForegroundColor Yellow
    }
    if (-not $ExistingDevice -or -not $ExistingBackend -or -not $ExistingSdnqModel) {
      Write-Host "Existing image server did not report the full Windows health contract. Close it and relaunch if image generation fails." -ForegroundColor Yellow
    }

    if (Test-Path $LatestImageServerLog) {
      $ImageServerLog = Get-Content -Path $LatestImageServerLog -TotalCount 1
      if ($ImageServerLog) {
        Write-Host "Existing image server log: $ImageServerLog"
      }
    }
  }
} elseif (-not $ImageOnly) {
  Write-Host "Skipping optional local image generation setup. The app can still launch for text play." -ForegroundColor Yellow
  Write-Host "Run Launch-Windows-Image-Smoke.bat or Launch-Windows-Image-Loop.bat later to install and test the image worker." -ForegroundColor Yellow
}

if ($ImageOnly) {
  Write-Step "Image worker setup complete"
  Write-Host "The image server should be available at http://127.0.0.1:7869/health." -ForegroundColor Green
  return
}

Write-Step "Starting Open Dungeon at http://localhost:3000"
Start-Job -ScriptBlock {
  Start-Sleep -Seconds 3
  Start-Process "http://localhost:3000"
} | Out-Null

npm run start
