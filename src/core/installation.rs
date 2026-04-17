use crate::core::desktop_integration::DesktopIntegration;
use crate::core::font_manager::{FontInstallResult, FontManager, SEGOE_UI_FONTS};
use crate::core::product::ProductType;
use crate::core::registry::RegistryManager;
use crate::core::rollback::RollbackManager;
use crate::core::state::{InstallStep, ProgressEvent};
use crate::utils::logging::LogLevel;
use crate::core::wine_prefix::WinePrefixManager;
use crate::utils::command::{CommandError, CommandExecutor};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{error, info, warn};

pub struct InstallationManager {
    prefix: WinePrefixManager,
    setup_path: PathBuf,
    product: ProductType,
}

impl InstallationManager {
    pub fn new(prefix: WinePrefixManager, setup_path: PathBuf, product: ProductType) -> Self {
        Self {
            prefix,
            setup_path,
            product,
        }
    }

    pub async fn execute_step(
        &self,
        step: &InstallStep,
        on_progress: impl Fn(ProgressEvent),
    ) -> Result<String, InstallationError> {
        info!("Executing step: {:?}", step);

        let result = match step {
            InstallStep::PrefixCreation => self.create_prefix(&on_progress).await,
            InstallStep::PreInstallDependencies => self.install_pre_dependencies(&on_progress).await,
            InstallStep::OfficeInstallation => self.install_product(&on_progress).await,
            InstallStep::PostInstallRegistry => self.apply_registry_fixes(&on_progress).await,
            InstallStep::FontFixes => self.fix_fonts(&on_progress).await,
            InstallStep::PostInstallDependencies => self.install_post_dependencies(&on_progress).await,
        };

        match &result {
            Ok(_) => info!("Step {:?} completed successfully", step),
            Err(e) => {
                error!("Step {:?} failed: {}", step, e);
                let rollback = RollbackManager::new(&self.prefix);
                if RollbackManager::should_rollback(step) {
                    warn!("Initiating automatic rollback");
                    let _ = rollback.perform_rollback().await;
                }
            }
        }

        result
    }

    async fn create_prefix(
        &self,
        on_progress: &impl Fn(ProgressEvent),
    ) -> Result<String, InstallationError> {
        let prefix_path = self.prefix.get_prefix_path().to_string_lossy().to_string();
        let arch = self.prefix.get_arch();
        let total = 2usize;

        tokio::fs::create_dir_all(&prefix_path)
            .await
            .map_err(|e| InstallationError::IoError(e.to_string()))?;

        on_progress(ProgressEvent::SubProgress {
            current: 1,
            total,
            detail: "Initializing prefix with wineboot...".to_string(),
        });

        CommandExecutor::execute(
            "wineboot",
            &["--init"],
            &[
                ("WINEPREFIX", &prefix_path),
                ("WINEARCH", arch),
            ],
            Duration::from_secs(120),
        )
        .await
        .map_err(|e| InstallationError::CommandFailed(format!("wineboot --init failed: {}", e)))?;

        on_progress(ProgressEvent::SubProgress {
            current: 2,
            total,
            detail: "Setting Windows version to 7...".to_string(),
        });

        CommandExecutor::execute_winetricks(
            &prefix_path,
            arch,
            "win7",
            Duration::from_secs(60),
        )
        .await
        .map_err(|e| InstallationError::WinetricksFailed("win7".into(), e.to_string()))?;

        Ok("Prefix configured as Windows 7".into())
    }

    async fn install_pre_dependencies(
        &self,
        on_progress: &impl Fn(ProgressEvent),
    ) -> Result<String, InstallationError> {
        let components = WinetricksComponents::pre_install_for(self.product);
        if components.is_empty() {
            on_progress(ProgressEvent::SubProgress {
                current: 1, total: 1,
                detail: "No pre-dependencies required".to_string(),
            });
            return Ok("No pre-dependencies needed".into());
        }
        let total = components.len();
        let mut outputs = Vec::new();

        for (i, component) in components.iter().enumerate() {
            on_progress(ProgressEvent::SubProgress {
                current: i + 1,
                total,
                detail: format!("Installing {}...", component),
            });
            info!("Installing winetricks: {}", component);

            let output = CommandExecutor::execute_winetricks(
                &self.prefix.get_prefix_path().to_string_lossy(),
                self.prefix.get_arch(),
                component,
                Duration::from_secs(300),
            )
            .await
            .map_err(|e| InstallationError::WinetricksFailed(component.to_string(), e.to_string()))?;

            outputs.push(format!("{}: {}", component, output));
        }

        Ok(outputs.join("\n"))
    }

