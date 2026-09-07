#!/bin/bash
# PyPack Auto Installer

downloader() {
    if command -v apt &> /dev/null; then
        sudo apt install rust
        return 0
    elif command -v dnf &> /dev/null; then
        sudo dnf install rust
        return 0
    elif command -v pacman &> /dev/null; then
        sudo pacman -S rust
        return 0
    elif command -v brew &> /dev/null; then
        brew install rust
        return 0
    else
        echo "Couldn't find apt, dnf, pacman or brew installed in the system"
        return 1
    fi
}

echo ╔════════════════════════════════════════════════════════╗
echo ║  PyPack - Python Code Bundler                          ║
echo ║  Turn your codes into single executables.              ║
echo ╚════════════════════════════════════════════════════════╝

if [[ $(uname -s) == "Linux" ]]; then
    source /etc/os-release
    echo Operating System: $PRETTY_NAME $(uname -r)
elif [[ $(uname -s) == "Darwin" ]]; then
    echo Operating System: $(sw_vers -productName) $(sw_vers -productVersion)
else
    echo Operating System: Unknown
fi

echo
echo This installer will set up PyPack into your computer.
echo "[1/2] Downloading Rust..."
if command -v rustc &> /dev/null; then
    echo Rust is already installed. Skipping.
else
    downloader
fi

echo Downloaded Rust successfully.
echo "[2/2] Installing/Downloading PyPack"

read -p "Is the repo cloned? (y/N): " conf
if [[ $conf == "y" || $conf == "Y" ]]; then
    read -p "Please enter the path of the cloned repo: " path
    cd $path
    cargo build --release
    if [[ -f "$path/target/pypack" ]]; then
        echo PyPack compiled successfully. Installing...
        cargo install --path $path
        if [[ -f "$HOME/.cargo/bin/pypack" ]]; then
            echo "Installation successfull. Please make sure to add $HOME/.cargo/bin to your path."
            return 0 
        else
            echo Installation failed.
            return 1
        fi
    else
        echo Failed to compile.
        return 1
    fi
else
    echo "[*] Installing PyPack from the official repo to $HOME/pypack.zip"
    curl -o "$HOME/pypack.zip" 
