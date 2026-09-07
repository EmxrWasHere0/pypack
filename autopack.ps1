# PyPack Auto Installer for PowerShell

function Install-Rust {
    if (Get-Command winget -ErrorAction SilentlyContinue) {
        Write-Host "Rust is being installed via winget..."
        winget install --id Rustlang.Rust -e --accept-source-agreements --accept-package-agreements
    } else {
        Write-Host "winget could not be found. Please install Rust manually from https://rustup.rs"
        exit 1
    }
}

Write-Host "╔════════════════════════════════════════════════════════╗"
Write-Host "║  PyPack - Python Code Bundler                          ║"
Write-Host "║  Turn your codes into single executables.              ║"
Write-Host "╚════════════════════════════════════════════════════════╝"

$osDescription = if ($PSVersionTable.Platform -eq 'Unix') { uname -s } else { "Windows" }
Write-Host "Operating System: $osDescription"
Write-Host ""
Write-Host "This installer will set up PyPack into your computer."
Write-Host "[1/2] Checking Rust..."

if (Get-Command cargo -ErrorAction SilentlyContinue) {
    Write-Host "Rust/Cargo is already installed. Skipping."
} else {
    Install-Rust
}

Write-Host "Rust is ready."
Write-Host "[2/2] Installing/Downloading PyPack"

$conf = Read-Host "Is the repo cloned? (y/N)"

if ($conf.ToLower() -eq 'y') {
    $path = Read-Host "Please enter the path of the cloned repo"
    $path = $path.Trim('"').Trim("'")
    
    if (-not (Test-Path $path)) {
        Write-Host "Error: Directory does not exist!"
        exit 1
    }
    
    Set-Location $path
    Write-Host "Compiling and installing PyPack via Cargo..."
    
    cargo install --path .
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Installation successful. Please make sure to add $HOME/.cargo/bin to your PATH."
        exit 0
    } else {
        Write-Host "Installation failed during cargo install."
        exit 1
    }
} else {
    $homeDir = $HOME
    $zipPath = Join-Path $homeDir "pypack.zip"
    $extractPath = Join-Path $homeDir "pypack"
    
    Write-Host "[*] Installing PyPack from the official repo to $zipPath"
    
    Invoke-WebRequest -Uri "https://github.com" -OutFile $zipPath
    
    if (-not (Test-Path $extractPath)) {
        New-Item -ItemType Directory -Path $extractPath | Out-Null
    }
    
    Write-Host "Extracting archive..."
    Expand-Archive -Path $zipPath -DestinationPath $extractPath -Force
    
    $targetSource = Join-Path $extractPath "pypack-master"
    if (Test-Path $targetSource) {
        Set-Location $targetSource
        Write-Host "Compiling and installing PyPack via Cargo..."
        
        cargo install --path .
        if ($LASTEXITCODE -eq 0) {
            Write-Host "Installation successful. Please make sure to add $HOME/.cargo/bin to your PATH."
            exit 0
        } else {
            Write-Host "Installation failed during cargo install."
            exit 1
        }
    } else {
        Write-Host "Error: Extracted directory not found!"
        exit 1
    }
}

Write-Host "Installation wizard is completed."
