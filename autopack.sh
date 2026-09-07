#!/bin/bash
# PyPack Auto Installer

downloader() {
    if command -v apt &> /dev/null; then
        sudo apt update && sudo apt install -y rustc cargo
        return 0
    elif command -v dnf &> /dev/null; then
        sudo dnf install -y rust cargo
        return 0
    elif command -v pacman &> /dev/null; then
        sudo pacman -S --noconfirm rust
        return 0
    elif command -v brew &> /dev/null; then
        brew install rust
        return 0
    else
        echo "Couldn't find apt, dnf, pacman or brew installed in the system"
        return 1
    fi
}

echo "╔════════════════════════════════════════════════════════╗"
echo "║  PyPack - Python Code Bundler                          ║"
echo "║  Turn your codes into single executables.              ║"
echo "╚════════════════════════════════════════════════════════╝"

if [[ $(uname -s) == "Linux" ]]; then
    if [[ -f /etc/os-release ]]; then
        source /etc/os-release
        echo "Operating System: $PRETTY_NAME $(uname -r)"
    else
        echo "Operating System: Linux (Unknown Distro)"
    fi
elif [[ $(uname -s) == "Darwin" ]]; then
    echo "Operating System: $(sw_vers -productName) $(sw_vers -productVersion)"
else
    echo "Operating System: Unknown"
fi

echo
echo "This installer will set up PyPack into your computer."
echo "[1/2] Downloading Rust..."
if command -v cargo &> /dev/null; then
    echo "Rust/Cargo is already installed. Skipping."
else
    downloader
    if [[ $? -ne 0 ]]; then
        echo "Rust installation failed. Exiting."
        exit 1
    fi
fi

echo "Rust is ready."
echo "[2/2] Installing/Downloading PyPack"

read -p "Is the repo cloned? (y/N): " conf
if [[ "${conf,,}" == "y" ]]; then
    read -p "Please enter the path of the cloned repo: " path
    
    path=$(eval echo "$path")
    
    if [[ ! -d "$path" ]]; then
        echo "Error: Directory does not exist!"
        exit 1
    fi
    
    cd "$path" || exit 1
    
    echo "Compiling and installing PyPack via Cargo..."
    if cargo install --path .; then
        echo "Installation successful. Please make sure to add $HOME/.cargo/bin to your PATH."
        exit 0
    else
        echo "Installation failed during cargo install."
        exit 1
    fi
else
    echo "[*] Installing PyPack from the official repo to $HOME/pypack.zip"
    curl -Lf -o "$HOME/pypack.zip" https://github.com/EmxrWasHere0/pypack/archive/refs/heads/master.zip
    
    cd "$HOME" || exit 1
    if ! command -v unzip &> /dev/null; then
        mkdir -pv pypack
        echo "Cannot unzip the archive automatically (unzip not found)."
        echo "Please unzip pypack.zip into $HOME/pypack directory manually."
        echo "Tree should be like this: $HOME/pypack/pypack-master/..."
        read -p "Hit [ENTER] after unzipping the archive..."
    else
        unzip -qo "$HOME/pypack.zip" -d "$HOME/pypack"
    fi
    
    if [[ -d "$HOME/pypack/pypack-master" ]]; then
        cd "$HOME/pypack/pypack-master" || exit 1
        echo "Compiling and installing PyPack via Cargo..."
        if cargo install --path .; then
            echo "Installation successful. Please make sure to add $HOME/.cargo/bin to your PATH."
            exit 0
        else
            echo "Installation failed during cargo install."
            exit 1
        fi
    else
        echo "Error: Extracted directory not found!"
        exit 1
    fi
fi

echo "Installation wizard is completed."
