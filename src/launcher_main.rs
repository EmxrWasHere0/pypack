// pypack native launcher — std-only, bağımlılık YOK.
// Bu dosya hem bağımsız binary olarak derlenir ([[bin]] pypack-launcher)
// hem de pypack tarafından include_str! ile gömülüp hedef platforma derlenir.
//
// Görevi: kendi bulunduğu dizindeki pypack.manifest'i okuyarak
// bundle içindeki Python runtime'ı ve uygulama entry point'ini bulur,
// ortamı ayarlar ve Python sürecini başlatıp exit code'u iletir.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const MANIFEST_FILE: &str = "pypack.manifest";

fn main() {
    if let Err(err) = run() {
        eprintln!("pypack-launcher: {}", err);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    // 1) Launcher'ın bulunduğu dizin (CWD'den BAĞIMSIZ)
    let exe_dir = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .ok_or("launcher konumu belirlenemedi")?;

    // 2) Manifest'i oku
    let manifest = read_manifest(&exe_dir.join(MANIFEST_FILE))?;

    let entry = manifest
        .get("entry")
        .cloned()
        .ok_or("manifest'te 'entry' eksik")?;
    let python_home = manifest
        .get("python_home")
        .cloned()
        .unwrap_or_else(|| "python".to_string());
    // "false" yazılmadıkça true
    let chdir = manifest.get("chdir").map(|v| v != "false").unwrap_or(true);
    let set_pythonhome = manifest
        .get("set_pythonhome")
        .map(|v| v != "false")
        .unwrap_or(true);

    // 3) Entry point ve Python runtime'ı doğrula
    let entry_path = exe_dir.join(&entry);
    if !entry_path.is_file() {
        return Err(format!("giriş noktası bulunamadı: {}", entry_path.display()));
    }

    let home = exe_dir.join(python_home);
    let python_bin = find_python_bin(&home)?;

    // 4) Ortam değişkenleri
    if set_pythonhome {
        env::set_var("PYTHONHOME", &home);
    }

    // PYTHONPATH: bundle lib/ + runtime site-packages + mevcut değer
    let mut pythonpath_parts: Vec<PathBuf> = vec![exe_dir.join("lib")];
    if let Some(sp) = site_packages_dir(&home) {
        pythonpath_parts.push(sp);
    }
    if let Ok(existing) = env::var("PYTHONPATH") {
        if !existing.is_empty() {
            pythonpath_parts.push(PathBuf::from(existing));
        }
    }
    if let Ok(joined) = env::join_paths(&pythonpath_parts) {
        env::set_var("PYTHONPATH", joined);
    }

    env::set_var("PYTHONNOUSERSITE", "1");
    env::set_var("PYTHONDONTWRITEBYTECODE", "1");

    // Platform'a göre dinamik kütüphane yolları
    #[cfg(target_os = "linux")]
    prepend_library_path("LD_LIBRARY_PATH", &[home.join("lib")]);

    #[cfg(target_os = "macos")]
    {
        prepend_library_path("DYLD_LIBRARY_PATH", &[home.join("lib")]);
        prepend_library_path("DYLD_FRAMEWORK_PATH", &[home.join("lib")]);
    }

    // 5) CWD: relative path'ler launcher dizinine göre çözülsün.
    //    (manifest'ten chdir=false ile kapatılabilir)
    if chdir {
        env::set_current_dir(&exe_dir)
            .map_err(|e| format!("çalışma dizini değiştirilemedi: {}", e))?;
    }

    // 6) Python'u başlat, argümanları ve exit code'u ilet
    let mut cmd = Command::new(&python_bin);
    cmd.arg(&entry_path);
    for arg in env::args().skip(1) {
        cmd.arg(arg);
    }

    let status = cmd.status().map_err(|e| {
        format!(
            "Python başlatılamadı: {}\n  binary: {}\n  script: {}",
            e,
            python_bin.display(),
            entry_path.display()
        )
    })?;

    propagate_exit(status);
}

/// Python sürecinin exit code'unu launcher'a iletir.
/// Unix'te sinyalle ölümse 128+signal döndürür (shell konvansiyonu).
fn propagate_exit(status: std::process::ExitStatus) -> ! {
    if let Some(code) = status.code() {
        std::process::exit(code);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            std::process::exit(128 + sig);
        }
    }
    std::process::exit(1);
}

/// Platform'a göre Python binary yolunu bulur
fn find_python_bin(home: &Path) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let candidates = [home.join("python.exe"), home.join("bin").join("python.exe")];

    #[cfg(not(target_os = "windows"))]
    let candidates = [
        home.join("bin").join("python3"),
        home.join("bin").join("python"),
    ];

    for c in candidates {
        if c.exists() {
            return Ok(c);
        }
    }
    Err(format!(
        "Python runtime bulunamadı: {} altında python binary yok",
        home.display()
    ))
}

/// Runtime'ın site-packages dizini
fn site_packages_dir(home: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        Some(home.join("Lib").join("site-packages"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        // unix: <home>/lib/python3.X/site-packages (X bilinmediği için tara)
        let lib = home.join("lib");
        let entries = fs::read_dir(&lib).ok()?;
        for e in entries.flatten() {
            let name = e.file_name();
            if name.to_string_lossy().starts_with("python3") {
                return Some(lib.join(name).join("site-packages"));
            }
        }
        None
    }
}

/// Ortam değişkeninin başına path dizileri ekler (LD_LIBRARY_PATH vb.)
fn prepend_library_path(var: &str, dirs: &[PathBuf]) {
    let mut parts: Vec<PathBuf> = dirs.to_vec();
    if let Ok(existing) = env::var(var) {
        if !existing.is_empty() {
            for p in existing.split(':') {
                parts.push(PathBuf::from(p));
            }
        }
    }
    if let Ok(joined) = env::join_paths(&parts) {
        env::set_var(var, joined);
    }
}

/// Basit key=value manifest parser (# yorum ve boş satır toleranslı)
fn read_manifest(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("manifest okunamadı ({}): {}", path.display(), e))?;

    let mut map = BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_parses_keys_comments_and_blanks() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("m");
        std::fs::write(
            &p,
            "# yorum\n\nentry=app/main.py\npython_home = python\nchdir=false\n",
        )
        .unwrap();
        let m = read_manifest(&p).unwrap();
        assert_eq!(m.get("entry").unwrap(), "app/main.py");
        assert_eq!(m.get("python_home").unwrap(), "python");
        assert_eq!(m.get("chdir").unwrap(), "false");
    }
}