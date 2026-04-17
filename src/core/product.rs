use regex::Regex;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProductType {
    Office2016,
    Project2016,
    Visio2016,
    Generic,
}

#[derive(Clone, Debug)]
pub struct ProductInfo {
    pub product_type: ProductType,
    pub display_name: &'static str,
    pub prefix_suffix: &'static str,
    pub exe_name: &'static str,
    pub parent_folder: &'static str,
    pub ww_patterns: &'static [&'static str],
}

pub(crate) const PRODUCTS: &[ProductInfo] = &[
    ProductInfo {
        product_type: ProductType::Office2016,
        display_name: "Microsoft Office",
        prefix_suffix: "office",
        exe_name: "WINWORD.EXE",
        parent_folder: "Microsoft Office",
        ww_patterns: &[
            "proplus*.ww",
            "standard*.ww",
            "professional*.ww",
            "homebusiness*.ww",
            "homestudent*.ww",
            "office*.ww",
            "office64*.ww",
        ],
    },
    ProductInfo {
        product_type: ProductType::Project2016,
        display_name: "Microsoft Project",
        prefix_suffix: "project",
        exe_name: "WINPROJ.EXE",
        parent_folder: "Microsoft Office",
        ww_patterns: &["prjstd.ww", "prjpro.ww", "prjstd*.ww", "prjpro*.ww"],
    },
    ProductInfo {
        product_type: ProductType::Visio2016,
        display_name: "Microsoft Visio",
        prefix_suffix: "visio",
        exe_name: "VISIO.EXE",
        parent_folder: "Microsoft Office",
        ww_patterns: &["visstd.ww", "vispro.ww", "visstd*.ww", "vispro*.ww"],
    },
    ProductInfo {
        product_type: ProductType::Generic,
        display_name: "Generic Application",
        prefix_suffix: "generic",
        exe_name: "",
        parent_folder: "",
        ww_patterns: &[],
    },
];

impl ProductType {
    pub fn to_info(&self) -> &'static ProductInfo {
        PRODUCTS
            .iter()
            .find(|p| p.product_type == *self)
            .unwrap_or(&PRODUCTS[PRODUCTS.len() - 1])
    }

    /// Parse a ProductType from a string. Kept for backward compatibility.
    pub fn from_str(s: &str) -> Self {
        Self::parse_from_str(s)
    }

    fn parse_from_str(s: &str) -> Self {
        match s {
            "Office2016" => ProductType::Office2016,
            "Project2016" => ProductType::Project2016,
            "Visio2016" => ProductType::Visio2016,
            "Generic" | "Unknown" => ProductType::Generic,
            _ => ProductType::Generic,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ProductType::Office2016 => "Office2016",
            ProductType::Project2016 => "Project2016",
            ProductType::Visio2016 => "Visio2016",
            ProductType::Generic => "Generic",
        }
    }

    pub fn is_office_family(&self) -> bool {
        matches!(
            self,
            ProductType::Office2016 | ProductType::Project2016 | ProductType::Visio2016
        )
    }
}

impl std::fmt::Display for ProductType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ProductType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse_from_str(s))
    }
}

pub struct ProductDetector;

impl ProductDetector {
    /// Detects the product type by analyzing .ww folders in the directory
    pub fn detect_from_directory(path: &Path) -> ProductType {
        let ww_folders = Self::find_ww_folders(path);
        Self::detect_from_ww_folders(&ww_folders)
    }

