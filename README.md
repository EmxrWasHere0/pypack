# 🐍 PyPack

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

> Bundle Python code, its dependencies, and the Python runtime into a single
> self-contained, cross-platform package — no Python installation required
> on the target machine.

**Türkçe dokümantasyon için → [README.tr.md](README.tr.md)**

---

## Table of Contents

- [How It Works](#-how-it-works)
- [Features](#-features)
- [Requirements](#-requirements)
- [Installation](#-installation)
- [Usage](#-usage)
- [Output Structure](#-output-structure)
- [Example](#-example)
- [Known Limitations](#️-known-limitations)
- [Contributing](#-contributing)
- [License](#-license)
- [Acknowledgments](#-acknowledgments)

---

## 🔍 How It Works

1. **Analyzes** your Python script using AST-based static import analysis —
   your code is *never executed* during analysis, so scripts with side
   effects (servers, file operations) are safe.
2. **Detects** external dependencies and separates them from standard library
   and local modules.
3. **Downloads** a standalone Python runtime
   ([python-build-standalone](https://github.com/indygreg/python-build-standalone))
   for each target platform.
4. **Downloads** the required packages (with transitive dependencies) for
   each platform using `pip download`.
5. **Generates** platform-appropriate launchers and a per-bundle README.

```
your_script.py ──► PyPack ──► dist/app_linux-x86_64/
                             dist/app_macos-aarch64/
                             dist/app_windows-x86_64/
```

---

## ✨ Features

- 🔍 **Static analysis** — AST-based import detection; the script is never run
- 📦 **Full runtime bundling** — ships a complete standalone Python interpreter
- 🌍 **Cross-compilation** — build for Linux, macOS and Windows *from one machine*
- 🏗️ **Multi-architecture** — x86_64 and aarch64 support
- 🚀 **Multiple launchers** — shell script, `.bat`, `.ps1` and Rust source
- 💾 **Local caching** — runtimes are cached in `~/.cache/pypack/` for fast rebuilds

---

## 📋 Requirements

**Build machine:**

| Requirement | Purpose |
|-------------|---------|
| [Rust](https://rustup.rs/) (2021 edition+) | Compiling pypack itself |
| Python 3 + `pip` | Import analysis and dependency downloads |
| Internet connection | First-time runtime & package downloads |

**Target machine:** nothing — the bundle is fully self-contained.

---

## 🔧 Installation

**From source:**

```bash
git clone https://github.com/KULLANICI_ADIN/pypack.git
cd pypack
cargo build --release
# Binary: target/release/pypack
```

**Global install (recommended):**

```bash
cargo install --path .
# Now usable from anywhere: pypack <script>
```

---

## 🚀 Usage

```bash
# Default targets (linux-x86_64, macos-aarch64, windows-x86_64)
pypack myapp.py

# Custom name and output directory
pypack myapp.py --name "MyApp" -o dist

# Specific targets only
pypack myapp.py -t linux-x86_64 -t windows-x86_64

# Choose the Python version (full version required)
pypack myapp.py -p 3.11.7

# Analyze only — no downloads, no bundling
pypack myapp.py --dry-run

# Every supported platform
pypack myapp.py --all-platforms
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `<SCRIPT>` | Python script to bundle | *(required)* |
| `-o, --output <DIR>` | Output directory | `dist` |
| `-n, --name <NAME>` | Application name | script filename |
| `-t, --targets <LIST>` | Comma-separated targets | `linux-x86_64,macos-aarch64,windows-x86_64` |
| `-p, --python-version <VER>` | Full Python version | `3.11.7` |
| `--no-clean` | Keep previous output | off |
| `--dry-run` | Analyze only | off |
| `--all-platforms` | Build all supported targets | off |

### Supported targets

`linux-x86_64` · `linux-aarch64` · `macos-x86_64` · `macos-aarch64` · `windows-x86_64`

---

## 📂 Output Structure

```
dist/
└── myapp_linux-x86_64/
    ├── myapp              # Unix launcher (executable)
    ├── myapp.bat          # Windows launcher
    ├── myapp.ps1          # PowerShell launcher
    ├── python/            # Standalone Python runtime
    ├── app/               # Your application code
    │   └── myapp.py
    ├── lib/               # Bundled dependencies
    ├── launcher_src/      # Optional Rust launcher source
    │   └── main.rs
    └── README.md          # Per-bundle usage notes
```

**Running a bundle:**

```bash
# Linux / macOS
./myapp [args...]

# Windows
myapp.bat [args...]
```

Command-line arguments are passed through to your script.

---

## 🎯 Example

```python
# app.py
import requests

def main():
    r = requests.get("https://api.github.com")
    print(f"Status: {r.status_code}")

if __name__ == "__main__":
    main()
```

```bash
$ pypack app.py --name github-checker

╔════════════════════════════════════════════════════════╗
║  PyPack - Python Code Bundler                          ║
╚════════════════════════════════════════════════════════╝

  Script: app.py
  App name: github-checker
  Python: 3.11.7
  Targets: ["linux-x86_64", "macos-aarch64", "windows-x86_64"]

linux-x86_64 hedefi için paketleniyor...
  [1/5] Python runtime indiriliyor...
  ...
```

PyPack detects `requests`, resolves and downloads it for each platform,
bundles the runtime, and produces ready-to-run packages.

---

## ⚠️ Known Limitations

- **Source-only packages** — pypack uses `pip download --only-binary=:all:`,
  so packages without prebuilt wheels for the target platform will fail.
- **Bundle size** — the full Python runtime is included (~50–200 MB per bundle).
- **First run** — runtimes are downloaded on first use; later runs use the cache.

---

## 🤝 Contributing

**Note that the comments in this project is written in Turkish.
Consider translating them beforehand.**

1. Fork the repository
2. Create your branch: `git checkout -b feature/amazing-feature`
3. Commit: `git commit -m 'Add amazing feature'`
4. Push: `git push origin feature/amazing-feature`
5. Open a Pull Request

Bug reports and suggestions are welcome via [Issues](../../issues).

---

## 📄 License

Copyright (C) 2026 EmxrDev

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version. See the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- [python-build-standalone](https://github.com/indygreg/python-build-standalone) — portable Python runtimes