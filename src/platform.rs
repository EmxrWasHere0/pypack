use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Architecture {
    X86_64,
    Aarch64,
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Architecture::X86_64 => write!(f, "x86_64"),
            Architecture::Aarch64 => write!(f, "aarch64"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatingSystem {
    Linux,
    Macos,
    Windows,
}

impl fmt::Display for OperatingSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OperatingSystem::Linux => write!(f, "linux"),
            OperatingSystem::Macos => write!(f, "macos"),
            OperatingSystem::Windows => write!(f, "windows"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Target {
    pub os: OperatingSystem,
    pub arch: Architecture,
}

impl Target {
    pub fn linux_x86_64() -> Self {
        Target { os: OperatingSystem::Linux, arch: Architecture::X86_64 }
    }

    pub fn linux_aarch64() -> Self {
        Target { os: OperatingSystem::Linux, arch: Architecture::Aarch64 }
    }

    pub fn macos_x86_64() -> Self {
        Target { os: OperatingSystem::Macos, arch: Architecture::X86_64 }
    }

    pub fn macos_aarch64() -> Self {
        Target { os: OperatingSystem::Macos, arch: Architecture::Aarch64 }
    }

    pub fn windows_x86_64() -> Self {
        Target { os: OperatingSystem::Windows, arch: Architecture::X86_64 }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid target format: '{}'. Use 'os-arch' (e.g., 'linux-x86_64')", s));
        }

        let os = match parts[0] {
            "linux" => OperatingSystem::Linux,
            "macos" | "darwin" => OperatingSystem::Macos,
            "windows" | "win" => OperatingSystem::Windows,
            _ => return Err(format!("Unknown OS: '{}'. Use linux, macos, or windows", parts[0])),
        };

        let arch = match parts[1] {
            "x86_64" | "x64" | "amd64" => Architecture::X86_64,
            "aarch64" | "arm64" => Architecture::Aarch64,
            _ => return Err(format!("Unknown architecture: '{}'. Use x86_64 or aarch64", parts[1])),
        };

        Ok(Target { os, arch })
    }

    pub fn triplet(&self) -> String {
        match (self.os, self.arch) {
            (OperatingSystem::Linux, Architecture::X86_64) => "x86_64-unknown-linux-gnu".to_string(),
            (OperatingSystem::Linux, Architecture::Aarch64) => "aarch64-unknown-linux-gnu".to_string(),
            (OperatingSystem::Macos, Architecture::X86_64) => "x86_64-apple-darwin".to_string(),
            (OperatingSystem::Macos, Architecture::Aarch64) => "aarch64-apple-darwin".to_string(),
            (OperatingSystem::Windows, Architecture::X86_64) => "x86_64-pc-windows-msvc".to_string(),
            (OperatingSystem::Windows, Architecture::Aarch64) => "aarch64-pc-windows-msvc".to_string(),
        }
    }
    pub fn host() -> Option<Target> {
        let os = match std::env::consts::OS {
            "linux" => OperatingSystem::Linux,
            "macos" => OperatingSystem::Macos,
            "windows" => OperatingSystem::Windows,
            _ => return None,
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => Architecture::X86_64,
            "aarch64" => Architecture::Aarch64,
            _ => return None,
        };
        Some(Target { os, arch })
    }

    pub fn is_host(&self) -> bool {
        Target::host().map(|h| h == *self).unwrap_or(false)
    }

    /// Cross-compile'da kullanılacak Rust triplet'i.
    /// None → pratik cross toolchain yok (ör. linux→windows-aarch64)
    pub fn rust_cross_triplet(&self) -> Option<&'static str> {
        match (self.os, self.arch) {
            (OperatingSystem::Windows, Architecture::X86_64) => Some("x86_64-pc-windows-gnu"),
            (OperatingSystem::Windows, Architecture::Aarch64) => None,
            (OperatingSystem::Linux, Architecture::X86_64) => Some("x86_64-unknown-linux-gnu"),
            (OperatingSystem::Linux, Architecture::Aarch64) => Some("aarch64-unknown-linux-gnu"),
            (OperatingSystem::Macos, Architecture::X86_64) => Some("x86_64-apple-darwin"),
            (OperatingSystem::Macos, Architecture::Aarch64) => Some("aarch64-apple-darwin"),
        }
    }

    /// Hedef OS'e göre çalıştırılabilir dosya adı: app.exe / app
    pub fn exe_file_name(&self, app_name: &str) -> String {
        match self.os {
            OperatingSystem::Windows => format!("{}.exe", app_name),
            _ => app_name.to_string(),
        }
    }

    /// Hedef OS'e göre binary adı (launcher build cache'i için)
    pub fn bin_file_name(&self, name: &str) -> String {
        match self.os {
            OperatingSystem::Windows => format!("{}.exe", name),
            _ => name.to_string(),
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.os, self.arch)
    }
}

/// Desteklenen hedefler
pub fn default_targets() -> Vec<Target> {
    vec![
        Target::linux_x86_64(),
        Target::macos_aarch64(),
        Target::windows_x86_64(),
    ]
}

pub fn all_targets() -> Vec<Target> {
    vec![
        Target::linux_x86_64(),
        Target::linux_aarch64(),
        Target::macos_x86_64(),
        Target::macos_aarch64(),
        Target::windows_x86_64(),
    ]
}