    /// Finds all folders ending with .ww in the directory
    pub fn find_ww_folders(path: &Path) -> Vec<String> {
        let mut folders = Vec::new();

        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.to_lowercase().ends_with(".ww") {
                                folders.push(name.to_lowercase());
                            }
                        }
                    }
                }
            }
        }

        folders
    }

    fn detect_from_ww_folders(folders: &[String]) -> ProductType {
        if folders.is_empty() {
            return ProductType::Generic;
        }

        for product in PRODUCTS {
            for pattern in product.ww_patterns {
                if !pattern.is_empty() && Self::matches_any_pattern(folders, pattern) {
                    return product.product_type;
                }
            }
        }

        ProductType::Generic
    }

    /// Checks if any folder matches a glob pattern
    fn matches_any_pattern(folders: &[String], pattern: &str) -> bool {
        let pattern_lower = pattern.to_lowercase();
        // Use glob::Pattern from the crate already in Cargo.toml instead of manual matching
        let Ok(glob_pattern) = glob::Pattern::new(&pattern_lower) else {
            return false;
        };
        folders.iter().any(|folder| glob_pattern.matches(folder))
    }

    pub fn detect_product_and_version(path: &Path) -> DetectedProduct {
        let ww_folders = Self::find_ww_folders(path);

        if ww_folders.is_empty() {
            return DetectedProduct {
                product_type: ProductType::Generic,
                version_year: 0,
                edition: "Generic".to_string(),
            };
        }

        if let Some(detected) = Self::detect_from_setup_xml(path, &ww_folders) {
            return detected;
        }

        let product_type = Self::detect_from_ww_folders(&ww_folders);

        if product_type == ProductType::Generic {
            return DetectedProduct {
                product_type: ProductType::Generic,
                version_year: 0,
                edition: "Generic".to_string(),
            };
        }

        let version_year = Self::extract_version_from_folders(path, &ww_folders, &product_type);

        DetectedProduct {
            product_type,
            version_year,
            edition: Self::extract_edition(&ww_folders),
        }
    }

    fn detect_from_setup_xml(base_path: &Path, folders: &[String]) -> Option<DetectedProduct> {
        for folder in folders {
            let ww_path = base_path.join(folder);
            let setup_xml_path = ww_path.join("Setup.xml");
            if !setup_xml_path.exists() {
                continue;
            }

            let content = fs::read_to_string(&setup_xml_path).ok()?;
            let version_year = Self::parse_product_code_version(&content)?;

            let product_type = Self::infer_product_type_from_folder(folder);
            let edition = Self::extract_edition_from_folder_and_xml(folder, &content);

            return Some(DetectedProduct {
                product_type,
                version_year,
                edition,
            });
        }

        None
    }

    fn infer_product_type_from_folder(folder: &str) -> ProductType {
        let lower = folder.to_lowercase();
        if lower.contains("prj") {
            ProductType::Project2016
        } else if lower.contains("vis") {
            ProductType::Visio2016
        } else {
            ProductType::Office2016
        }
    }

    fn extract_edition_from_folder_and_xml(folder: &str, content: &str) -> String {
        if let Some(edition) = Self::extract_edition_from_setup_id(content) {
            return edition;
        }
        Self::extract_edition_from_folder_name(folder)
    }

    fn extract_edition_from_setup_id(content: &str) -> Option<String> {
        lazy_static::lazy_static! {
            static ref SETUP_ID_RE: Regex = Regex::new(
                r#"Setup Id="([^"]+)""#
            ).unwrap();
        }

        let caps = SETUP_ID_RE.captures(content)?;
        let id = caps.get(1)?.as_str().to_lowercase();

        let edition = match id.as_str() {
            s if s.contains("proplus") => "ProPlus",
            s if s.contains("professional") && !s.contains("project") => "Professional",
            s if s.contains("standard") && !s.contains("project") && !s.contains("visio") => {
                "Standard"
            }
            s if s.contains("homebusiness") => "HomeBusiness",
            s if s.contains("homestudent") => "HomeStudent",
            s if s.contains("prjpro") || s.contains("project") && s.contains("pro") => {
                "Professional"
            }
            s if s.contains("prjstd") => "Standard",
            s if s.contains("vispro") => "Professional",
            s if s.contains("visstd") => "Standard",
            _ => return None,
        };

        Some(edition.to_string())
    }

    fn extract_edition_from_folder_name(folder: &str) -> String {
        Self::extract_edition(&[folder.to_lowercase()])
    }

    /// Extracts the version year from .ww folders
    /// First attempts to parse ProductCode from Setup.xml, then uses fallback
    fn extract_version_from_folders(
        base_path: &Path,
        folders: &[String],
        _product: &ProductType,
    ) -> u16 {
        // Attempt to extract version from Setup.xml in each .ww folder
        for folder in folders {
            let ww_path = base_path.join(folder);
            if let Some(version) = Self::extract_version_from_setup_xml(&ww_path) {
                return version;
            }
        }

        // Fallback to folder name detection
        Self::fallback_version_detection(folders)
    }

    /// Extracts version from Setup.xml by parsing ProductCode GUID
    /// Format: {90XX0000-XXXX-XXXX-XXXX-XXXXXXXXXXXX} where XX = version
    fn extract_version_from_setup_xml(ww_folder: &Path) -> Option<u16> {
        let setup_xml_path = ww_folder.join("Setup.xml");
        if !setup_xml_path.exists() {
            return None;
        }

        let content = fs::read_to_string(&setup_xml_path).ok()?;
        Self::parse_product_code_version(&content)
    }

    /// Parses ProductCode GUID from Setup.xml content
    /// Mapping: 90140000 -> 2010, 90150000 -> 2013, 90160000 -> 2016, 90180000 -> 2019
    fn parse_product_code_version(content: &str) -> Option<u16> {
        // Regex to capture version code from ProductCode
        lazy_static::lazy_static! {
            static ref PRODUCTCODE_RE: Regex = Regex::new(
                r#"ProductCode="\{90(\d{2})0000-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}\}"#
            ).unwrap();
        }

        if let Some(caps) = PRODUCTCODE_RE.captures(content) {
            if let Some(version_match) = caps.get(1) {
                let version_num = version_match.as_str().parse::<u16>().ok()?;
                // Map internal version to release year
                return match version_num {
                    14 => Some(2010),
                    15 => Some(2013),
                    16 => Some(2016),
                    18 => Some(2019),
                    19 => Some(2021),
                    21 => Some(2024),
                    _ => Some(2000 + version_num), // Versiones futuras
                };
            }
        }

        None
    }

    /// Fallback detection using folder names
    fn fallback_version_detection(folders: &[String]) -> u16 {
        // Office 2016+ has office64*.ww folders
        let has_office64 = folders.iter().any(|f| f.starts_with("office64"));
        if has_office64 {
            return 2016;
        }

        // office*.ww without 64-bit may be 2010, 2013, or 2016
        let has_office = folders.iter().any(|f| f.starts_with("office"));
        if has_office {
            return 2013; // Conservative default
        }

        // proplus.ww and similar without office* -> likely 2010/2013
        2016 // Default
    }

    /// Extracts product edition from .ww folders
    fn extract_edition(folders: &[String]) -> String {
        for folder in folders {
            match folder.as_str() {
                f if f.contains("proplus") => return "ProPlus".to_string(),
                f if f.contains("standard") => return "Standard".to_string(),
                f if f.contains("prjpro") => return "Professional".to_string(),
                f if f.contains("prjstd") => return "Standard".to_string(),
                f if f.contains("vispro") => return "Professional".to_string(),
                f if f.contains("visstd") => return "Standard".to_string(),
                _ => {}
            }
        }
        "Standard".to_string()
    }
}