    async fn install_product(
        &self,
        on_progress: &impl Fn(ProgressEvent),
    ) -> Result<String, InstallationError> {
        let setup_str = self.setup_path.to_string_lossy().to_string();
        let product_display = self.product.to_info().display_name;
        let total = if self.product.is_office_family() { 3usize } else { 2usize };

        info!("Starting {} installation from: {}", product_display, setup_str);
        on_progress(ProgressEvent::SubProgress {
            current: 1,
            total,
            detail: format!("Launching {} setup...", product_display),
        });

        let output = CommandExecutor::execute(
            "wine",
            &[&setup_str],
            &[
                (
                    "WINEPREFIX",
                    &self.prefix.get_prefix_path().to_string_lossy(),
                ),
                ("WINEARCH", self.prefix.get_arch()),
            ],
            Duration::from_secs(1800),
        )
        .await
        .map_err(|e| InstallationError::OfficeInstallFailed(e.to_string()))?;

        on_progress(ProgressEvent::SubProgress {
            current: 2,
            total,
            detail: "Verifying installation...".to_string(),
        });

        if self.product.is_office_family() {
            if !self.prefix.is_product_installed(self.product) {
                return Err(InstallationError::ProductNotDetected(
                    product_display.to_string()
                ));
            }

            on_progress(ProgressEvent::SubProgress {
                current: 3,
                total,
                detail: "Creating desktop entries...".to_string(),
            });

            // Desktop integration is non-critical: the product is already installed.
            // A failure here does not warrant rollback or installation loss.
            // Users can repair via 'Repair Desktop Integration' in the Instance Manager.
            if let Err(e) = DesktopIntegration::create_entries_for_prefix(&self.prefix).await {
                warn!(
                    "Desktop integration failed (non-critical, Office is installed): {}. \
                     Run 'Repair Desktop Integration' from instance manager to retry.",
                    e
                );
                on_progress(ProgressEvent::Log(
                    LogLevel::Warn,
                    format!("Desktop entries skipped: {}. Use 'Repair' from instance list.", e),
                ));
            }
        } else {
            if let Err(e) = DesktopIntegration::create_entries_for_prefix(&self.prefix).await {
                warn!("Could not create desktop entries: {}", e);
            }
        }

        Ok(output)
    }

    async fn apply_registry_fixes(
        &self,
        on_progress: &impl Fn(ProgressEvent),
    ) -> Result<String, InstallationError> {
        if !self.product.is_office_family() {
            on_progress(ProgressEvent::SubProgress {
                current: 1,
                total: 1,
                detail: "Skipped (not applicable for generic application)".to_string(),
            });
            return Ok("Skipped for generic application".into());
        }

        let registry = RegistryManager::new(&self.prefix);
        let mut outputs = Vec::new();
        let total = 3usize;

        on_progress(ProgressEvent::SubProgress {
            current: 1,
            total,
            detail: "Applying hardware acceleration fix...".to_string(),
        });
        match registry.disable_hardware_acceleration().await {
            Ok(o) => outputs.push(o),
            Err(e) => warn!("Could not disable hardware acceleration: {}", e),
        }

        on_progress(ProgressEvent::SubProgress {
            current: 2,
            total,
            detail: "Applying Direct3D fix...".to_string(),
        });
        match registry.set_max_version_gl().await {
            Ok(o) => outputs.push(o),
            Err(e) => warn!("Could not configure MaxVersionGL: {}", e),
        }

        on_progress(ProgressEvent::SubProgress {
            current: 3,
            total,
            detail: "Applying Direct2D fix...".to_string(),
        });
        match registry.set_max_version_factory().await {
            Ok(o) => outputs.push(o),
            Err(e) => warn!("Could not configure max_version_factory: {}", e),
        }

        Ok(outputs.join("\n"))
    }

