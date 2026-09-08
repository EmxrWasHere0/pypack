use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use crate::platform::{OperatingSystem, Target};
use std::env;
use std::process::Command;

/// Launcher'ın Rust kaynağı — derleme zamanında binary'ye gömülür.
/// Böylece pypack `cargo install` ile kurulmuş olsa bile kaynak eldedir.
pub const LAUNCHER_SRC: &str = include_str!("launcher_main.rs");

/// Kaynağın hash'i — launcher build cache'inin anahtarı
fn launcher_src_hash() -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(LAUNCHER_SRC.as_bytes());
    hex::encode(h.finalize())[..12].to_string()
}

/// Native launcher'ı hedef için derler (cache'li).
/// Host hedefiyse --target'sız, cross ise --target <triple> ile derlenir.
pub fn build_native_launcher(target: &Target) -> Result<PathBuf, String> {
    let hash = launcher_src_hash();
    let cache_root = crate::downloader::cache_dir();

    let cache_bin = cache_root
        .join("launchers")
        .join(format!("{}-{}", target.triplet(), hash))
        .join(target.bin_file_name("pypack-launcher"));
    if cache_bin.exists() {
        println!("    ✓ Launcher from cache: {}", cache_bin.display());
        return Ok(cache_bin);
    }

    ensure_cargo()?;

    // Geçici Cargo projesi (kaynak binary'ye gömülü — pypack'in kurulum
    // dizinine bağımlılık yok)
    let proj_dir = cache_root
        .join("launcher-build")
        .join(format!("{}-{}", target.triplet(), hash));
    write_cargo_project(&proj_dir)?;

    let triple: Option<&str> = if target.is_host() {
        None // default toolchain — host'ta her zaman çalışır
    } else {
        let t = target.rust_cross_triplet().ok_or_else(|| {
            format!(
                "Target {} couldn't compiled in this machine. \
                 Script launcher fallback will be used.",
                target
            )
        })?;
        ensure_target_installed(t)?;
        Some(t)
    };

    println!(
        "    ⚙ Compiling Rust launcher ({})...",
        if triple.is_some() { "cross" } else { "host" }
    );
    run_cargo_build(&proj_dir, triple)?;

    let built = match triple {
        Some(t) => proj_dir.join("target").join(t).join("release"),
        None => proj_dir.join("target").join("release"),
    }
    .join(target.bin_file_name("pypack-launcher"));

    if !built.exists() {
        return Err(format!("couldn't find compiled launcher: {}", built.display()));
    }

    if let Some(parent) = cache_bin.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cache directory: {}", e))?;
    }
    fs::copy(&built, &cache_bin).map_err(|e| format!("couldn't cache launcher: {}", e))?;
    Ok(cache_bin)
}

/// Derlenmiş launcher'ı bundle'a kurar: dist/app veya dist/app.exe
pub fn install_native_launcher(
    bundle_dir: &Path,
    target: &Target,
    app_name: &str,
) -> Result<PathBuf, String> {
    let src = build_native_launcher(target)?;
    let dest = bundle_dir.join(target.exe_file_name(app_name));
    fs::copy(&src, &dest).map_err(|e| format!("couldn't copy launcher: {}", e))?;
    make_executable(&dest);
    Ok(dest)
}

fn write_cargo_project(dir: &Path) -> Result<(), String> {
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).map_err(|e| format!("project directory: {}", e))?;

    fs::write(
        dir.join("Cargo.toml"),
        r#"[package]
name = "pypack-launcher"
version = "0.1.0"
edition = "2021"

[profile.release]
opt-level = "z"
lto = true
strip = true
panic = "abort"
codegen-units = 1
"#,
    )
    .map_err(|e| format!("couldn't write Cargo.toml: {}", e))?;

    fs::write(src_dir.join("main.rs"), LAUNCHER_SRC)
        .map_err(|e| format!("couldn't write launcher source: {}", e))?;
    Ok(())
}

fn ensure_cargo() -> Result<(), String> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    match Command::new(&cargo).arg("--version").output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(_) => Err("cargo is not working".to_string()),
        Err(_) => Err(
            "cargo bulunamadı. Native launcher için Rust toolchain gerekir; \
             yoksa script launcher'lar kullanılır."
                .to_string(),
        ),
    }
}

fn ensure_target_installed(triple: &str) -> Result<(), String> {
    let installed = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map(|o| {
            o.status.success()
                && String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .any(|l| l.trim() == triple)
        })
        .unwrap_or(false);

    if installed {
        return Ok(());
    }

    // Otomatik kurmayı dene (rustup varsa)
    let ok = Command::new("rustup")
        .args(["target", "add", triple])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if ok {
        Ok(())
    } else {
        Err(format!(
            "Rust target '{}' is not ready. Set it up manually: rustup target add {}",
            triple, triple
        ))
    }
}

