use crate::core::registry::RegistryManager;
use crate::core::wine_prefix::WinePrefixManager;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

const SEGOE_UI_REPO_BASE: &str =
    "https://raw.githubusercontent.com/mrbvrz/segoe-ui-linux/master/font";

pub struct SegoeFont {
    pub filename: &'static str,
    pub registry_name: &'static str,
}

pub static SEGOE_UI_FONTS: &[SegoeFont] = &[
    SegoeFont {
        filename: "segoeui.ttf",
        registry_name: "Segoe UI (TrueType)",
    },
    SegoeFont {
        filename: "segoeuib.ttf",
        registry_name: "Segoe UI Bold (TrueType)",
    },
    SegoeFont {
        filename: "segoeuii.ttf",
        registry_name: "Segoe UI Italic (TrueType)",
    },
    SegoeFont {
        filename: "segoeuil.ttf",
        registry_name: "Segoe UI Light (TrueType)",
    },
    SegoeFont {
        filename: "segoeuisl.ttf",
        registry_name: "Segoe UI Semilight (TrueType)",
    },
    SegoeFont {
        filename: "segoeuiz.ttf",
        registry_name: "Segoe UI Bold Italic (TrueType)",
    },
    SegoeFont {
        filename: "seguibl.ttf",
        registry_name: "Segoe UI Black (TrueType)",
    },
    SegoeFont {
        filename: "seguibli.ttf",
        registry_name: "Segoe UI Black Italic (TrueType)",
    },
    SegoeFont {
        filename: "seguiemj.ttf",
        registry_name: "Segoe UI Emoji (TrueType)",
    },
    SegoeFont {
        filename: "seguihis.ttf",
        registry_name: "Segoe UI Historic (TrueType)",
    },
    SegoeFont {
        filename: "seguili.ttf",
        registry_name: "Segoe UI Semibold Italic (TrueType)",
    },
    SegoeFont {
        filename: "seguisb.ttf",
        registry_name: "Segoe UI Semibold (TrueType)",
    },
    SegoeFont {
        filename: "seguisbi.ttf",
        registry_name: "Segoe UI Semibold Bold Italic (TrueType)",
    },
    SegoeFont {
        filename: "seguisli.ttf",
        registry_name: "Segoe UI Semilight Italic (TrueType)",
    },
    SegoeFont {
        filename: "seguisym.ttf",
        registry_name: "Segoe UI Symbol (TrueType)",
    },
];

pub struct FontManager {
    cache_dir: PathBuf,
    client: reqwest::Client,
}

#[derive(Debug)]
pub enum FontInstallResult {
    Installed,
    Cached,
    Failed(String),
}

impl FontManager {
    pub async fn new() -> anyhow::Result<Self> {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("vineoffice")
            .join("fonts");

        tokio::fs::create_dir_all(&cache_dir).await?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            cache_dir,
            client,
        })
    }

    pub fn get_cache_path(&self, filename: &str) -> PathBuf {
        self.cache_dir.join(filename)
    }

    pub fn is_cached(&self, filename: &str) -> bool {
        let path = self.get_cache_path(filename);
        path.exists() && path.metadata().map(|m| m.len() > 0).unwrap_or(false)
    }

    pub async fn ensure_cached(&self, filename: &str) -> anyhow::Result<PathBuf> {
        if self.is_cached(filename) {
            return Ok(self.get_cache_path(filename));
        }

        let url = format!("{}/{}", SEGOE_UI_REPO_BASE, filename);
        let cache_path = self.get_cache_path(filename);

        let mut last_error = None;
        for attempt in 1..=3 {
            match self.download_file(&url, &cache_path).await {
                Ok(_) => {
                    info!("Downloaded {} to cache", filename);
                    return Ok(cache_path);
                }
                Err(e) => {
                    warn!(
                        "Download attempt {} failed for {}: {}",
                        attempt, filename, e
                    );
                    last_error = Some(e);
                    tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64)).await;
                }
            }
        }

        Err(anyhow::anyhow!(
            "Failed to download {} after 3 attempts: {:?}",
            filename,
            last_error
        ))
    }

    async fn download_file(&self, url: &str, dest: &Path) -> anyhow::Result<()> {
        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("HTTP {}", response.status()));
        }

        let bytes = response.bytes().await?;
        tokio::fs::write(dest, bytes).await?;

        Ok(())
    }

    pub async fn install_to_prefix(
        &self,
        font: &SegoeFont,
        prefix: &WinePrefixManager,
        registry: &RegistryManager<'_>,
    ) -> FontInstallResult {
        let cache_result = self.ensure_cached(font.filename).await;

        let cache_path = match cache_result {
            Ok(path) => path,
            Err(e) => {
                return FontInstallResult::Failed(format!(
                    "{}: cache/download failed - {}",
                    font.filename, e
                ))
            }
        };

        let was_cached = self.is_cached(font.filename);

        let dest_path = prefix.get_windows_fonts_path().join(font.filename);

        if let Err(e) = tokio::fs::copy(&cache_path, &dest_path).await {
            return FontInstallResult::Failed(format!(
                "Failed to copy {} to prefix: {}",
                font.filename, e
            ));
        }

        match registry.register_font(font.registry_name, font.filename).await {
            Ok(_) => {
                if was_cached {
                    FontInstallResult::Cached
                } else {
                    FontInstallResult::Installed
                }
            }
            Err(e) => {
                warn!(
                    "Could not register {} in registry: {}. File copied but not registered.",
                    font.filename, e
                );
                if was_cached {
                    FontInstallResult::Cached
                } else {
                    FontInstallResult::Installed
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn should_expose_15_segoe_ui_fonts_with_registry_names() {
        assert_eq!(SEGOE_UI_FONTS.len(), 15);
        assert_eq!(SEGOE_UI_FONTS[0].filename, "segoeui.ttf");
        assert_eq!(SEGOE_UI_FONTS[0].registry_name, "Segoe UI (TrueType)");
        assert_eq!(SEGOE_UI_FONTS[14].filename, "seguisym.ttf");
    }

    #[test]
    fn should_detect_cached_font_when_file_exists_with_content() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let temp_dir = TempDir::new().unwrap();
            let cache_dir = temp_dir.path().join("fonts");
            tokio::fs::create_dir_all(&cache_dir).await.unwrap();

            let font_file = cache_dir.join("segoeui.ttf");
            let mut file = std::fs::File::create(&font_file).unwrap();
            file.write_all(b"dummy font content").unwrap();

            let manager = FontManager {
                cache_dir,
                client: reqwest::Client::new(),
            };

            assert!(manager.is_cached("segoeui.ttf"));
            assert!(!manager.is_cached("nonexistent.ttf"));
        });
    }

    #[test]
    fn should_reject_empty_file_as_invalid_cache_entry() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let temp_dir = TempDir::new().unwrap();
            let cache_dir = temp_dir.path().join("fonts");
            tokio::fs::create_dir_all(&cache_dir).await.unwrap();

            let font_file = cache_dir.join("empty.ttf");
            std::fs::File::create(&font_file).unwrap();

            let manager = FontManager {
                cache_dir,
                client: reqwest::Client::new(),
            };

            assert!(!manager.is_cached("empty.ttf"));
        });
    }
}
