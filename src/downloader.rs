use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::platform::Target;

const RELEASE_TAG: &str = "20240107";
const GITHUB_REPO: &str = "indygreg/python-build-standalone";

/// Cache dizinini alır
pub fn cache_dir() -> PathBuf {
    let base = dirs::cache_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from(".cache"));
    base.join("pypack")
}

/// Release metadata'sını çeker (yerel olarak cache'lenir)
///
/// GitHub API'si kimliksiz (token'sız) isteklerde saatte 60 çağrıya izin
/// verir; bu yüzden her tag için metadata yalnızca bir kez indirilir.
fn fetch_release(release_tag: &str) -> Result<serde_json::Value, String> {
    let releases_cache = cache_dir().join("releases");
    fs::create_dir_all(&releases_cache)
        .map_err(|e| format!("Couldn't create release cache: {}", e))?;

    let cache_path = releases_cache.join(format!("{}.json", release_tag));

    // Cache'te varsa oradan oku
    if cache_path.exists() {
        if let Ok(content) = fs::read_to_string(&cache_path) {
            if let Ok(value) = serde_json::from_str(&content) {
                return Ok(value);
            }
        }
    }

    println!("  ↓ Getting Release info: {} ({})", release_tag, GITHUB_REPO);

    let api_url = format!(
        "https://api.github.com/repos/{}/releases/tags/{}",
        GITHUB_REPO, release_tag
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    // GitHub API, User-Agent olmadan isteği reddeder
    let response = client
        .get(&api_url)
        .header("User-Agent", "pypack")
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub API error: HTTP {} — tag: {}\n(API rate limit might be exceeded; \
             clean ~/.cache/pypack/releases/ and retry few minutes later.)",
            response.status(),
            release_tag
        ));
    }

    let release: serde_json::Value = response
        .json()
        .map_err(|e| format!("GitHub API response couldn't be parsed: {}", e))?;

    // Cache'e yaz
    if let Ok(serialized) = serde_json::to_string_pretty(&release) {
        let _ = fs::write(&cache_path, serialized);
    }

    Ok(release)
}

/// Release asset'leri arasında hedef platforma uyanı bulur.
///
/// Adlandırma şeması release'ler arasında değiştiği için sabit URL
/// kurmuyoruz; gerçek asset adını API'den okuyoruz.
///
/// Bilinen desenler:
///   cpython-3.11.7+20240107-x86_64-unknown-linux-gnu-install_only.tar.gz
///   cpython-3.11.7+20240107-x86_64-pc-windows-msvc-SHARED-install_only.tar.gz
///   cpython-3.11.7+20240107-aarch64-apple-darwin-install_only.tar.gz
///
/// Returns: (download_url, asset_name)
/// Release asset'leri arasında hedef platforma uyanı bulur.
/// Returns: (download_url, asset_name)
fn find_asset(
    release: &serde_json::Value,
    target: &Target,
    python_version: &str,
    release_tag: &str,
) -> Result<(String, String), String> {
    let prefix = format!(
        "cpython-{}+{}-{}-",
        python_version,
        release_tag,
        target.triplet()
    );

    let assets = release["assets"]
        .as_array()
        .ok_or("Couldn't read release asset list")?;

    for asset in assets {
        let name = match asset["name"].as_str() {
            Some(n) => n,
            None => continue,
        };

        // 1) Doğru prefix ile başlıyor mu?
        //    rest: "install_only.tar.gz" | "shared-install_only.tar.gz" | diğer
        let rest = match name.strip_prefix(&prefix) {
            Some(r) => r,
            None => continue,
        };

        // 2) Tam kurulum (install_only) paketi mi?
        //    strip_suffix → Some(varyant) veya None (pgo-full, debug, static vb.)
        let variant = match rest
            .strip_suffix("install_only.tar.gz")
            .or_else(|| rest.strip_suffix("install_only.zip"))
        {
            Some(v) => v, // "" (Linux/macOS) veya "shared-" (Windows)
            None => continue,
        };

        // 3) Kabul edilebilir varyant mı? (boş veya "shared")
        let variant = variant.trim_end_matches('-');
        if !variant.is_empty() && variant != "shared" {
            continue;
        }

        let url = asset["browser_download_url"]
            .as_str()
            .ok_or("Couldn't read asset download URL")?
            .to_string();

        return Ok((url, name.to_string()));
    }

    // Bulunamadı — yardımcı hata mesajı üret
    let version_prefix = format!("cpython-{}+{}", python_version, release_tag);
    let mut available: Vec<&str> = assets
        .iter()
        .filter_map(|a| a["name"].as_str())
        .filter(|n| {
            n.starts_with(&version_prefix)
                && (n.ends_with("-install_only.tar.gz") || n.ends_with("-install_only.zip"))
        })
        .collect();
    available.sort();

    if available.is_empty() {
        Err(format!(
            "Python {} does not exist on this release (tag: {}).\nFor version list: \
             https://github.com/{}/releases/tag/{}",
            python_version, release_tag, GITHUB_REPO, release_tag
        ))
    } else {
        Err(format!(
            "Couldn't find any asset for {} target. Available assets on this Python version:\n  {}",
            target,
            available.join("\n  ")
        ))
    }
}

