#Requires -Version 5.1

$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $RootDir

function Confirm-Step {
  param(
    [string]$Prompt,
    [bool]$DefaultYes
  )

  $suffix = if ($DefaultYes) { "[Y/n]" } else { "[y/N]" }
  $answer = Read-Host "$Prompt $suffix"

  if ([string]::IsNullOrWhiteSpace($answer)) {
    return $DefaultYes
  }

  return $answer -match "^[Yy]$"
}

function Get-SessionDexProcess {
  Get-Process -Name "SessionDex", "sessiondex" -ErrorAction SilentlyContinue
}

function Wait-SessionDexExit {
  param([int]$Seconds)

  $deadline = (Get-Date).AddSeconds($Seconds)

  while ((Get-Date) -lt $deadline) {
    if (-not (Get-SessionDexProcess)) {
      return $true
    }

    Start-Sleep -Seconds 1
  }

  return -not (Get-SessionDexProcess)
}

function Request-CloseRunningApp {
  $processes = Get-SessionDexProcess
  if (-not $processes) {
    return $true
  }

  Write-Host "SessionDex is currently running." -ForegroundColor Yellow

  if (-not (Confirm-Step "Close SessionDex before installing the generated app?" $true)) {
    Write-Host "App installation skipped. Rerun .\install.cmd after closing SessionDex."
    return $false
  }

  $processes | ForEach-Object {
    try {
      $_.CloseMainWindow() | Out-Null
    } catch {
      # Process may have exited between detection and close request.
    }
  }

  if (Wait-SessionDexExit 15) {
    return $true
  }

  Write-Host "SessionDex did not close within 15 seconds." -ForegroundColor Yellow

  if (-not (Confirm-Step "Force close SessionDex now?" $false)) {
    Write-Host "App installation skipped. Close SessionDex and rerun .\install.cmd."
    return $false
  }

  Get-SessionDexProcess | Stop-Process -Force

  if (Wait-SessionDexExit 5) {
    return $true
  }

  Write-Host "SessionDex is still running. App installation skipped." -ForegroundColor Red
  return $false
}

function Invoke-Doctor {
  param([switch]$Install)

  $doctorScript = Join-Path $RootDir "scripts\doctor.ps1"
  $arguments = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $doctorScript)

  if ($Install) {
    $arguments += "-Install"
  }

  & powershell @arguments
  $script:DoctorExitCode = $LASTEXITCODE
}

Write-Host "SessionDex Source Installer" -ForegroundColor White
Write-Host "---------------------------"
Write-Host ""

Invoke-Doctor
if ($script:DoctorExitCode -ne 0) {
  if (Confirm-Step "Run available prerequisite installers now?" $false) {
    Invoke-Doctor -Install
  }

  Write-Host ""
  Write-Host "Rechecking prerequisites..."
  Invoke-Doctor
  if ($script:DoctorExitCode -ne 0) {
    Write-Host ""
    Write-Host "Install cannot continue until the required prerequisites pass." -ForegroundColor Red
    exit 1
  }
}

if (-not (Confirm-Step "Build SessionDex now?" $true)) {
  Write-Host "Build skipped."
  exit 0
}

Write-Host ""
Write-Host "Installing npm dependencies" -ForegroundColor White
& npm ci
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

Write-Host ""
Write-Host "Building SessionDex (nsis)" -ForegroundColor White
& npm run build -- --bundles nsis
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

Write-Host ""
Write-Host "Build complete." -ForegroundColor Green
Write-Host "Generated artifacts are under src-tauri\target\release\bundle"

if (Confirm-Step "Install the generated app now?" $true) {
  if (-not (Request-CloseRunningApp)) {
    exit 0
  }

  $nsisDir = Join-Path $RootDir "src-tauri\target\release\bundle\nsis"
  $setup = Get-ChildItem -Path $nsisDir -Recurse -Filter "*setup*.exe" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime |
    Select-Object -Last 1

  if ($setup) {
    Write-Host "Running $($setup.FullName)"
    Start-Process -FilePath $setup.FullName -Wait
  } else {
    Write-Host "Could not find generated NSIS setup executable." -ForegroundColor Yellow
  }
} else {
  Write-Host "App installation skipped."
}