    async fn fix_fonts(
        &self,
        on_progress: &impl Fn(ProgressEvent),
    ) -> Result<String, InstallationError> {
        if !self.product.is_office_family() {
            on_progress(ProgressEvent::SubProgress {
                current: 1,
                total: 1,
                detail: "Skipped (not applicable for generic application)".to_string(),
            });
            return Ok("Skipped for generic application".into());
        }

        let registry = RegistryManager::new(&self.prefix);
        let fonts_path = self.prefix.get_windows_fonts_path();

        tokio::fs::create_dir_all(&fonts_path)
            .await
            .map_err(|e| InstallationError::IoError(e.to_string()))?;

        let system_fonts: &[(&str, &str)] = &[
            ("symbol.ttf", "Symbol (TrueType)"),
            ("wingding.ttf", "Wingdings (TrueType)"),
        ];

        let segoe_count = SEGOE_UI_FONTS.len();
        let system_count = system_fonts.len();
        let total_fonts = system_count + segoe_count;
        let mut outputs = Vec::new();
        let mut current = 0usize;

        // Install system fonts (symbol, wingding)
        for (filename, registry_name) in system_fonts.iter() {
            current += 1;
            on_progress(ProgressEvent::SubProgress {
                current,
                total: total_fonts,
                detail: format!("Copying {}...", filename),
            });

            match Self::find_wine_font_path(filename) {
                Some(source) => {
                    let dest = fonts_path.join(filename);
                    match tokio::fs::copy(&source, &dest).await {
                        Ok(_) => {
                            info!("{} copied from {}", filename, source.display());
                            match registry.register_font(registry_name, filename).await {
                                Ok(o) => outputs.push(o),
                                Err(e) => warn!("Could not register {}: {}", filename, e),
                            }
                        }
                        Err(e) => warn!(
                            "Could not copy {} from {}: {}",
                            filename,
                            source.display(),
                            e
                        ),
                    }
                }
                None => warn!("{} not found in any Wine fonts directory", filename),
            }
        }

        // Install Segoe UI fonts from GitHub
        match FontManager::new().await {
            Ok(font_manager) => {
                for font in SEGOE_UI_FONTS.iter() {
                    current += 1;
                    on_progress(ProgressEvent::SubProgress {
                        current,
                        total: total_fonts,
                        detail: format!("Installing {}...", font.filename),
                    });

                    match font_manager
                        .install_to_prefix(font, &self.prefix, &registry)
                        .await
                    {
                        FontInstallResult::Installed => {
                            info!("{} downloaded and installed", font.filename);
                            outputs.push(format!("{}: Downloaded+Installed", font.filename));
                        }
                        FontInstallResult::Cached => {
                            info!("{} installed from cache", font.filename);
                            outputs.push(format!("{}: Cached+Installed", font.filename));
                        }
                        FontInstallResult::Failed(e) => {
                            warn!("Could not install {}: {}", font.filename, e);
                            outputs.push(format!("{}: Failed (non-critical)", font.filename));
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Could not initialize font manager: {}", e);
                outputs
                    .push("Segoe UI fonts: Skipped (font manager initialization failed)".to_string());
            }
        }

        Ok(outputs.join("\n"))
    }

    /// Searches for a Wine font file across known distro-specific paths.
    /// Returns the first existing path or None.
    fn find_wine_font_path(font_filename: &str) -> Option<PathBuf> {
        const WINE_FONT_DIRS: &[&str] = &[
            "/usr/share/wine/fonts",             // Debian/Ubuntu
            "/usr/lib/wine/fonts",               // Fedora
            "/usr/lib32/wine/fonts",             // Arch lib32-wine
            "/opt/wine-staging/share/wine/fonts", // wine-staging
            "/opt/wine/share/wine/fonts",         // manual builds
            "/usr/share/wine-staging/fonts",      // staging variants
        ];

        WINE_FONT_DIRS
            .iter()
            .map(|dir| PathBuf::from(dir).join(font_filename))
            .find(|p| p.exists())
    }

    async fn install_post_dependencies(
        &self,
        on_progress: &impl Fn(ProgressEvent),
    ) -> Result<String, InstallationError> {
        if !self.product.is_office_family() {
            on_progress(ProgressEvent::SubProgress {
                current: 1,
                total: 1,
                detail: "Skipped (not applicable for generic application)".to_string(),
            });
            return Ok("Skipped for generic application".into());
        }

        let components = WinetricksComponents::post_install_for(self.product);
        let total = components.len();
        let mut outputs = Vec::new();

        for (i, component) in components.iter().enumerate() {
            on_progress(ProgressEvent::SubProgress {
                current: i + 1,
                total,
                detail: format!("Installing {}...", component),
            });
            info!("Installing winetricks: {}", component);

            let timeout = Duration::from_secs(300);

            let output = CommandExecutor::execute_winetricks(
                &self.prefix.get_prefix_path().to_string_lossy(),
                self.prefix.get_arch(),
                component,
                timeout,
            )
            .await;

            match output {
                Ok(o) => {
                    outputs.push(format!("{}: OK", component));
                    outputs.push(o);
                }
                Err(e) => {
                    warn!("Could not install {}: {}", component, e);
                    outputs.push(format!("{}: Failed (non-critical)", component));
                }
            }
        }

        Ok(outputs.join("\n"))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InstallationError {
    #[error("IO error: {0}")]
    IoError(String),

    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Winetricks failed for {0}: {1}")]
    WinetricksFailed(String, String),

    #[error("Installation failed: {0}")]
    OfficeInstallFailed(String),

    #[error("{0} not detected after installation")]
    ProductNotDetected(String),
}

impl From<CommandError> for InstallationError {
    fn from(e: CommandError) -> Self {
        InstallationError::CommandFailed(e.to_string())
    }
}

/// Centralizes winetricks component lists per product type.
/// Avoids hardcoding component lists inline in installation methods.
pub struct WinetricksComponents;

impl WinetricksComponents {
    /// Components required BEFORE the Office installer runs.
    pub fn pre_install_for(product: ProductType) -> &'static [&'static str] {
        match product {
            ProductType::Office2016 | ProductType::Project2016 | ProductType::Visio2016 => {
                &["msxml6", "riched20", "corefonts"]
            }
            ProductType::Generic => &[],
        }
    }

    /// Components required AFTER the Office installer completes.
    pub fn post_install_for(product: ProductType) -> &'static [&'static str] {
        match product {
            ProductType::Office2016 | ProductType::Project2016 | ProductType::Visio2016 => {
                &["msxml3", "msxml4", "vcrun2013", "vcrun2015", "gdiplus", "riched30", "pptfonts"]
            }
            ProductType::Generic => &[],
        }
    }
}
