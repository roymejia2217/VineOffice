use crate::core::desktop_integration::DesktopIntegration;
use crate::core::wine_prefix::WinePrefixManager;
use crate::utils::command::CommandExecutor;
use std::time::Duration;
use tracing::{info, warn};

pub struct RollbackManager<'a> {
    prefix: &'a WinePrefixManager,
}

impl<'a> RollbackManager<'a> {
    pub fn new(prefix: &'a WinePrefixManager) -> Self {
        Self { prefix }
    }

    pub async fn perform_rollback(&self) -> RollbackResult {
        info!("Initiating installation rollback");

        let mut results = Vec::new();

        // Step 1: Kill Wine processes
        info!("Step 1: Stopping Wine processes");
        let kill_result = self.kill_wine_processes().await;
        results.push(("kill_wine_processes", kill_result.is_ok()));

        tokio::time::sleep(Duration::from_secs(2)).await;

        // Step 2: Remove desktop entries BEFORE deleting prefix
        // (we need prefix info to know which entries to remove)
        info!("Step 2: Removing desktop entries");
        match DesktopIntegration::remove_entries_for_prefix(self.prefix).await {
            Ok(_) => results.push(("remove_desktop_entries", true)),
            Err(e) => {
                warn!("Could not remove desktop entries during rollback: {}", e);
                results.push(("remove_desktop_entries", false));
            }
        }

        // Step 3: Remove prefix directory
        info!("Step 3: Removing WINEPREFIX");
        let remove_result = self.remove_prefix().await;
        results.push(("remove_prefix", remove_result.is_ok()));

        // Verify result
        let all_success = results.iter().all(|(_, success)| *success);

        if all_success {
            info!("Rollback completed successfully");
            RollbackResult::Success
        } else {
            warn!("Partial rollback");
            RollbackResult::Partial
        }
    }

    async fn kill_wine_processes(&self) -> Result<(), RollbackError> {
        let prefix_path = self.prefix.get_prefix_path().to_string_lossy().to_string();

        // Attempt to kill wineserver
        let _ = CommandExecutor::execute(
            "wineserver",
            &["-k"],
            &[("WINEPREFIX", &prefix_path)],
            Duration::from_secs(10),
        )
        .await;

        // Wait a moment
        tokio::time::sleep(Duration::from_secs(1)).await;

        Ok(())
    }

    async fn remove_prefix(&self) -> Result<(), RollbackError> {
        let path = self.prefix.get_prefix_path();
        
        if path.exists() {
            tokio::fs::remove_dir_all(path)
                .await
                .map_err(|e| RollbackError::RemoveFailed(e.to_string()))?;
        }
        
        Ok(())
    }

    pub fn should_rollback(step: &super::state::InstallStep) -> bool {
        // Only rollback for steps that leave no value if they fail or corrupt the prefix
        matches!(
            step,
            super::state::InstallStep::PrefixCreation
                | super::state::InstallStep::PreInstallDependencies
                | super::state::InstallStep::OfficeInstallation
        )
        // NOTE: PostInstallRegistry, FontFixes, PostInstallDependencies are
        // optional enhancements. Their failure does not invalidate the base installation.
        // DO NOT rollback on these steps.
    }
}

#[derive(Debug)]
pub enum RollbackResult {
    Success,
    Partial,
}

#[derive(Debug, thiserror::Error)]
pub enum RollbackError {
    #[error("Failed to remove prefix: {0}")]
    RemoveFailed(String),
}
