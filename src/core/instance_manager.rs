use crate::core::desktop_integration::DesktopIntegration;
use crate::core::prefix_naming::PrefixNaming;
use crate::core::product::ProductType;
use crate::core::wine_prefix::WinePrefixManager;
use crate::utils::command::CommandExecutor;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::{error, info, warn};

#[derive(Clone, Debug)]
pub struct WineInstance {
    pub name: String,
    pub path: PathBuf,
    pub product_type: ProductType,
    pub created_at: Option<SystemTime>,
    pub size_bytes: u64,
    pub is_installed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum InstanceError {
    #[error("Prefix not managed by this application: {0}")]
    NotManaged(String),
    #[error("Could not delete prefix: {0}")]
    DeleteFailed(String),
    #[error("Could not launch application: {0}")]
    LaunchFailed(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct InstanceManager;

impl InstanceManager {
    /// Detects all managed Wine instances asynchronously.
    /// Uses spawn_blocking to prevent blocking the UI event loop during
    /// filesystem-heavy walkdir operations on large prefixes.
    pub async fn detect_instances() -> Vec<WineInstance> {
        tokio::task::spawn_blocking(Self::detect_instances_sync)
            .await
            .unwrap_or_else(|e| {
                warn!("detect_instances task panicked: {}", e);
                Vec::new()
            })
    }

    fn detect_instances_sync() -> Vec<WineInstance> {
        let mut instances = Vec::new();

        let Some(_home) = dirs::home_dir() else {
            warn!("Could not determine home directory");
            return instances;
        };

        let patterns = PrefixNaming::all_glob_patterns();

        for pattern in patterns {
            match glob::glob(&pattern) {
                Ok(paths) => {
                    for entry in paths.flatten() {
                        if let Some(mut instance) = Self::analyze_prefix(&entry) {
                            instance.size_bytes = Self::calculate_size_sync(&entry);
                            instances.push(instance);
                        }
                    }
                }
                Err(e) => {
                    warn!("Glob pattern error {}: {}", pattern, e);
                }
            }
        }

        instances.sort_by(|a, b| {
            b.created_at.unwrap_or(SystemTime::UNIX_EPOCH)
                .cmp(&a.created_at.unwrap_or(SystemTime::UNIX_EPOCH))
        });

        info!("Detected {} Wine instances", instances.len());
        instances
    }

    fn analyze_prefix(path: &Path) -> Option<WineInstance> {
        if !path.is_dir() {
            return None;
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let product_type = PrefixNaming::extract_product_from_prefix_name(&name)
            .unwrap_or(ProductType::Generic);

        let is_installed = Self::is_product_installed(path, product_type);
        let created_at = Self::get_creation_time(path);

        Some(WineInstance {
            name,
            path: path.to_path_buf(),
            product_type,
            created_at,
            size_bytes: 0,
            is_installed,
        })
    }

    fn calculate_size_sync(path: &Path) -> u64 {
        walkdir::WalkDir::new(path)
            .into_iter()
            .flatten()
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .map(|m| m.len())
            .sum()
    }

    /// Delegates entirely to WinePrefixManager for product detection.
    /// Generic products only need drive_c and system.reg to be considered installed.
    fn is_product_installed(path: &Path, product: ProductType) -> bool {
        if product == ProductType::Generic {
            return path.join("drive_c").exists() && path.join("system.reg").exists();
        }
        // WinePrefixManager encapsulates all detection logic — no duplication
        WinePrefixManager::from_existing_path(path.to_path_buf())
            .map(|pm| pm.is_product_installed(product))
            .unwrap_or(false)
    }

    fn get_creation_time(path: &Path) -> Option<SystemTime> {
        std::fs::metadata(path)
            .ok()
            .and_then(|m| m.created().ok())
    }

    pub fn is_managed_prefix(path: &Path) -> bool {
        if !path.is_dir() {
            return false;
        }

        let drive_c = path.join("drive_c");
        let user_reg = path.join("user.reg");

        if !drive_c.exists() || !user_reg.exists() {
            return false;
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        PrefixNaming::is_managed_prefix(&name)
    }

    pub async fn delete_instance(path: &Path) -> Result<(), InstanceError> {
        if !Self::is_managed_prefix(path) {
            return Err(InstanceError::NotManaged(path.display().to_string()));
        }

        info!("Deleting instance: {}", path.display());

        let prefix_path = path.to_string_lossy().to_string();

        let _ = CommandExecutor::execute(
            "wineserver",
            &["-k"],
            &[("WINEPREFIX", &prefix_path)],
            Duration::from_secs(10),
        )
        .await;

        tokio::time::sleep(Duration::from_secs(1)).await;

        let prefix_manager = match WinePrefixManager::from_existing_path(path.to_path_buf()) {
            Ok(pm) => pm,
            Err(e) => {
                warn!("Invalid prefix for desktop integration: {}", e);
                return tokio::fs::remove_dir_all(path).await
                    .map_err(|e| InstanceError::DeleteFailed(e.to_string()));
            }
        };
        if let Err(e) = DesktopIntegration::remove_entries_for_prefix(&prefix_manager).await {
            warn!("Could not remove desktop entries: {}", e);
        }

        match tokio::fs::remove_dir_all(path).await {
            Ok(_) => {
                info!("Instance deleted successfully: {}", path.display());
                Ok(())
            }
            Err(e) => {
                error!("Error deleting {}: {}", path.display(), e);
                Err(InstanceError::DeleteFailed(e.to_string()))
            }
        }
    }

    pub async fn launch_product(path: &Path, product: ProductType) -> Result<(), InstanceError> {
        if product == ProductType::Generic {
            Self::open_in_file_manager(path);
            return Ok(());
        }

        if !Self::is_managed_prefix(path) {
            return Err(InstanceError::NotManaged(path.display().to_string()));
        }

        let prefix_manager = match WinePrefixManager::from_existing_path(path.to_path_buf()) {
            Ok(pm) => pm,
            Err(e) => {
                return Err(InstanceError::LaunchFailed(
                    format!("Could not access prefix: {}", e)
                ));
            }
        };

        let info = product.to_info();
        let exe_path = prefix_manager.get_product_exe_path(product);

        if !exe_path.exists() {
            return Err(InstanceError::LaunchFailed(
                format!("{} not found", info.exe_name)
            ));
        }

        let prefix_path = path.to_string_lossy().to_string();
        let exe_str = exe_path.to_string_lossy().to_string();

        info!("Launching {} from: {}", info.display_name, prefix_path);

        // GUI apps run indefinitely — spawn without waiting for termination.
        // Using spawn_detached instead of execute() which had a 30s timeout
        // that always caused Timeout errors for interactive applications.
        CommandExecutor::spawn_detached(
            "wine",
            &[&exe_str],
            &[("WINEPREFIX", &prefix_path), ("WINEARCH", crate::core::wine_prefix::WINE_DEFAULT_ARCH)],
        )
        .map(|_child| {
            info!("{} launched successfully", info.display_name);
        })
        .map_err(|e| {
            error!("Error launching {}: {}", info.display_name, e);
            InstanceError::LaunchFailed(e.to_string())
        })
    }

    pub fn open_in_file_manager(path: &Path) {
        let path_str = path.to_string_lossy().to_string();

        let commands = [
            ("xdg-open", vec![&path_str]),
            ("nautilus", vec![&path_str]),
            ("dolphin", vec![&path_str]),
            ("thunar", vec![&path_str]),
            ("pcmanfm", vec![&path_str]),
        ];

        for (cmd, args) in commands {
            if which::which(cmd).is_ok() {
                let _ = std::process::Command::new(cmd)
                    .args(args)
                    .spawn();
                break;
            }
        }
    }
}
