# PowerShell installer for tLog (Windows)
# Usage (recommended): Download then run with -ExecutionPolicy Bypass
#   Invoke-WebRequest -Uri 'https://raw.githubusercontent.com/FrederikNorlyk/tlog/main/scripts/windows-install.ps1' -OutFile $env:TEMP\tlog-install.ps1; powershell -ExecutionPolicy Bypass -File $env:TEMP\tlog-install.ps1

$ErrorActionPreference = 'Stop'

Write-Host "Creating temporary directory..."
$TEMP_DIR = Join-Path -Path ([System.IO.Path]::GetTempPath()) -ChildPath ("tlog_install_{0}" -f ([guid]::NewGuid()))
New-Item -ItemType Directory -Path $TEMP_DIR -Force | Out-Null

$assetName = "tlog-windows-x86_64.zip"
$url = "https://github.com/FrederikNorlyk/tlog/releases/latest/download/$assetName"
$zipPath = Join-Path $TEMP_DIR "tlog.zip"

Write-Host "Downloading $assetName from $url..."
Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing

Write-Host "Extracting..."
$extractPath = Join-Path $TEMP_DIR "extract"
Expand-Archive -Path $zipPath -DestinationPath $extractPath -Force

Write-Host "Locating tlog.exe..."
$tlogExe = Get-ChildItem -Path $extractPath -Filter "tlog.exe" -Recurse -File | Select-Object -First 1
if (-not $tlogExe) {
    Write-Error "tlog.exe not found inside archive. Aborting."
    Remove-Item -Recurse -Force $TEMP_DIR
    exit 1
}

# Choose install location: Program Files if running as admin, otherwise LOCALAPPDATA\Programs\tlog
$programFilesPath = Join-Path $env:ProgramFiles "tlog"
$userPath = Join-Path $env:LOCALAPPDATA "Programs\tlog"

$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $installDir = $programFilesPath
} else {
    $installDir = $userPath
}

Write-Host "Installing to $installDir"
New-Item -ItemType Directory -Path $installDir -Force | Out-Null
Copy-Item -Path $tlogExe.FullName -Destination (Join-Path $installDir "tlog.exe") -Force

# Add install dir to user PATH if it's not already there (setx affects the user environment)
$userPathVar = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($userPathVar -notlike "*$installDir*") {
    Write-Host "Adding $installDir to user PATH..."
    if ([string]::IsNullOrEmpty($userPathVar)) {
        $newPath = $installDir
    } else {
        $newPath = "$userPathVar;$installDir"
    }
    setx PATH $newPath | Out-Null
    Write-Host "User PATH updated. Restart terminal (or log out/in) to pick up changes."
} else {
    Write-Host "$installDir already in user PATH."
}

Write-Host "Cleaning up..."
Remove-Item -Recurse -Force $TEMP_DIR

Write-Host "Installation complete! You can run 'tlog' from a new shell."
