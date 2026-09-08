//! Native launcher'ın bundle'daki Python runtime'ına doğru eriştiğini
//! doğrulayan uçtan uca test. Sahte bir bundle kurar (gerçek python3'ü
//! sahte "runtime" olarak symlink'ler), launcher'ı FARKLI bir CWD'den
//! çalıştırır ve entry/env/exit-code/chdir davranışını kontrol eder.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const TEST_PY: &str = r#"
import os, sys

print("ENTRY_OK")
print("MARKER_" + ("OK" if os.path.isfile("marker.txt") else "FAIL"))
print("ENV_" + ("OK" if os.environ.get("PYTHONNOUSERSITE") == "1" else "FAIL"))
print("PP_" + ("OK" if "lib" in os.environ.get("PYTHONPATH", "") else "FAIL"))
print("PH_" + ("True" if "PYTHONHOME" in os.environ else "False"))
print("ARG_" + (sys.argv[1] if len(sys.argv) > 1 else "FAIL"))
sys.exit(7)
"#;

#[test]
#[cfg(unix)]
fn native_launcher_end_to_end() {
    // python3 yoksa testi atla (CI pragmatizmi)
    let probe = Command::new("python3").arg("--version").output();
    if !matches!(&probe, Ok(o) if o.status.success()) {
        eprintln!("python3 bulunamadı — test atlandı");
        return;
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // 1) Launcher'ı derle
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let build = Command::new(&cargo)
        .current_dir(&manifest_dir)
        .args(["build", "--release", "--bin", "pypack-launcher"])
        .output()
        .expect("cargo build");
    assert!(
        build.status.success(),
        "launcher derlenemedi:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let built_launcher = manifest_dir.join("target/release/pypack-launcher");

    // 2) Sahte bundle kur
    let tmp = tempfile::tempdir().expect("tempdir");
    let bundle = tmp.path().join("demoapp");
    fs::create_dir_all(bundle.join("app")).unwrap();
    fs::create_dir_all(bundle.join("lib")).unwrap();
    fs::create_dir_all(bundle.join("python").join("bin")).unwrap();

    // Gerçek python3'ü sahte "runtime" olarak bağla
    let real = String::from_utf8_lossy(
        &Command::new("python3")
            .arg("-c")
            .arg("import sys; print(sys.executable)")
            .output()
            .unwrap()
            .stdout,
    )
    .trim()
    .to_string();
    std::os::unix::fs::symlink(&real, bundle.join("python/bin/python3")).unwrap();

    fs::write(bundle.join("app/main.py"), TEST_PY).unwrap();
    fs::write(bundle.join("marker.txt"), "").unwrap();
    fs::write(
        bundle.join("pypack.manifest"),
        "entry=app/main.py\npython_home=python\nchdir=true\nset_pythonhome=false\n",
    )
    .unwrap();

    // 3) Launcher'ı bundle'a koy, FARKLI bir dizinden çalıştır
    let exe = bundle.join("demoapp");
    fs::copy(&built_launcher, &exe).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(&exe).unwrap().permissions();
        p.set_mode(0o755);
        fs::set_permissions(&exe, p).unwrap();
    }

    let elsewhere = tmp.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();

    let out = Command::new(&exe)
        .current_dir(&elsewhere) // CWD ≠ bundle dizini — critical test
        .env_remove("PYTHONHOME")
        .arg("--test-arg")
        .output()
        .expect("launcher çalıştırılamadı");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    println!("launcher stdout:\n{}", stdout);

    assert!(stdout.contains("ENTRY_OK"), "entry çalışmadı. stderr: {}", stderr);
    assert!(stdout.contains("MARKER_OK"), "chdir=false davranışı — CWD launcher dizinine göre çözülmedi: {}", stdout);
    assert!(stdout.contains("ENV_OK"), "env değişkenleri set edilmedi: {}", stdout);
    assert!(stdout.contains("PP_OK"), "PYTHONPATH bundle lib'i içermiyor: {}", stdout);
    assert!(stdout.contains("PH_False"), "set_pythonhome=false yok sayıldı: {}", stdout);
    assert!(stdout.contains("ARG_--test-arg"), "argümanlar iletilmedi: {}", stdout);
    assert_eq!(out.status.code(), Some(7), "exit code iletilmedi. stderr: {}", stderr);
}