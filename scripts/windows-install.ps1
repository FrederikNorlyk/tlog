# PowerShell installer for tLog (Windows)
# Usage (recommended): Download then run with -ExecutionPolicy Bypass
#   Invoke-WebRequest -Uri 'https://raw.githubusercontent.com/FrederikNorlyk/tlog/main/scripts/windows-install.ps1' -OutFile $env:TEMP\tlog-install.ps1; powershell -ExecutionPolicy Bypass -File $env:TEMP\tlog-install.ps1

$ErrorActionPreference = 'Stop'

Write-Host "Creating temporary directory..."
$TEMP_DIR = Join-Path -Path ([System.IO.Path]::GetTempPath()) -ChildPath ("tlog_install_{0}" -f ([Guid]::NewGuid()))
New-Item -ItemType Directory -Path $TEMP_DIR -Force | Out-Null

try {
    $assetName = "tlog-windows-x86_64.zip"
    $url = "https://github.com/FrederikNorlyk/tlog/releases/latest/download/$assetName"
    $zipPath = Join-Path $TEMP_DIR "tlog.zip"

    Write-Host "Downloading $assetName..."
    try {
        Invoke-WebRequest -Uri $url -OutFile $zipPath
    }
    catch {
        Write-Error "Failed to download $url"
        exit 1
    }

    if ((Get-Item $zipPath).Length -eq 0) {
        Write-Error "Downloaded file is empty."
        exit 1
    }

    Write-Host "Extracting..."
    $extractPath = Join-Path $TEMP_DIR "extract"
    Expand-Archive -Path $zipPath -DestinationPath $extractPath -Force

    Write-Host "Locating tlog.exe..."
    $tlogExe = Get-ChildItem -Path $extractPath -Filter "tlog.exe" -Recurse -File | Select-Object -First 1

    if (-not $tlogExe) {
        Write-Error "tlog.exe not found inside archive. Aborting."
        exit 1
    }

    # Install into the current user's local Programs directory.
    $installDir = Join-Path $env:LOCALAPPDATA "Programs\tlog"

    if (Test-Path (Join-Path $installDir "tlog.exe")) {
        Write-Host "Updating existing installation..."
    }
    else {
        Write-Host "Installing..."
    }

    Write-Host "Installing to $installDir"
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Copy-Item -Path $tlogExe.FullName -Destination (Join-Path $installDir "tlog.exe") -Force

    # Add install directory to the user's PATH if it isn't already present.
    $userPathVar = [Environment]::GetEnvironmentVariable("PATH", "User")

    $pathEntries = @()
    if (-not [string]::IsNullOrWhiteSpace($userPathVar)) {
        $pathEntries = $userPathVar -split ';'
    }

    if ($pathEntries -notcontains $installDir) {
        Write-Host "Adding $installDir to user PATH..."

        if ([string]::IsNullOrWhiteSpace($userPathVar)) {
            $newPath = $installDir
        }
        else {
            $newPath = "$userPathVar;$installDir"
        }

        [Environment]::SetEnvironmentVariable(
            "PATH",
            $newPath,
            "User"
        )

        Write-Host "User PATH updated. Restart any open terminals before using tlog."
    }
    else {
        Write-Host "$installDir is already in the user PATH."
    }

    Write-Host ""
    Write-Host "Installation complete!"
    Write-Host "Open a new terminal and run:"
    Write-Host "  tlog"
}
finally {
    Write-Host "Cleaning up..."
    Remove-Item -Recurse -Force $TEMP_DIR -ErrorAction SilentlyContinue
}
