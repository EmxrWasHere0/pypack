use std::fs;
use std::path::{Path, PathBuf};

use crate::analyzer::AnalysisResult;
use crate::downloader;
use crate::launcher;
use crate::platform::{OperatingSystem, Target};

pub struct BundleConfig {
    pub app_name: String,
    pub script_path: PathBuf,
    pub output_dir: PathBuf,
    pub targets: Vec<Target>,
    pub python_version: String,
    pub clean: bool,
    pub try_native: bool,
}

pub struct BundleResult {
    pub output_path: PathBuf,
    pub target: Target,
    pub total_size: u64,
}

/// Tüm hedef platformlar için bundle oluşturur
pub fn bundle_all(config: &BundleConfig) -> Result<Vec<BundleResult>, String> {
    // Önce Python'ı bul ve analizi yap
    let python = crate::analyzer::find_python()?;
    println!("Found Python: {}", python.display());

    let analysis = crate::analyzer::analyze_script(&python, &config.script_path)?;
    println!("Analysis Completed:");
    println!("  Python version    : {}", analysis.python_version);
    println!("  Imports           : {}", analysis.user_imports.len());
    println!("  External packages : {:?}", analysis.external_packages);

    let mut results = Vec::new();

    for target in &config.targets {
        println!("\nPackaging for {} target...", target);
        let result = bundle_for_target(config, &analysis, target)?;
        results.push(result);
    }

    Ok(results)
}

/// Belirli bir hedef platform için bundle oluşturur
fn bundle_for_target(
    config: &BundleConfig,
    analysis: &AnalysisResult,
    target: &Target,
) -> Result<BundleResult, String> {
    // Çıktı dizinini hazırla
    let target_dir = config.output_dir.join(format!("{}_{}", config.app_name, target));
    if target_dir.exists() && config.clean {
        fs::remove_dir_all(&target_dir)
            .map_err(|e| format!("Couldn't delete the old directory: {}", e))?;
    }

    fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Couldn't create the output directory: {}", e))?;

    // 1. Python runtime indir ve aç
        // 1. Python runtime indir ve aç
    println!("  [1/5] Downloading Python runtime...");
    let archive_path = downloader::download_python(target, &config.python_version)?;

    // Python'ı geçici dizine aç
    let temp_dir = config.output_dir.join(format!(".tmp_{}", target));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)
            .map_err(|e| format!("Couldn't delete temp directory: {}", e))?;
    }
    fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Couldn't create temp directory: {}", e))?;

    downloader::extract_archive(&archive_path, &temp_dir)?;

    // python dizinini bul (genelde python/ veya install/ altında)
    let python_dir = find_python_dir(&temp_dir, target)?;

    // Python dizinini hedefe taşı
    let dest_python_dir = target_dir.join("python");
    if dest_python_dir.exists() {
        fs::remove_dir_all(&dest_python_dir)
            .map_err(|e| format!("Couldn't delete the Python directory: {}", e))?;
    }
    fs::rename(&python_dir, &dest_python_dir)
        .or_else(|_| copy_dir_recursive(&python_dir, &dest_python_dir))
        .map_err(|e| format!("Couldn't move the Python directory: {}", e))?;

    // 2. Uygulama kodunu kopyala
    println!("  [2/5] Copying application code...");
    let app_dir = target_dir.join("app");
    fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Couldn't create the app directory: {}", e))?;

    let script_name = config.script_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("main.py");

    // Ana script'i kopyala
    fs::copy(&config.script_path, app_dir.join(script_name))
        .map_err(|e| format!("Couldn't copy the script: {}", e))?;

    // Aynı dizindeki diğer .py dosyalarını da kopyala
    if let Some(parent) = config.script_path.parent() {
        let local_files = crate::analyzer::find_local_python_files(parent);
        for file in local_files {
            if file != config.script_path {
                let file_name = file.file_name().unwrap();
                let dest = app_dir.join(file_name);
                if !dest.exists() {
                    fs::copy(&file, &dest)
                        .map_err(|e| format!("Couldn't copy the file: {}", e))?;
                }
            }
        }
    }

    // 3. Bağımlılıkları indir ve ekle
    println!("  [3/5] Installing dependencies...");
    let lib_dir = target_dir.join("lib");
    fs::create_dir_all(&lib_dir)
        .map_err(|e| format!("Couldn't create the lib directory: {}", e))?;

    if !analysis.external_packages.is_empty() {
        let packages_dir = lib_dir.join("packages");
        fs::create_dir_all(&packages_dir)
            .map_err(|e| format!("Couldn't create the packages directory: {}", e))?;

        downloader::download_packages(
            &analysis.python_executable,   // ← bu satır eksik
            &analysis.external_packages,
            target,
            &config.python_version,
            &packages_dir,
        )?;

        // Wheel dosyalarını aç
        extract_wheels(&packages_dir, target)?;
    }

    // 4. Launcher oluştur
        // 4. Launcher: native (öncelik) + script (fallback) + manifest
    println!("  [4/5] Creating Launcher...");
    if config.try_native {
        match launcher::install_native_launcher(&target_dir, target, &config.app_name) {
            Ok(p) => println!(
                "    ✓ Native launcher: {}",
                p.file_name().unwrap_or_default().to_string_lossy()
            ),
            Err(e) => {
                println!("    ⚠ No native launcher found: {}", e);
                println!("    → Fallback to: Script launcher");
            }
        }
    }

    // Script launcher'lar her zaman üretilir (fallback garantisi)
    launcher::create_launcher(&target_dir, target, &config.app_name, script_name)?;

    // Native launcher'ın okuduğu bundle manifest'i
    let manifest = format!(
        "# pypack bundle manifest\nname={}\nentry=app/{}\npython_home=python\nchdir=true\n",
        config.app_name, script_name
    );
    fs::write(target_dir.join("pypack.manifest"), manifest)
        .map_err(|e| format!("couldn't write manifest: {}", e))?;

    // README oluştur
    launcher::create_readme(&target_dir, &config.app_name, target)?;

    // 5. Temizlik ve boyut hesaplama
    println!("  [5/5] Last touch...");
    let total_size = calculate_dir_size(&target_dir);
    let size_mb = total_size as f64 / (1024.0 * 1024.0);

    // Temp dizini temizle
    if temp_dir.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
    }

    println!("  ✓ Package completed: {} ({:.1} MB)", target_dir.display(), size_mb);

    Ok(BundleResult {
        output_path: target_dir,
        target: *target,
        total_size,
    })
}

