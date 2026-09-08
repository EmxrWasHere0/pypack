#![allow(dead_code)]

mod analyzer;
mod bundler;
mod downloader;
mod launcher;
mod platform;

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "pypack",
    version = "0.1.0",
    about = "Packages the code with Python runtime and dependencies.",
    long_about = "PyPack turns Python codes into executable files.\n\nPython runtime, libraries and dependencies\nget packaged into a single executable file."
)]
struct Args {
    /// Python script to be packaged
    #[arg(value_name = "SCRIPT")]
    script: PathBuf,

    /// Output directory
    #[arg(short, long, default_value = "dist")]
    output: PathBuf,

    /// Application name
    #[arg(short, long)]
    name: Option<String>,

    /// Target platforms (separated with comma)
    /// e.g.: linux-x86_64,macos-aarch64,windows-x86_64
    #[arg(
        short,
        long,
        value_delimiter = ',',
        default_value = "linux-x86_64,macos-aarch64,windows-x86_64"
    )]
    targets: Vec<String>,

    #[arg(short, long, default_value = "3.11.7")]
    python_version: String,

    /// Clean the current output
    /// Don't clean the old output (default: cleans)
    #[arg(long)]
    no_clean: bool,

    /// Only shows the analysis (don't do packaging)
    #[arg(long)]
    dry_run: bool,

    /// Packaging for all supported platforms
    #[arg(long)]
    all_platforms: bool,

    /// Skip native executable generation (only script launcher)
    #[arg(long)]
    no_native: bool,
}

fn main() {
    let args = Args::parse();

    // Banner
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║  PyPack - Python Code Bundler                          ║");
    println!("║  Turn your codes into single executables.              ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!();

    // Script dosyasını kontrol et
    if !args.script.exists() {
        eprintln!("ERROR: Couldn't find the script file: {}", args.script.display());
        std::process::exit(1);
    }

    // Uygulama adını belirle
    let app_name = args.name.clone().unwrap_or_else(|| {
        args.script
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("app")
            .to_string()
    });

    // Hedef platformları parse et
    let targets = if args.all_platforms {
        platform::all_targets()
    } else {
        let mut parsed = Vec::new();
        for target_str in &args.targets {
            match platform::Target::parse(target_str) {
                Ok(t) => parsed.push(t),
                Err(e) => {
                    eprintln!("ERROR: {}", e);
                    std::process::exit(1);
                }
            }
        }
        parsed
    };

    if targets.is_empty() {
        eprintln!("ERROR: No valid target platform given.");
        std::process::exit(1);
    }

    println!("  Script            : {}", args.script.display());
    println!("  Application name  : {}", app_name);
    println!("  Python            : {}", args.python_version);
    println!("  Targets           : {:?}", targets.iter().map(|t| t.to_string()).collect::<Vec<_>>());
    println!("  Output            : {}", args.output.display());
    println!();

    // Dry run - sadece analiz
    if args.dry_run {
        match dry_run_analysis(&args) {
            Ok(()) => {
                println!("\n✓ Analysis completed (dry-run mode)");
            }
            Err(e) => {
                eprintln!("\n✗ Analysis error: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // Bundle konfigürasyonu
    let config = bundler::BundleConfig {
        app_name: app_name.clone(),
        script_path: args.script.clone(),
        output_dir: args.output.clone(),
        targets,
        python_version: args.python_version.clone(),
        clean: !args.no_clean,
        try_native: !args.no_native,
    };

    // Paketleme
    match bundler::bundle_all(&config) {
        Ok(results) => {
            println!("\n╔════════════════════════════════════════════════════════╗");
            println!("║  ✓ Packaging has been Completed!                       ║");
            println!("╚════════════════════════════════════════════════════════╝");
            println!();

            for result in &results {
                let size_mb = result.total_size as f64 / (1024.0 * 1024.0);
                println!("  {} → {} ({:.1} MB)",
                    result.target,
                    result.output_path.display(),
                    size_mb
                );
            }

            println!("\n  Usage instructions are included in the README.md file of each package.");
        }
        Err(e) => {
            eprintln!("\n✗ Packaging error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Dry-run: Sadece analiz yapar, dosya oluşturmaz
fn dry_run_analysis(args: &Args) -> Result<(), String> {
    println!("─── Analysis Mode (Dry Run) ───\n");

    // Python bul
    let python = analyzer::find_python()?;
    println!("  Python             : {}", python.display());

    // Versiyon al
    let version = analyzer::get_python_version(&python)?;
    println!("  Python Version     : {}", version);

    // Script analizi
    let analysis = analyzer::analyze_script(&python, &args.script)?;

    println!("\n  ── Import Analysis ──\n");

    // Tüm importları göster
    let mut imports: Vec<_> = analysis.user_imports.iter().collect();
    imports.sort();

    println!("  Found modules ({}):", imports.len());
    for imp in imports {
        println!("    • {}", imp);
    }

    // Harici paketler
    if !analysis.external_packages.is_empty() {
        println!("\n  External packages ({}):", analysis.external_packages.len());
        for pkg in &analysis.external_packages {
            println!("    ◦ {}", pkg);
        }
    }

    // Modül yolları
    println!("\n  Module file paths: {} file", analysis.module_paths.len());

    // Requirements dosyası önizleme
    let requirements = analyzer::generate_requirements(&analysis);
    println!("\n  ── requirements.txt preview ──\n{}", requirements);

    Ok(())
}