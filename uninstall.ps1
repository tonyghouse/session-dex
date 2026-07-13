#Requires -Version 5.1

[CmdletBinding()]
param(
  [switch]$Yes,
  [switch]$DeleteData
)

$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $RootDir

function Confirm-Step {
  param(
    [string]$Prompt,
    [bool]$DefaultYes
  )

  if ($script:Yes) {
    Write-Host "$Prompt yes"
    return $true
  }

  $suffix = if ($DefaultYes) { "[Y/n]" } else { "[y/N]" }
  $answer = Read-Host "$Prompt $suffix"

  if ([string]::IsNullOrWhiteSpace($answer)) {
    return $DefaultYes
  }

  return $answer -match "^[Yy]$"
}

function Confirm-DataDelete {
  if ($script:DeleteData) {
    Write-Host "Preference/data deletion enabled by -DeleteData."
    return $true
  }

  if ($script:Yes) {
    Write-Host "Keeping SessionDex preferences and app data. Use -DeleteData to remove them."
    return $false
  }

  return Confirm-Step "Delete SessionDex preferences and app data, including sessiondex.sqlite3?" $false
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

  if (-not (Confirm-Step "Close SessionDex before uninstalling?" $true)) {
    Write-Host "Uninstall skipped. Close SessionDex and rerun this script."
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
    Write-Host "Uninstall skipped. Close SessionDex and rerun this script."
    return $false
  }

  Get-SessionDexProcess | Stop-Process -Force

  if (Wait-SessionDexExit 5) {
    return $true
  }

  Write-Host "SessionDex is still running. Uninstall skipped." -ForegroundColor Red
  return $false
}