/// Detected product with dynamic version information parsed from setup files
#[derive(Clone, Debug)]
pub struct DetectedProduct {
    pub product_type: ProductType,
    pub version_year: u16,
    pub edition: String,
}

impl DetectedProduct {
    pub fn get_display_name(&self) -> String {
        match self.product_type {
            ProductType::Office2016 => {
                format!("Microsoft Office {} {}", self.version_year, self.edition)
            }
            ProductType::Project2016 => {
                format!("Microsoft Project {}", self.version_year)
            }
            ProductType::Visio2016 => {
                format!("Microsoft Visio {}", self.version_year)
            }
            ProductType::Generic => "Generic Application".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_office_2010_proplus() {
        let temp_dir = TempDir::new().unwrap();
        let proplus_ww = temp_dir.path().join("proplus.ww");
        fs::create_dir(&proplus_ww).unwrap();

        let setup_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Setup Id="ProPlus" Type="Product" ProductCode="{90140000-0011-0000-1000-0000000FF1CE}">
</Setup>"#;
        fs::write(proplus_ww.join("Setup.xml"), setup_xml).unwrap();

        let detected = ProductDetector::detect_product_and_version(temp_dir.path());

        assert_eq!(detected.product_type, ProductType::Office2016);
        assert_eq!(detected.version_year, 2010);
        assert_eq!(detected.edition, "ProPlus");
    }

    #[test]
    fn test_detect_office_2013_proplus() {
        let temp_dir = TempDir::new().unwrap();
        let proplus_ww = temp_dir.path().join("proplus.ww");
        fs::create_dir(&proplus_ww).unwrap();

        let setup_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Setup Id="ProPlus" Type="Product" ProductCode="{90150000-0011-0000-1000-0000000FF1CE}">
</Setup>"#;
        fs::write(proplus_ww.join("Setup.xml"), setup_xml).unwrap();

        let detected = ProductDetector::detect_product_and_version(temp_dir.path());

        assert_eq!(detected.product_type, ProductType::Office2016);
        assert_eq!(detected.version_year, 2013);
        assert_eq!(detected.edition, "ProPlus");
    }

    #[test]
    fn test_detect_office_2016_proplus() {
        let temp_dir = TempDir::new().unwrap();
        let proplus_ww = temp_dir.path().join("proplus.ww");
        fs::create_dir(&proplus_ww).unwrap();

        let setup_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Setup Id="ProPlus" Type="Product" ProductCode="{90160000-0011-0000-0000-0000000FF1CE}">
</Setup>"#;
        fs::write(proplus_ww.join("Setup.xml"), setup_xml).unwrap();

        let detected = ProductDetector::detect_product_and_version(temp_dir.path());

        assert_eq!(detected.product_type, ProductType::Office2016);
        assert_eq!(detected.version_year, 2016);
        assert_eq!(detected.edition, "ProPlus");
    }

    // Test ProductCode parsing function
    #[test]
    fn test_parse_product_code_version() {
        // Office 2010: 9014 -> 2010
        let content_2010 = r#"ProductCode="{90140000-0011-0000-1000-0000000FF1CE}""#;
        assert_eq!(
            ProductDetector::parse_product_code_version(content_2010),
            Some(2010)
        );

        // Office 2013: 9015 -> 2013
        let content_2013 = r#"ProductCode="{90150000-0011-0000-1000-0000000FF1CE}""#;
        assert_eq!(
            ProductDetector::parse_product_code_version(content_2013),
            Some(2013)
        );

        // Office 2016: 9016 -> 2016
        let content_2016 = r#"ProductCode="{90160000-0011-0000-1000-0000000FF1CE}""#;
        assert_eq!(
            ProductDetector::parse_product_code_version(content_2016),
            Some(2016)
        );

        // Office 2019: 9018 -> 2019
        let content_2019 = r#"ProductCode="{90180000-0011-0000-1000-0000000FF1CE}""#;
        assert_eq!(
            ProductDetector::parse_product_code_version(content_2019),
            Some(2019)
        );

        // Office 2021: 9019 -> 2021
        let content_2021 = r#"ProductCode="{90190000-0011-0000-1000-0000000FF1CE}""#;
        assert_eq!(
            ProductDetector::parse_product_code_version(content_2021),
            Some(2021)
        );

        // Office 2024: 9021 -> 2024
        let content_2024 = r#"ProductCode="{90210000-0011-0000-1000-0000000FF1CE}""#;
        assert_eq!(
            ProductDetector::parse_product_code_version(content_2024),
            Some(2024)
        );
    }

    #[test]
    fn test_office_2016_not_detected_as_2013() {
        let temp_dir = TempDir::new().unwrap();
        let proplus_ww = temp_dir.path().join("proplus.ww");
        fs::create_dir(&proplus_ww).unwrap();

        let setup_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Setup Id="ProPlus" Type="Product" ProductCode="{90160000-0011-0000-0000-0000000FF1CE}">
</Setup>"#;
        fs::write(proplus_ww.join("Setup.xml"), setup_xml).unwrap();

        let detected = ProductDetector::detect_product_and_version(temp_dir.path());

        assert_eq!(
            detected.version_year, 2016,
            "Office 2016 was incorrectly detected as {} instead of 2016",
            detected.version_year
        );
    }

    #[test]
    fn test_product_type_from_str() {
        assert_eq!(ProductType::from_str("Office2016"), ProductType::Office2016);
        assert_eq!(
            ProductType::from_str("Project2016"),
            ProductType::Project2016
        );
        assert_eq!(ProductType::from_str("Visio2016"), ProductType::Visio2016);
        assert_eq!(ProductType::from_str("Unknown"), ProductType::Generic);
        assert_eq!(ProductType::from_str("Generic"), ProductType::Generic);
        assert_eq!(ProductType::from_str("SomethingElse"), ProductType::Generic);
    }

    #[test]
    fn test_matches_pattern_exact() {
        let folders = vec!["prjpro.ww".to_string(), "office.en-us.ww".to_string()];
        assert!(ProductDetector::matches_any_pattern(&folders, "prjpro.ww"));
        assert!(!ProductDetector::matches_any_pattern(&folders, "vispro.ww"));
    }

    #[test]
    fn test_matches_pattern_wildcard() {
        let folders = vec![
            "office64.en-us.ww".to_string(),
            "office.en-us.ww".to_string(),
        ];
        assert!(ProductDetector::matches_any_pattern(&folders, "office*.ww"));
        assert!(ProductDetector::matches_any_pattern(
            &folders,
            "office64*.ww"
        ));
    }

    #[test]
    fn test_detect_from_ww_folders() {
        let office_folders = vec![
            "office64.en-us.ww".to_string(),
            "office.en-us.ww".to_string(),
        ];
        assert_eq!(
            ProductDetector::detect_from_ww_folders(&office_folders),
            ProductType::Office2016
        );

        let project_folders = vec!["prjpro.ww".to_string()];
        assert_eq!(
            ProductDetector::detect_from_ww_folders(&project_folders),
            ProductType::Project2016
        );

        let visio_folders = vec!["vispro.ww".to_string()];
        assert_eq!(
            ProductDetector::detect_from_ww_folders(&visio_folders),
            ProductType::Visio2016
        );

        let empty: Vec<String> = vec![];
        assert_eq!(
            ProductDetector::detect_from_ww_folders(&empty),
            ProductType::Generic
        );

        let unrecognized_folders = vec!["random.ww".to_string()];
        assert_eq!(
            ProductDetector::detect_from_ww_folders(&unrecognized_folders),
            ProductType::Generic
        );
    }

    #[test]
    fn test_detect_generic_from_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let detected = ProductDetector::detect_product_and_version(temp_dir.path());
        assert_eq!(detected.product_type, ProductType::Generic);
        assert_eq!(detected.version_year, 0);
        assert_eq!(detected.edition, "Generic");
    }

    #[test]
    fn test_is_office_family() {
        assert!(ProductType::Office2016.is_office_family());
        assert!(ProductType::Project2016.is_office_family());
        assert!(ProductType::Visio2016.is_office_family());
        assert!(!ProductType::Generic.is_office_family());
    }

    #[test]
    fn test_detect_proplus_ww_real_structure() {
        let temp_dir = TempDir::new().unwrap();
        let proplus_ww = temp_dir.path().join("proplus.ww");
        fs::create_dir(&proplus_ww).unwrap();

        let setup_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Setup Id="ProPlus" Type="Product" ProductCode="{90160000-0011-0000-0000-0000000FF1CE}">
</Setup>"#;
        fs::write(proplus_ww.join("Setup.xml"), setup_xml).unwrap();

        let detected = ProductDetector::detect_product_and_version(temp_dir.path());

        assert_eq!(detected.product_type, ProductType::Office2016);
        assert_eq!(detected.version_year, 2016);
        assert_eq!(detected.edition, "ProPlus");
    }

    #[test]
    fn test_detect_project_from_prjpro_ww() {
        let temp_dir = TempDir::new().unwrap();
        let prjpro_ww = temp_dir.path().join("prjpro.ww");
        fs::create_dir(&prjpro_ww).unwrap();

        let setup_xml = r#"<Setup Id="PrjPro" Type="Product" ProductCode="{90160000-003B-0000-0000-0000000FF1CE}"></Setup>"#;
        fs::write(prjpro_ww.join("Setup.xml"), setup_xml).unwrap();

        let detected = ProductDetector::detect_product_and_version(temp_dir.path());

        assert_eq!(detected.product_type, ProductType::Project2016);
        assert_eq!(detected.version_year, 2016);
    }

    #[test]
    fn test_detect_visio_from_vispro_ww() {
        let temp_dir = TempDir::new().unwrap();
        let vispro_ww = temp_dir.path().join("vispro.ww");
        fs::create_dir(&vispro_ww).unwrap();

        let setup_xml = r#"<Setup Id="VisPro" Type="Product" ProductCode="{90160000-0051-0000-0000-0000000FF1CE}"></Setup>"#;
        fs::write(vispro_ww.join("Setup.xml"), setup_xml).unwrap();

        let detected = ProductDetector::detect_product_and_version(temp_dir.path());

        assert_eq!(detected.product_type, ProductType::Visio2016);
        assert_eq!(detected.version_year, 2016);
    }

    #[test]
    fn test_detect_from_ww_folders_with_proplus() {
        let folders = vec!["proplus.ww".to_string()];
        assert_eq!(
            ProductDetector::detect_from_ww_folders(&folders),
            ProductType::Office2016
        );
    }

    #[test]
    fn test_infer_product_type_from_folder() {
        assert_eq!(
            ProductDetector::infer_product_type_from_folder("proplus.ww"),
            ProductType::Office2016
        );
        assert_eq!(
            ProductDetector::infer_product_type_from_folder("standard.ww"),
            ProductType::Office2016
        );
        assert_eq!(
            ProductDetector::infer_product_type_from_folder("prjpro.ww"),
            ProductType::Project2016
        );
        assert_eq!(
            ProductDetector::infer_product_type_from_folder("prjstd.ww"),
            ProductType::Project2016
        );
        assert_eq!(
            ProductDetector::infer_product_type_from_folder("vispro.ww"),
            ProductType::Visio2016
        );
        assert_eq!(
            ProductDetector::infer_product_type_from_folder("visstd.ww"),
            ProductType::Visio2016
        );
        assert_eq!(
            ProductDetector::infer_product_type_from_folder("homebusiness.ww"),
            ProductType::Office2016
        );
    }
}
