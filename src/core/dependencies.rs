use crate::utils::validators::DependencyValidator;

pub struct SystemDependencies;

#[derive(Clone, Debug)]
pub enum DistroPackageManager {
    Apt,
    Dnf,
    Pacman,
    Zypper,
}

impl SystemDependencies {
    pub fn verify_all() -> DependencyCheckResult {
        let mut missing = DependencyValidator::get_missing_dependencies();
        let wine32_supported = Self::check_wine32_support();

        // Critical check: win32 support required for Office 2016
        if !wine32_supported {
            missing.push("wine32-support");
        }

        DependencyCheckResult {
            all_present: missing.is_empty(),
            missing,
            wine_version: Self::get_wine_version(),
        }
    }

    fn get_wine_version() -> Option<String> {
        match std::process::Command::new("wine").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                Some(version.trim().to_string())
            }
            _ => None,
        }
    }

    pub fn check_wine32_support() -> bool {
        if which::which("wine32").is_ok() {
            return true;
        }

        let wine32_indicators = [
            "/usr/bin/wine32",
            "/usr/lib/wine/wine32",
            "/usr/lib/wine/i386-unix",
            "/usr/lib32/wine",
            "/usr/lib/wine32",
        ];

        for path in &wine32_indicators {
            if std::path::Path::new(path).exists() {
                return true;
            }
        }

        if which::which("wine").is_ok() && which::which("wine64").is_err() {
            return true;
        }

        false
    }

    pub fn detect_package_manager() -> Option<DistroPackageManager> {
        if which::which("apt-get").is_ok() || which::which("apt").is_ok() {
            Some(DistroPackageManager::Apt)
        } else if which::which("dnf").is_ok() {
            Some(DistroPackageManager::Dnf)
        } else if which::which("pacman").is_ok() {
            Some(DistroPackageManager::Pacman)
        } else if which::which("zypper").is_ok() {
            Some(DistroPackageManager::Zypper)
        } else {
            None
        }
    }

    pub fn get_install_instructions(dep: &str) -> String {
        match dep {
            "wine" => Self::get_wine_install_instructions(),
            "wine32-support" => Self::get_wine32_install_instructions(),
            "winetricks" => Self::get_winetricks_install_instructions(),
            "cabextract" => Self::get_cabextract_install_instructions(),
            "winbind" => Self::get_winbind_install_instructions(),
            _ => "See your distribution documentation".to_string(),
        }
    }

    fn get_wine_install_instructions() -> String {
        match Self::detect_package_manager() {
            Some(DistroPackageManager::Apt) => 
                "sudo dpkg --add-architecture i386 && sudo apt update && sudo apt install -y wine64 wine32".to_string(),
            Some(DistroPackageManager::Dnf) => 
                "sudo dnf install -y wine".to_string(),
            Some(DistroPackageManager::Pacman) => 
                "sudo pacman -S --needed wine".to_string(),
            Some(DistroPackageManager::Zypper) => 
                "sudo zypper install -y wine".to_string(),
            None =>
                "Install Wine for your distribution".to_string(),
        }
    }

    fn get_wine32_install_instructions() -> String {
        match Self::detect_package_manager() {
            Some(DistroPackageManager::Apt) => {
                "sudo dpkg --add-architecture i386 && sudo apt update && sudo apt install -y wine32"
                    .to_string()
            }
            Some(DistroPackageManager::Dnf) => "sudo dnf install -y wine.i686".to_string(),
            Some(DistroPackageManager::Pacman) => {
                "sudo pacman -S --needed lib32-wine (o instalar wine-staging desde AUR)".to_string()
            }
            Some(DistroPackageManager::Zypper) => "sudo zypper install -y wine-32bit".to_string(),
            None => "Install wine32 for your architecture (i386/x86)".to_string(),
        }
    }

    fn get_winetricks_install_instructions() -> String {
        match Self::detect_package_manager() {
            Some(DistroPackageManager::Apt) => "sudo apt install -y winetricks".to_string(),
            Some(DistroPackageManager::Dnf) => "sudo dnf install -y winetricks".to_string(),
            Some(DistroPackageManager::Pacman) => {
                "sudo pacman -S --needed winetricks (o desde AUR)".to_string()
            }
            Some(DistroPackageManager::Zypper) => "sudo zypper install -y winetricks".to_string(),
            None => "Install Winetricks from https://github.com/Winetricks/winetricks".to_string(),
        }
    }

    fn get_cabextract_install_instructions() -> String {
        match Self::detect_package_manager() {
            Some(DistroPackageManager::Apt) => "sudo apt install -y cabextract".to_string(),
            Some(DistroPackageManager::Dnf) => "sudo dnf install -y cabextract".to_string(),
            Some(DistroPackageManager::Pacman) => "sudo pacman -S --needed cabextract".to_string(),
            Some(DistroPackageManager::Zypper) => "sudo zypper install -y cabextract".to_string(),
            None => "Install cabextract for your distribution".to_string(),
        }
    }

    fn get_winbind_install_instructions() -> String {
        match Self::detect_package_manager() {
            Some(DistroPackageManager::Apt) => "sudo apt install -y winbind".to_string(),
            Some(DistroPackageManager::Dnf) => "sudo dnf install -y samba-winbind".to_string(),
            Some(DistroPackageManager::Pacman) => "sudo pacman -S --needed samba".to_string(),
            Some(DistroPackageManager::Zypper) => {
                "sudo zypper install -y samba-winbind".to_string()
            }
            None => "Install winbind for your distribution".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DependencyCheckResult {
    pub all_present: bool,
    pub missing: Vec<&'static str>,
    pub wine_version: Option<String>,
}