fn run_cargo_build(dir: &Path, triple: Option<&str>) -> Result<(), String> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut cmd = Command::new(&cargo);
    cmd.current_dir(dir).args(["build", "--release"]);
    if let Some(t) = triple {
        cmd.args(["--target", t]);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("cargo çalıştırılamadı: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let hint = if stderr.contains("linker") {
            "\nTip: System linker is required for cross-compile \
             (Linux→Windows: 'sudo apt install mingw-w64', \
             Linux→linux-arm64: 'sudo apt install gcc-aarch64-linux-gnu')."
        } else {
            ""
        };
        return Err(format!("cargo build failed:{}\n{}", hint, stderr));
    }
    Ok(())
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(path, perms);
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Linux/macOS için launcher script oluşturur
pub fn create_unix_launcher(output_dir: &Path, app_name: &str, script_name: &str) -> Result<(), String> {
    let launcher_content = format!(
        r#"#!/bin/bash
# {app_name} - Python Runtime Bundle
# This script launches the app with built-in Python runtime.

set -e

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
PYTHON_DIR="$SCRIPT_DIR/python"
PYTHON_BIN="$PYTHON_DIR/bin/python3"

if [[ "$(uname)" == "Linux" ]]; then
    export LD_LIBRARY_PATH="$PYTHON_DIR/lib:$LD_LIBRARY_PATH"
fi

if [[ "$(uname)" == "Darwin" ]]; then
    export DYLD_LIBRARY_PATH="$PYTHON_DIR/lib:$DYLD_LIBRARY_PATH"
    export DYLD_FRAMEWORK_PATH="$PYTHON_DIR/lib:$DYLD_FRAMEWORK_PATH"
fi

export PYTHONHOME="$PYTHON_DIR"
export PYTHONPATH="$SCRIPT_DIR/lib:$PYTHON_DIR/lib/python3.11/site-packages"
export PYTHONNOUSERSITE=1
export PYTHONDONTWRITEBYTECODE=1

exec "$PYTHON_BIN" "$SCRIPT_DIR/app/{script_name}" "$@"
"#,
        app_name = app_name,
        script_name = script_name,
    );

    let launcher_path = output_dir.join(format!("{}.sh", app_name));
    fs::write(&launcher_path, launcher_content)
        .map_err(|e| format!("Couldn't create the launcher: {}", e))?;

    // Çalıştırılabilir yap
    let metadata = fs::metadata(&launcher_path)
        .map_err(|e| format!("Couldn't read the metadata: {}", e))?;
    let mut perms = metadata.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&launcher_path, perms)
        .map_err(|e| format!("Couldn't set the permissions: {}", e))?;

    Ok(())
}

/// Windows için launcher batch dosyası oluşturur
pub fn create_windows_launcher(output_dir: &Path, app_name: &str, script_name: &str) -> Result<(), String> {
    let launcher_content = format!(
        r#"@echo off
REM {app_name} - Python Runtime Bundle
REM This script launches the app with built-in Python runtime.

setlocal

set SCRIPT_DIR=%~dp0
set PYTHON_DIR=%SCRIPT_DIR%python
set PYTHON_BIN=%PYTHON_DIR%\python.exe

set PYTHONHOME=%PYTHON_DIR%
set PYTHONPATH=%SCRIPT_DIR%lib;%PYTHON_DIR%\Lib\site-packages
set PYTHONNOUSERSITE=1
set PYTHONDONTWRITEBYTECODE=1

"%PYTHON_BIN%" "%SCRIPT_DIR%app\{script_name}" %*

endlocal
"#,
        app_name = app_name,
        script_name = script_name,
    );

    let launcher_path = output_dir.join(format!("{}.bat", app_name));
    fs::write(&launcher_path, launcher_content)
        .map_err(|e| format!("Couldn't create Windows launcher: {}", e))?;

    // PowerShell script de oluştur
    let ps_content = format!(
        r#"# {app_name} - PowerShell Launcher
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PythonDir = Join-Path $ScriptDir "python"
$PythonBin = Join-Path $PythonDir "python.exe"

$env:PYTHONHOME = $PythonDir
$env:PYTHONPATH = Join-Path $ScriptDir "lib"
$env:PYTHONNOUSERSITE = "1"
$env:PYTHONDONTWRITEBYTECODE = "1"

& $PythonBin (Join-Path $ScriptDir "app" "{script_name}") @args
"#,
        app_name = app_name,
        script_name = script_name,
    );

    let ps_path = output_dir.join(format!("{}.ps1", app_name));
    fs::write(&ps_path, ps_content)
        .map_err(|e| format!("Couldn't create PowerShell launcher: {}", e))?;

    Ok(())
}


/// Launcher'ı hedef platform için oluşturur
pub fn create_launcher(
    output_dir: &Path,
    target: &Target,
    app_name: &str,
    script_name: &str,
) -> Result<(), String> {
    match target.os {
        OperatingSystem::Windows => create_windows_launcher(output_dir, app_name, script_name),
        _ => create_unix_launcher(output_dir, app_name, script_name),
    }
}

/// README dosyası oluşturur
pub fn create_readme(output_dir: &Path, app_name: &str, target: &Target) -> Result<(), String> {
    let readme = format!(
        r#"# {app_name}

## Python Runtime Bundle

This package includes every module that is
needed to make {app_name} run independently.

- Python runtime (standalone build)
- Application code
- Dependencies

## Platform: {target}

## Usage

### Native (recommended)
Linux/macOS:  ./demoapp
Windows:      demoapp.exe

### Script fallback
Linux/macOS:  ./demoapp.sh
Windows:      demoapp.bat  (or demoapp.ps1)
```
Directory Tree

{app_name}/
├── {app_name}          # Linux/macOS launcher
├── {app_name}.bat      # Windows launcher
├── {app_name}.ps1      # PowerShell launcher
├── python/             # Python runtime
│   ├── bin/            # Python binary (Linux/macOS)
│   ├── lib/            # Python libraries
│   └── python.exe      # Python binary (Windows)
├── app/                # Application code
│   └── main.py         # Main script
├── lib/                # Extra libraries
└── launcher_src/       # Rust launcher source code
    └── main.rs

System requirements

    {target} compatible operating system

This package is created with pypack
"#,
app_name = app_name,
target = target,
);

fs::write(output_dir.join("README.md"), readme)
    .map_err(|e| format!("Couldn't create README.md: {}", e))?;

Ok(())

}