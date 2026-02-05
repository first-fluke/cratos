# Cratos Installer for Windows
# Usage: irm https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.ps1 | iex
#
# Environment variables:
#   CRATOS_INSTALL_DIR - Installation directory (default: ~/.cratos/bin)
#   CRATOS_VERSION     - Version to install (default: latest)
#   CRATOS_NO_WIZARD   - Skip running wizard after install (default: false)

$ErrorActionPreference = 'Stop'

# GitHub repository
$REPO = "first-fluke/cratos"
$BINARY_NAME = "cratos.exe"

# Print banner
function Write-Banner {
    Write-Host ""
    Write-Host "  =====================================================================" -ForegroundColor Cyan
    Write-Host "  |                                                                   |" -ForegroundColor Cyan
    Write-Host "  |           Cratos - AI-Powered Personal Assistant                  |" -ForegroundColor Cyan
    Write-Host "  |                       Installer                                   |" -ForegroundColor Cyan
    Write-Host "  |                                                                   |" -ForegroundColor Cyan
    Write-Host "  =====================================================================" -ForegroundColor Cyan
    Write-Host ""
}

# Print info message
function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor Blue
}

# Print success message
function Write-Success {
    param([string]$Message)
    Write-Host "[OK] $Message" -ForegroundColor Green
}

# Print warning message
function Write-Warning {
    param([string]$Message)
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

# Print error message and exit
function Write-ErrorAndExit {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
    exit 1
}

# Get architecture
function Get-Architecture {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        "X64" { return "x86_64" }
        "Arm64" { return "aarch64" }
        default { Write-ErrorAndExit "Unsupported architecture: $arch" }
    }
}

# Get target triple
function Get-Target {
    $arch = Get-Architecture
    return "${arch}-pc-windows-msvc"
}

# Get latest version from GitHub
function Get-LatestVersion {
    try {
        $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$REPO/releases/latest" -UseBasicParsing
        return $response.tag_name
    }
    catch {
        Write-ErrorAndExit "Failed to fetch latest version: $_"
    }
}

# Get installation directory
function Get-InstallDir {
    if ($env:CRATOS_INSTALL_DIR) {
        return $env:CRATOS_INSTALL_DIR
    }

    # Default to user's local bin directory
    $installDir = Join-Path $env:USERPROFILE ".cratos\bin"

    if (-not (Test-Path $installDir)) {
        New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    }

    return $installDir
}

# Add directory to PATH
function Add-ToPath {
    param([string]$Directory)

    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")

    if ($currentPath -notlike "*$Directory*") {
        $newPath = "$Directory;$currentPath"
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        $env:Path = "$Directory;$env:Path"
        Write-Warning "Added $Directory to PATH"
        Write-Warning "Please restart your terminal for PATH changes to take effect"
    }
}

# Main installation
function Install-Cratos {
    Write-Banner

    # Detect platform
    $target = Get-Target
    Write-Info "Detected platform: $target"

    # Get version
    if ($env:CRATOS_VERSION) {
        $version = $env:CRATOS_VERSION
    }
    else {
        Write-Info "Fetching latest version..."
        $version = Get-LatestVersion
    }
    Write-Info "Installing version: $version"

    # Get installation directory
    $installDir = Get-InstallDir
    Write-Info "Installation directory: $installDir"

    # Create temporary directory
    $tmpDir = Join-Path $env:TEMP "cratos-install-$(Get-Random)"
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

    try {
        # Download binary
        $archiveName = "cratos-${target}.zip"
        $downloadUrl = "https://github.com/$REPO/releases/download/$version/$archiveName"
        $archivePath = Join-Path $tmpDir $archiveName

        Write-Info "Downloading from: $downloadUrl"

        try {
            Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath -UseBasicParsing
        }
        catch {
            Write-ErrorAndExit "Failed to download. Check if the release exists at: https://github.com/$REPO/releases"
        }

        # Extract
        Write-Info "Extracting archive..."
        Expand-Archive -Path $archivePath -DestinationPath $tmpDir -Force

        # Install
        Write-Info "Installing to $installDir..."
        $binaryPath = Join-Path $tmpDir $BINARY_NAME
        $destPath = Join-Path $installDir $BINARY_NAME

        if (Test-Path $destPath) {
            Remove-Item $destPath -Force
        }
        Move-Item -Path $binaryPath -Destination $destPath -Force

        # Add to PATH
        Add-ToPath $installDir

        # Verify installation
        $installedBinary = Join-Path $installDir $BINARY_NAME
        if (Test-Path $installedBinary) {
            Write-Success "Cratos installed successfully!"
        }
        else {
            Write-ErrorAndExit "Installation verification failed."
        }

        # Print version
        Write-Host ""
        & $installedBinary --version
        Write-Host ""

        # Run wizard unless disabled
        if (-not $env:CRATOS_NO_WIZARD) {
            Write-Host ""
            Write-Info "Starting setup wizard..."
            Write-Host ""
            & $installedBinary wizard
        }
        else {
            Write-Host ""
            Write-Success "Installation complete!"
            Write-Host ""
            Write-Host "  Next steps:"
            Write-Host "    1. Run the setup wizard:  cratos wizard"
            Write-Host "    2. Or run init:           cratos init"
            Write-Host "    3. Start the server:      cratos serve"
            Write-Host ""
        }
    }
    finally {
        # Cleanup
        if (Test-Path $tmpDir) {
            Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

# Run main
Install-Cratos