function Split-UninstallCommand {
  param([string]$Command)

  if ([string]::IsNullOrWhiteSpace($Command)) {
    return $null
  }

  $trimmed = $Command.Trim()
  if ($trimmed.StartsWith('"')) {
    $closingQuote = $trimmed.IndexOf('"', 1)
    if ($closingQuote -gt 1) {
      $filePath = $trimmed.Substring(1, $closingQuote - 1)
      $arguments = $trimmed.Substring($closingQuote + 1).Trim()
      return [pscustomobject]@{ FilePath = $filePath; Arguments = $arguments }
    }
  }

  $match = [regex]::Match($trimmed, '^(?<path>.*?\.exe)(?<args>\s+.*)?$', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
  if ($match.Success) {
    return [pscustomobject]@{
      FilePath = $match.Groups["path"].Value.Trim()
      Arguments = $match.Groups["args"].Value.Trim()
    }
  }

  return $null
}

function Get-RegistryUninstallCommands {
  $roots = @(
    "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*",
    "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*",
    "HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*"
  )

  foreach ($root in $roots) {
    Get-ItemProperty -Path $root -ErrorAction SilentlyContinue |
      Where-Object { $_.DisplayName -eq "SessionDex" -or $_.DisplayName -like "SessionDex *" } |
      ForEach-Object {
        if ($_.QuietUninstallString) {
          Split-UninstallCommand $_.QuietUninstallString
        } elseif ($_.UninstallString) {
          Split-UninstallCommand $_.UninstallString
        }

        if ($_.InstallLocation) {
          $uninstaller = Join-Path $_.InstallLocation "uninstall.exe"
          if (Test-Path -LiteralPath $uninstaller) {
            [pscustomobject]@{ FilePath = $uninstaller; Arguments = "" }
          }
        }
      }
  }
}

function Get-KnownUninstallCommands {
  $paths = @()

  if ($env:LOCALAPPDATA) {
    $paths += Join-Path $env:LOCALAPPDATA "Programs\SessionDex\uninstall.exe"
    $paths += Join-Path $env:LOCALAPPDATA "SessionDex\uninstall.exe"
  }
  if ($env:ProgramFiles) {
    $paths += Join-Path $env:ProgramFiles "SessionDex\uninstall.exe"
  }
  if (${env:ProgramFiles(x86)}) {
    $paths += Join-Path ${env:ProgramFiles(x86)} "SessionDex\uninstall.exe"
  }

  $paths |
    Where-Object { Test-Path -LiteralPath $_ } |
    ForEach-Object { [pscustomobject]@{ FilePath = $_; Arguments = "" } }
}

function Invoke-SessionDexUninstaller {
  $commands = @(Get-RegistryUninstallCommands) + @(Get-KnownUninstallCommands)
  $command = $commands |
    Where-Object { $_ -and $_.FilePath -and (Test-Path -LiteralPath $_.FilePath) } |
    Select-Object -First 1

  if (-not $command) {
    Write-Host "No SessionDex uninstaller executable found."
    return $false
  }

  $arguments = $command.Arguments
  if ($arguments -notmatch '(^|\s)/S(\s|$)') {
    $arguments = "$arguments /S".Trim()
  }

  Write-Host "Running $($command.FilePath)"
  $process = Start-Process -FilePath $command.FilePath -ArgumentList $arguments -Wait -PassThru
  if ($process.ExitCode -ne 0) {
    Write-Host "SessionDex uninstaller exited with code $($process.ExitCode)." -ForegroundColor Yellow
  }

  return $true
}

function Get-AppInstallPaths {
  $paths = @()

  if ($env:LOCALAPPDATA) {
    $paths += Join-Path $env:LOCALAPPDATA "Programs\SessionDex"
    $paths += Join-Path $env:LOCALAPPDATA "SessionDex"
  }
  if ($env:ProgramFiles) {
    $paths += Join-Path $env:ProgramFiles "SessionDex"
  }
  if (${env:ProgramFiles(x86)}) {
    $paths += Join-Path ${env:ProgramFiles(x86)} "SessionDex"
  }

  $paths
}

function Get-ShortcutPaths {
  $paths = @()

  if ($env:APPDATA) {
    $paths += Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\SessionDex.lnk"
    $paths += Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\SessionDex"
  }
  if ($env:USERPROFILE) {
    $paths += Join-Path $env:USERPROFILE "Desktop\SessionDex.lnk"
  }
  if ($env:PUBLIC) {
    $paths += Join-Path $env:PUBLIC "Desktop\SessionDex.lnk"
  }

  $paths
}

function Remove-PathSafe {
  param(
    [string]$Path,
    [string]$Label
  )

  if ([string]::IsNullOrWhiteSpace($Path)) {
    return
  }

  if (-not (Test-Path -LiteralPath $Path)) {
    Write-Host "Not found: $Path"
    return
  }

  try {
    Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction Stop
    Write-Host "Removed ${Label}: $Path"
  } catch {
    Write-Host "Could not remove ${Path}: $($_.Exception.Message)" -ForegroundColor Yellow
    Write-Host "If this is under Program Files, rerun PowerShell as Administrator."
  }
}

function Remove-AppFallbackPaths {
  Write-Host "Checking common app install locations."
  Get-AppInstallPaths | ForEach-Object { Remove-PathSafe $_ "app" }
  Get-ShortcutPaths | ForEach-Object { Remove-PathSafe $_ "shortcut" }
}

function Get-DataPaths {
  $paths = @()

  if ($env:APPDATA) {
    $paths += Join-Path $env:APPDATA "io.sessiondex.desktop"
  }
  if ($env:LOCALAPPDATA) {
    $paths += Join-Path $env:LOCALAPPDATA "io.sessiondex.desktop"
  }

  $paths
}

function Write-DataSummary {
  Write-Host ""
  Write-Host "Preferences and app data" -ForegroundColor White
  Write-Host "SessionDex stores its app-owned SQLite metadata as sessiondex.sqlite3 in the app data directory."
  Write-Host "Deleting this data removes custom names, hidden/pinned sessions, settings, and cache."
  Write-Host "Codex and Claude session history will not be touched."
  Write-Host ""
  Write-Host "Paths checked:"
  Get-DataPaths | ForEach-Object { Write-Host "  $_" }
}

Write-Host "SessionDex Uninstaller" -ForegroundColor White
Write-Host "----------------------"
Write-Host ""
Write-Host "This removes the installed SessionDex app."
Write-Host "Later, this script asks whether to delete SessionDex local data. Press Enter to keep it."

if (Confirm-Step "Remove the SessionDex application now?" $true) {
  if (-not (Request-CloseRunningApp)) {
    exit 0
  }

  Write-Host ""
  Write-Host "Removing Windows app" -ForegroundColor White
  $uninstallerRan = Invoke-SessionDexUninstaller
  Remove-AppFallbackPaths
} else {
  Write-Host "Application removal skipped."
}

Write-DataSummary
if (Confirm-DataDelete) {
  if (-not (Request-CloseRunningApp)) {
    exit 0
  }

  Get-DataPaths | ForEach-Object { Remove-PathSafe $_ "preferences/data" }
  Write-Host "SessionDex preferences and app data removed." -ForegroundColor Green
} else {
  Write-Host "SessionDex preferences and app data preserved."
}

Write-Host ""
Write-Host "SessionDex uninstall finished." -ForegroundColor Green
