use crate::core::product::ProductType;
use std::path::PathBuf;

const VINEOFFICE_PREFIX: &str = ".vineoffice_";
const GENERIC_ID_LEN: usize = 8;

pub struct PrefixNaming;

impl PrefixNaming {
    #[allow(dead_code)]
    pub fn prefix_name_with_version(product: &ProductType, version_year: u16) -> String {
        let info = product.to_info();
        format!(
            "{}{}{}",
            VINEOFFICE_PREFIX, info.prefix_suffix, version_year
        )
    }

    pub fn generate_unique_prefix_name(product: &ProductType) -> String {
        Self::generate_unique_prefix_name_with_version(product, 2016)
    }

    pub fn generate_unique_prefix_name_with_version(
        product: &ProductType,
        version_year: u16,
    ) -> String {
        let info = product.to_info();

        if *product == ProductType::Generic {
            let random_id = Self::generate_random_id();
            return format!("{}{}", VINEOFFICE_PREFIX, random_id);
        }

        format!(
            "{}{}{}",
            VINEOFFICE_PREFIX, info.prefix_suffix, version_year
        )
    }

    /// Generates a unique 8-char hex ID using uuid::Uuid::new_v4().
    /// uuid is already in Cargo.toml — stable, cryptographically secure,
    /// unlike DefaultHasher which is not stable across Rust versions.
    fn generate_random_id() -> String {
        uuid::Uuid::new_v4()
            .simple()
            .to_string()
            .chars()
            .take(GENERIC_ID_LEN)
            .collect()
    }

    pub fn extract_product_from_prefix_name(name: &str) -> Option<ProductType> {
        Self::extract_product_and_version(name).map(|(product, _)| product)
    }

    pub fn extract_product_and_version(name: &str) -> Option<(ProductType, u16)> {
        let name_to_check = if name.starts_with('.') {
            name
        } else if name.starts_with("vineoffice") {
            name
        } else if name.starts_with(VINEOFFICE_PREFIX.trim_start_matches('.')) {
            name
        } else {
            return None;
        };

        let full_name = if name_to_check.starts_with('.') {
            name_to_check.to_string()
        } else {
            format!(".{}", name_to_check)
        };

        let suffix_part = if full_name.starts_with(VINEOFFICE_PREFIX) {
            &full_name[VINEOFFICE_PREFIX.len()..]
        } else {
            &full_name[1..]
        };

        let base_suffix = if let Some(underscore_pos) = suffix_part.rfind('_') {
            let after_underscore = &suffix_part[underscore_pos + 1..];
            if after_underscore.parse::<u32>().is_ok() {
                &suffix_part[..underscore_pos]
            } else {
                suffix_part
            }
        } else {
            suffix_part
        };

        if let Some(result) = Self::parse_known_product_suffix(base_suffix) {
            return Some(result);
        }

        if Self::is_generic_id(base_suffix) {
            return Some((ProductType::Generic, 0));
        }

        None
    }

    fn parse_known_product_suffix(suffix: &str) -> Option<(ProductType, u16)> {
        for product in &[
            ProductType::Office2016,
            ProductType::Project2016,
            ProductType::Visio2016,
        ] {
            let info = product.to_info();
            if suffix.starts_with(info.prefix_suffix) {
                let version_str = &suffix[info.prefix_suffix.len()..];
                if let Ok(version) = version_str.parse::<u16>() {
                    return Some((*product, version));
                }
            }
        }
        None
    }

    fn is_generic_id(suffix: &str) -> bool {
        suffix.len() == GENERIC_ID_LEN && suffix.chars().all(|c| c.is_ascii_hexdigit())
    }

    pub fn all_glob_patterns() -> Vec<String> {
        use crate::core::product::PRODUCTS;

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let home_str = home.display();

        // Derive from PRODUCTS — adding a new product auto-creates its pattern (OCP)
        let mut patterns: Vec<String> = PRODUCTS
            .iter()
            .filter(|p| p.product_type != ProductType::Generic)
            .map(|p| format!("{}/{}{}*", home_str, VINEOFFICE_PREFIX, p.prefix_suffix))
            .collect();

        // Pattern for Generic prefixes (8-char hex UUID suffix)
        patterns.push(format!("{}/{}????????", home_str, VINEOFFICE_PREFIX));
        patterns
    }

    pub fn is_managed_prefix(name: &str) -> bool {
        name.starts_with(VINEOFFICE_PREFIX) && name.len() > VINEOFFICE_PREFIX.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_product_from_prefix_name() {
        assert_eq!(
            PrefixNaming::extract_product_from_prefix_name(".vineoffice_office2016"),
            Some(ProductType::Office2016)
        );
        assert_eq!(
            PrefixNaming::extract_product_from_prefix_name(".vineoffice_project2019"),
            Some(ProductType::Project2016)
        );
        assert_eq!(
            PrefixNaming::extract_product_from_prefix_name(".vineoffice_visio2021"),
            Some(ProductType::Visio2016)
        );
    }

    #[test]
    fn test_extract_product_and_version() {
        assert_eq!(
            PrefixNaming::extract_product_and_version(".vineoffice_office2013"),
            Some((ProductType::Office2016, 2013))
        );
        assert_eq!(
            PrefixNaming::extract_product_and_version(".vineoffice_office2016"),
            Some((ProductType::Office2016, 2016))
        );
        assert_eq!(
            PrefixNaming::extract_product_and_version(".vineoffice_office2019"),
            Some((ProductType::Office2016, 2019))
        );
        assert_eq!(
            PrefixNaming::extract_product_and_version(".vineoffice_visio2021"),
            Some((ProductType::Visio2016, 2021))
        );
        assert_eq!(
            PrefixNaming::extract_product_and_version(".vineoffice_f9e8d7c6"),
            Some((ProductType::Generic, 0))
        );
    }

    #[test]
    fn test_extract_product_without_prefix_returns_none() {
        assert_eq!(
            PrefixNaming::extract_product_and_version("office2016"),
            None
        );
    }

    #[test]
    fn test_extract_product_with_various_formats() {
        assert_eq!(
            PrefixNaming::extract_product_and_version(".vineoffice_office2016"),
            Some((ProductType::Office2016, 2016))
        );
        assert_eq!(
            PrefixNaming::extract_product_and_version("vineoffice_office2016"),
            Some((ProductType::Office2016, 2016))
        );
        assert_eq!(
            PrefixNaming::extract_product_and_version("office2016"),
            None
        );
    }

    #[test]
    fn test_generic_prefix_uniqueness() {
        let name1 = PrefixNaming::generate_unique_prefix_name(&ProductType::Generic);
        let name2 = PrefixNaming::generate_unique_prefix_name(&ProductType::Generic);
        assert_ne!(name1, name2);
    }
}
