use crate::core::prefix_naming::PrefixNaming;
use crate::core::product::ProductType;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::warn;

pub const WINE_DEFAULT_ARCH: &str = "win32";

#[derive(Debug)]
pub struct WinePrefixManager {
    prefix_path: PathBuf,
    arch: String,
}

impl WinePrefixManager {
    pub fn new(prefix_path: impl AsRef<Path>, arch: &str) -> Self {
        Self {
            prefix_path: prefix_path.as_ref().to_path_buf(),
            arch: arch.to_string(),
        }
    }

    pub fn default_office_prefix() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self::new(home.join(".vineoffice_office2016"), WINE_DEFAULT_ARCH)
    }

    pub fn for_product(product: ProductType) -> Self {
        let prefix_name = PrefixNaming::generate_unique_prefix_name(&product);
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self::new(home.join(&prefix_name), WINE_DEFAULT_ARCH)
    }

    pub fn for_product_with_version(product: ProductType, version_year: u16) -> Self {
        let prefix_name =
            PrefixNaming::generate_unique_prefix_name_with_version(&product, version_year);
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self::new(home.join(&prefix_name), WINE_DEFAULT_ARCH)
    }

    pub fn from_existing_path(path: PathBuf) -> Result<Self, PrefixError> {
        let detected_arch = Self::detect_arch_from_prefix(&path);

        match detected_arch.as_deref() {
            Some("win64") => Err(PrefixError::IncompatibleArch(
                "Office 2016 requires win32 prefix. This prefix is win64.".into(),
            )),
            Some("win32") | None => Ok(Self::new(path, WINE_DEFAULT_ARCH)),
            _ => Err(PrefixError::UnknownArch),
        }
    }

    fn detect_arch_from_prefix(path: &Path) -> Option<String> {
        let system_reg = path.join("system.reg");
        let content = std::fs::read_to_string(system_reg).ok()?;

        if content.contains("#arch=win64") {
            Some("win64".to_string())
        } else if content.contains("#arch=win32") {
            Some("win32".to_string())
        } else {
            None
        }
    }

    pub fn detect_product(&self) -> ProductType {
        let from_name = self
            .prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|name| PrefixNaming::extract_product_from_prefix_name(name));

        if let Some(product) = from_name {
            return product;
        }

        for product in [
            ProductType::Visio2016,
            ProductType::Project2016,
            ProductType::Office2016,
        ] {
            if self.is_product_installed(product) {
                return product;
            }
        }

        warn!(
            "Could not detect product in '{}'. Using fallback Generic.",
            self.prefix_path.display()
        );
        ProductType::Generic
    }

    pub fn get_prefix_path(&self) -> &Path {
        &self.prefix_path
    }

    pub fn get_arch(&self) -> &str {
        &self.arch
    }

    pub fn detect_office_folder(&self) -> Option<String> {
        let office_base = self
            .prefix_path
            .join("drive_c")
            .join("Program Files")
            .join("Microsoft Office");

        if let Ok(entries) = std::fs::read_dir(&office_base) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.starts_with("Office") && name.len() > 6 {
                                let version_part = &name[6..];
                                if version_part.chars().all(|c| c.is_ascii_digit()) {
                                    return Some(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    pub fn get_office_installation_path(&self) -> PathBuf {
        let office_folder = self.detect_office_folder().unwrap_or_else(|| {
            warn!("Could not detect Office folder dynamically, using default Office16");
            "Office16".to_string()
        });
        self.prefix_path
            .join("drive_c")
            .join("Program Files")
            .join("Microsoft Office")
            .join(office_folder)
    }

    pub fn get_product_exe_path(&self, product: ProductType) -> PathBuf {
        if product == ProductType::Generic {
            return self.prefix_path.clone();
        }

        let info = product.to_info();
        // parent_folder is always a single path component (e.g. "Microsoft Office")
        let mut path = self.prefix_path
            .join("drive_c")
            .join("Program Files")
            .join(info.parent_folder);

        if let Some(office_folder) = self.detect_office_folder() {
            path = path.join(office_folder);
        } else {
            path = path.join("Office16");
        }

        path.join(info.exe_name)
    }

    pub fn is_product_installed(&self, product: ProductType) -> bool {
        if product == ProductType::Generic {
            return self.prefix_exists_and_valid();
        }
        self.get_product_exe_path(product).exists()
    }

    fn prefix_exists_and_valid(&self) -> bool {
        let drive_c = self.prefix_path.join("drive_c");
        let system_reg = self.prefix_path.join("system.reg");
        drive_c.exists() && system_reg.exists()
    }

    pub fn get_windows_fonts_path(&self) -> PathBuf {
        self.prefix_path
            .join("drive_c")
            .join("windows")
            .join("Fonts")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_mock_prefix(base: &Path, office_folder: Option<&str>, exes: &[&str]) {
        let mut office_path = base.join("drive_c").join("Program Files").join("Microsoft Office");
        fs::create_dir_all(&office_path).unwrap();

        if let Some(folder) = office_folder {
            office_path = office_path.join(folder);
            fs::create_dir_all(&office_path).unwrap();

            for exe in exes {
                fs::write(office_path.join(exe), "").unwrap();
            }
        }
    }

    #[test]
    fn test_detect_office_folder_finds_office16() {
        let temp_dir = TempDir::new().unwrap();
        create_mock_prefix(temp_dir.path(), Some("Office16"), &["WINWORD.EXE"]);

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let result = prefix.detect_office_folder();

        assert_eq!(result, Some("Office16".to_string()));
    }

    #[test]
    fn test_detect_office_folder_finds_office15() {
        let temp_dir = TempDir::new().unwrap();
        create_mock_prefix(temp_dir.path(), Some("Office15"), &["WINWORD.EXE"]);

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let result = prefix.detect_office_folder();

        assert_eq!(result, Some("Office15".to_string()));
    }

    #[test]
    fn test_detect_office_folder_returns_none_when_no_office_folder() {
        let temp_dir = TempDir::new().unwrap();
        // Create Microsoft Office dir but no OfficeNN subfolder
        let office_base = temp_dir.path().join("drive_c").join("Program Files").join("Microsoft Office");
        fs::create_dir_all(&office_base).unwrap();

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let result = prefix.detect_office_folder();

        assert_eq!(result, None);
    }

    #[test]
    fn test_detect_office_folder_handles_missing_microsoft_office_dir() {
        let temp_dir = TempDir::new().unwrap();
        // Only create drive_C, no Microsoft Office
        fs::create_dir_all(temp_dir.path().join("drive_c")).unwrap();

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let result = prefix.detect_office_folder();

        assert_eq!(result, None);
    }

    #[test]
    fn test_detect_office_folder_ignores_non_numeric_suffix() {
        let temp_dir = TempDir::new().unwrap();
        create_mock_prefix(temp_dir.path(), Some("OfficeFoo"), &["WINWORD.EXE"]);

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let result = prefix.detect_office_folder();

        assert_eq!(result, None);
    }

    #[test]
    fn test_detect_office_folder_finds_first_valid_office_folder() {
        let temp_dir = TempDir::new().unwrap();
        let office_base = temp_dir.path().join("drive_c").join("Program Files").join("Microsoft Office");
        fs::create_dir_all(office_base.join("Office15")).unwrap();
        fs::create_dir_all(office_base.join("Office16")).unwrap();
        fs::create_dir_all(office_base.join("OfficeFoo")).unwrap();

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let result = prefix.detect_office_folder();

        // Should find either Office15 or Office16 (first valid one)
        assert!(result.is_some());
        let folder = result.unwrap();
        assert!(folder == "Office15" || folder == "Office16");
    }

    #[test]
    fn test_get_office_installation_path_uses_detected_folder() {
        let temp_dir = TempDir::new().unwrap();
        create_mock_prefix(temp_dir.path(), Some("Office16"), &["WINWORD.EXE"]);

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let path = prefix.get_office_installation_path();

        assert!(path.to_string_lossy().contains("Office16"));
    }

    #[test]
    fn test_get_office_installation_path_defaults_to_office16_when_not_found() {
        let temp_dir = TempDir::new().unwrap();
        // Create Microsoft Office dir but no OfficeNN subfolder
        let office_base = temp_dir.path().join("drive_c").join("Program Files").join("Microsoft Office");
        fs::create_dir_all(&office_base).unwrap();

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let path = prefix.get_office_installation_path();

        assert!(path.to_string_lossy().contains("Office16"));
    }

    #[test]
    fn test_detect_installed_office_apps_finds_word() {
        let temp_dir = TempDir::new().unwrap();
        create_mock_prefix(temp_dir.path(), Some("Office16"), &["WINWORD.EXE"]);

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let apps = crate::core::desktop_integration::DesktopIntegration::detect_installed_office_apps(&prefix);

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].exe_name, "WINWORD.EXE");
    }

    #[test]
    fn test_detect_installed_office_apps_finds_multiple() {
        let temp_dir = TempDir::new().unwrap();
        create_mock_prefix(temp_dir.path(), Some("Office16"), &[
            "WINWORD.EXE", "EXCEL.EXE", "POWERPNT.EXE"
        ]);

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let apps = crate::core::desktop_integration::DesktopIntegration::detect_installed_office_apps(&prefix);

        assert_eq!(apps.len(), 3);
    }

    #[test]
    fn test_detect_installed_office_apps_returns_empty_when_no_exes() {
        let temp_dir = TempDir::new().unwrap();
        create_mock_prefix(temp_dir.path(), Some("Office16"), &[]);

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let apps = crate::core::desktop_integration::DesktopIntegration::detect_installed_office_apps(&prefix);

        assert!(apps.is_empty());
    }

    #[test]
    fn test_detect_installed_office_apps_with_wrong_office_folder() {
        let temp_dir = TempDir::new().unwrap();
        // Create Office15 but detect_office_folder should find it
        create_mock_prefix(temp_dir.path(), Some("Office15"), &["WINWORD.EXE"]);

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let apps = crate::core::desktop_integration::DesktopIntegration::detect_installed_office_apps(&prefix);

        // Should find Word because detect_office_folder returns Office15
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].exe_name, "WINWORD.EXE");
    }

    #[test]
    fn test_is_product_installed_returns_true_when_exe_exists() {
        let temp_dir = TempDir::new().unwrap();
        create_mock_prefix(temp_dir.path(), Some("Office16"), &["WINWORD.EXE"]);

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let result = prefix.is_product_installed(ProductType::Office2016);

        assert!(result);
    }

    #[test]
    fn test_is_product_installed_returns_false_when_exe_missing() {
        let temp_dir = TempDir::new().unwrap();
        create_mock_prefix(temp_dir.path(), Some("Office16"), &[]);

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let result = prefix.is_product_installed(ProductType::Office2016);

        assert!(!result);
    }

    #[test]
    fn test_get_product_exe_path_constructs_correct_path() {
        let temp_dir = TempDir::new().unwrap();
        create_mock_prefix(temp_dir.path(), Some("Office16"), &["WINWORD.EXE"]);

        let prefix = WinePrefixManager::new(temp_dir.path(), "win32");
        let path = prefix.get_product_exe_path(ProductType::Office2016);

        assert!(path.to_string_lossy().contains("WINWORD.EXE"));
        assert!(path.to_string_lossy().contains("Office16"));
    }

    #[test]
    fn test_detect_product_from_existing_prefix_with_vineoffice_name() {
        let temp_dir = TempDir::new().unwrap();
        // Prefix with vineoffice naming
        let prefix_path = temp_dir.path().join(".vineoffice_office2016");
        create_mock_prefix(&prefix_path, Some("Office16"), &["WINWORD.EXE"]);
        fs::write(prefix_path.join("system.reg"), "#arch=win32").unwrap();

        let prefix = WinePrefixManager::new(&prefix_path, "win32");
        let product = prefix.detect_product();

        assert_eq!(product, ProductType::Office2016);
    }

    #[test]
    fn test_detect_product_from_existing_prefix_without_vineoffice_name() {
        let temp_dir = TempDir::new().unwrap();
        // Prefix WITHOUT vineoffice naming
        let prefix_path = temp_dir.path().join("my-custom-prefix");
        create_mock_prefix(&prefix_path, Some("Office16"), &["WINWORD.EXE"]);
        fs::write(prefix_path.join("system.reg"), "#arch=win32").unwrap();

        let prefix = WinePrefixManager::new(&prefix_path, "win32");
        let product = prefix.detect_product();

        // Should detect from EXE since name doesn't match vineoffice pattern
        assert_eq!(product, ProductType::Office2016);
    }

    #[test]
    fn test_detect_product_returns_generic_when_no_exes() {
        let temp_dir = TempDir::new().unwrap();
        let prefix_path = temp_dir.path().join("some-prefix");
        fs::create_dir_all(prefix_path.join("drive_c")).unwrap();
        fs::write(prefix_path.join("system.reg"), "#arch=win32").unwrap();

        let prefix = WinePrefixManager::new(&prefix_path, "win32");
        let product = prefix.detect_product();

        assert_eq!(product, ProductType::Generic);
    }

    #[test]
    fn test_from_existing_path_validates_arch_and_creates_manager() {
        let temp_dir = TempDir::new().unwrap();
        let prefix_path = temp_dir.path().join("test-prefix");
        fs::create_dir_all(&prefix_path).unwrap();
        fs::write(prefix_path.join("system.reg"), "#arch=win32").unwrap();
        fs::create_dir_all(prefix_path.join("drive_c")).unwrap();

        let result = WinePrefixManager::from_existing_path(prefix_path);

        assert!(result.is_ok());
        let prefix = result.unwrap();
        assert_eq!(prefix.get_arch(), "win32");
    }

    #[test]
    fn test_from_existing_path_rejects_win64() {
        let temp_dir = TempDir::new().unwrap();
        let prefix_path = temp_dir.path().join("test-prefix");
        fs::create_dir_all(&prefix_path).unwrap();
        fs::write(prefix_path.join("system.reg"), "#arch=win64").unwrap();
        fs::create_dir_all(prefix_path.join("drive_c")).unwrap();

        let result = WinePrefixManager::from_existing_path(prefix_path);

        assert!(result.is_err());
        match result.unwrap_err() {
            PrefixError::IncompatibleArch(_) => {}
            _ => panic!("Expected IncompatibleArch error"),
        }
    }
}

#[derive(Debug, Error)]
pub enum PrefixError {
    #[error("Incompatible architecture: {0}")]
    IncompatibleArch(String),
    #[error("Unknown architecture detected in prefix")]
    UnknownArch,
}