/// Python standalone build'i indirir (asset keşfi API üzerinden yapılır)
pub fn download_python(target: &Target, python_version: &str) -> Result<PathBuf, String> {
    let cache = cache_dir();
    fs::create_dir_all(&cache)
        .map_err(|e| format!("Couldn't create cache directory: {}", e))?;

    let release = fetch_release(RELEASE_TAG)?;
    let (url, asset_name) = find_asset(&release, target, python_version, RELEASE_TAG)?;

    // Cache anahtarı olarak GERÇEK asset adını kullanıyoruz — böylece
    // uzantı da doğru olur (.tar.gz) ve extract aşaması doğru dallanır
    let archive_path = cache.join(&asset_name);

    if archive_path.exists() {
        println!("  ✓ Using from cache: {}", asset_name);
        return Ok(archive_path);
    }

    println!("  ↓ Downloading: {}", url);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let response = client
        .get(&url)
        .header("User-Agent", "pypack")
        .send()
        .map_err(|e| format!("Download error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Download failed: HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .map_err(|e| format!("Couldn't read contents: {}", e))?;

    fs::write(&archive_path, &bytes)
        .map_err(|e| format!("Couldn't write the file: {}", e))?;

    println!(
        "  ✓ Downloaded: {} ({} MB)",
        asset_name,
        bytes.len() / (1024 * 1024)
    );

    Ok(archive_path)
}

/// Arşivi belirtilen dizine açar
pub fn extract_archive(archive_path: &Path, output_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Couldn't create output directory: {}", e))?;

    let ext = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "zip" => extract_zip(archive_path, output_dir),
        "gz" => extract_tar_gz(archive_path, output_dir),
        _ => Err(format!("Unsupported archive format: {}", ext)),
    }
}

/// ZIP dosyası açar
fn extract_zip(archive_path: &Path, output_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path)
        .map_err(|e| format!("Couldn't open ZIP archive: {}", e))?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Couldn't read ZIP archive: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Couldn't read ZIP entry: {}", e))?;

        let name = file.name().to_string();
        let output_path = output_dir.join(&name);

        if file.is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|e| format!("Couldn't create directory {}: {}", name, e))?;
        } else {
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Couldn't create directory: {}", e))?;
            }

            let mut output_file = fs::File::create(&output_path)
                .map_err(|e| format!("Couldn't create file {}: {}", name, e))?;

            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)
                .map_err(|e| format!("Couldn't read file: {}", e))?;

            output_file
                .write_all(&buffer)
                .map_err(|e| format!("Couldn't write file: {}", e))?;
        }
    }

    Ok(())
}

/// tar.gz dosyası açar
fn extract_tar_gz(archive_path: &Path, output_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path)
        .map_err(|e| format!("Couldn't open TAR.GZ archive: {}", e))?;

    let gz_decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz_decoder);

    archive
        .unpack(output_dir)
        .map_err(|e| format!("TAR.GZ unarchiving error: {}", e))?;

    Ok(())
}

/// Python paketlerini pip download ile hedef platform için indirir
pub fn download_packages(
    python: &Path,
    packages: &[String],
    target: &Target,
    python_version: &str,
    output_dir: &Path,
) -> Result<(), String> {
    if packages.is_empty() {
        return Ok(());
    }

    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Couldn't create package directory: {}", e))?;

    let platform_str = match target.triplet().as_str() {
        "x86_64-unknown-linux-gnu" => "manylinux2014_x86_64",
        "aarch64-unknown-linux-gnu" => "manylinux2014_aarch64",
        "x86_64-apple-darwin" => "macosx_10_9_x86_64",
        "aarch64-apple-darwin" => "macosx_11_0_arm64",
        "x86_64-pc-windows-msvc" => "win_amd64",
        "aarch64-pc-windows-msvc" => "win_arm64",
        t => return Err(format!("Unknown platform: {}", t)),
    };

    // pip "3.11" formatını kabul eder; "3.11.7" gibi tam sürümü kırpalım
    let major_minor = python_version
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".");

    println!("  ↓ Downloading packages: {:?}", packages);
    println!("    Platform: {}, Python: {}", platform_str, major_minor);

    let mut cmd = std::process::Command::new(python);
    cmd.args([
        "-m",
        "pip",
        "download",
        "--dest",
        output_dir.to_str().unwrap_or("."),
        "--platform",
        platform_str,
        "--only-binary=:all:",
        "--python-version",
        major_minor.as_str(),
    ]);

    for pkg in packages {
        cmd.arg(pkg);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Couldn't run `pip download`: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("`pip download` failed: {}", stderr));
    }

    println!("  ✓ Packages have been downloaded!");
    Ok(())
}