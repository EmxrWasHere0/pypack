use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Python script'inin import ettiği modülleri analiz eder
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    /// Kullanıcı script'inin import ettiği tüm modüller
    pub user_imports: HashSet<String>,
    /// Bu modüllerin dosya yolları
    pub module_paths: Vec<PathBuf>,
    /// Pip ile kurulması gereken paketler (stdlib olmayan)
    pub external_packages: Vec<String>,
    /// Python versiyonu
    pub python_version: String,
    /// Python executable yolu
    pub python_executable: PathBuf,
}

/// Yerel Python'ı bulur
pub fn find_python() -> Result<PathBuf, String> {
    // Önce python3 dene, sonra python
    for cmd in &["python3", "python"] {
        if let Ok(output) = Command::new(cmd)
            .args(["-c", "import sys; print(sys.executable)"])
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Ok(PathBuf::from(path));
                }
            }
        }
    }

    // which kullanarak dene
    if let Ok(path) = which::which("python3") {
        return Ok(path);
    }
    if let Ok(path) = which::which("python") {
        return Ok(path);
    }

    Err("Couldn't find python. Set the PYTHON_PATH environment variable.".to_string())
}

/// Python versiyonunu alır
pub fn get_python_version(python: &Path) -> Result<String, String> {
    let output = Command::new(python)
        .args(["-c", "import platform; print(platform.python_version())"])
        .output()
        .map_err(|e| format!("Couldn't get Python version: {}", e))?;

    if !output.status.success() {
        return Err("Couldn't get Python version".to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn analyze_script(python: &Path, script_path: &Path) -> Result<AnalysisResult, String> {
    let python_version = get_python_version(python)?;

    let finder_script = r#"
import ast, importlib.util, json, os, sys

script_path = os.path.abspath(sys.argv[1])
script_dir = os.path.dirname(script_path)
sys.path.insert(0, script_dir)

with open(script_path, encoding="utf-8") as f:
    tree = ast.parse(f.read(), filename=script_path)

imports = set()
for node in ast.walk(tree):
    if isinstance(node, ast.Import):
        for alias in node.names:
            imports.add(alias.name.split(".")[0])
    elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
        imports.add(node.module.split(".")[0])

stdlib = getattr(sys, "stdlib_module_names", set())
site_packages = [os.path.abspath(p) for p in sys.path if "site-packages" in p]

modules = {}
local = []
external = {}

for name in sorted(imports):
    if name in stdlib or name in ("__main__", "__future__"):
        continue
    try:
        spec = importlib.util.find_spec(name)
    except (ImportError, ValueError):
        spec = None
    origin = spec.origin if (spec is not None and spec.origin) else None

    if origin is None:
        # Kurulu olmayan paket - yine de harici kabul et, pip indirsin
        external[name] = None
    else:
        modules[name] = origin
        abs_origin = os.path.abspath(origin)
        if abs_origin.startswith(script_dir):
            local.append(name)
        elif any(abs_origin.startswith(d) for d in site_packages):
            external[name] = origin

print(json.dumps({
    "all_imports": sorted(imports),
    "modules": modules,
    "local": local,
    "external": external,
}))
"#;

    let output = Command::new(python)
        .arg("-c")
        .arg(finder_script)
        .arg(script_path.to_str().ok_or("Script path is not valid UTF-8")?)
        .output()
        .map_err(|e| format!("Analiz hatası: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Python analizi başarısız: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    // Çıktıdaki JSON satırını bul (sondan doğru, içteki '{' hatalı parse ederdi)
    let json_str = stdout
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with('{'))
        .ok_or("Python analiz çıktısında JSON bulunamadı")?;

    let data: serde_json::Value = serde_json::from_str(json_str.trim())
        .map_err(|e| format!("JSON parse hatası: {}", e))?;

    let mut user_imports = HashSet::new();
    let mut module_paths = Vec::new();
    let mut external_packages = Vec::new();

    if let Some(imports) = data["all_imports"].as_array() {
        for imp in imports {
            if let Some(s) = imp.as_str() {
                user_imports.insert(s.to_string());
            }
        }
    }

    if let Some(modules) = data["modules"].as_object() {
        for (name, path) in modules {
            if let Some(path_str) = path.as_str() {
                let pb = PathBuf::from(path_str);
                if pb.exists() {
                    module_paths.push(pb);
                }
                user_imports.insert(name.clone());
            }
        }
    }

    if let Some(external) = data["external"].as_object() {
        for name in external.keys() {
            external_packages.push(name.clone());
        }
    }

    external_packages.sort();
    external_packages.dedup();

    Ok(AnalysisResult {
        user_imports,
        module_paths,
        external_packages,
        python_version,
        python_executable: python.to_path_buf(),
    })
}

/// Pip paketlerini requirements.txt formatına dönüştürür
pub fn generate_requirements(analysis: &AnalysisResult) -> String {
    let mut reqs = String::new();
    for pkg in &analysis.external_packages {
        reqs.push_str(&format!("{}\n", pkg));
    }
    reqs
}

/// Script'in çalışma dizinindeki tüm .py dosyalarını bulur
pub fn find_local_python_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for entry in walkdir::WalkDir::new(dir)
        .max_depth(10)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().extension().map_or(false, |e| e == "py") {
            files.push(entry.path().to_path_buf());
        }
    }

    files
}