/// Açılmış arşivde Python dizinini bulur
fn find_python_dir(temp_dir: &Path, target: &Target) -> Result<PathBuf, String> {
    // python-build-standalone genelde python/ dizini açar
    let candidates = vec!["python", "install", ""];

    for candidate in candidates {
        let path = temp_dir.join(candidate);
        if candidate.is_empty() {
            // Doğrudan temp_dir'de python binary var mı kontrol et
            let python_bin = match target.os {
                OperatingSystem::Windows => temp_dir.join("python.exe"),
                _ => temp_dir.join("bin").join("python3"),
            };
            if python_bin.exists() {
                return Ok(temp_dir.to_path_buf());
            }
        } else {
            let python_bin = match target.os {
                OperatingSystem::Windows => path.join("python.exe"),
                _ => path.join("bin").join("python3"),
            };
            if python_bin.exists() {
                return Ok(path);
            }
        }
    }

    // Walkdir ile python binary ara
    for entry in walkdir::WalkDir::new(temp_dir)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let name = entry.file_name().to_str().unwrap_or("");
        let is_python = match target.os {
            OperatingSystem::Windows => name == "python.exe",
            _ => name == "python3" || name == "python",
        };

        if is_python {
            if let Some(parent) = entry.path().parent() {
                // Binary'nin parent'ının parent'ını al (bin/python3 -> python_dir)
                if parent.file_name().map_or(false, |n| n == "bin") {
                    if let Some(python_root) = parent.parent() {
                        return Ok(python_root.to_path_buf());
                    }
                } else {
                    return Ok(parent.to_path_buf());
                }
            }
        }
    }

    Err(format!(
        "Couldn't find the Python binary: in {}",
        temp_dir.display()
    ))
}

/// Wheel dosyalarını açar
fn extract_wheels(packages_dir: &Path, _target: &Target) -> Result<(), String> {
    let entries: Vec<PathBuf> = std::fs::read_dir(packages_dir)
        .map_err(|e| format!("Couldn't read the directory: {}", e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |ext| ext == "whl"))
        .collect();

    for wheel_path in entries {
        println!("    Opening Wheel: {}", wheel_path.file_name().unwrap().to_str().unwrap_or("?"));

        let file = fs::File::open(&wheel_path)
            .map_err(|e| format!("Couldn't open Wheel: {}", e))?;

        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("Couldn't read Wheel: {}", e))?;

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| format!("Couldn't read Wheel entry: {}", e))?;

            let name = file.name().to_string();
            let output_path = packages_dir.join(&name);

            if file.is_dir() {
                fs::create_dir_all(&output_path)
                    .map_err(|e| format!("Couldn't create the directory: {}", e))?;
            } else {
                if let Some(parent) = output_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Couldn't create the directory: {}", e))?;
                }

                let mut buffer = Vec::new();
                std::io::Read::read_to_end(&mut file, &mut buffer)
                    .map_err(|e| format!("Couldn't read the file: {}", e))?;

                fs::write(&output_path, buffer)
                    .map_err(|e| format!("Couldn't write the file: {}", e))?;
            }
        }
    }

    Ok(())
}
fn calculate_dir_size(dir: &Path) -> u64 {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst)
        .map_err(|e| format!("Couldn't create the source directory: {}", e))?;

    for entry in fs::read_dir(src).map_err(|e| format!("Couldn't read the source directory: {}", e))? {
        let entry = entry.map_err(|e| format!("Couldn't read the entry: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("Couldn't copy the file: {}", e))?;
        }
    }

    Ok(())
}