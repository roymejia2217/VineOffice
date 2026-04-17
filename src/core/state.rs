use crate::core::product::ProductType;
use crate::utils::logging::LogLevel;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum InstallStep {
    PrefixCreation,
    PreInstallDependencies,
    OfficeInstallation,
    PostInstallRegistry,
    FontFixes,
    PostInstallDependencies,
}

impl InstallStep {
    pub fn all_steps() -> Vec<InstallStep> {
        vec![
            InstallStep::PrefixCreation,
            InstallStep::PreInstallDependencies,
            InstallStep::OfficeInstallation,
            InstallStep::PostInstallRegistry,
            InstallStep::FontFixes,
            InstallStep::PostInstallDependencies,
        ]
    }

    pub fn step_count() -> usize {
        Self::all_steps().len()
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            InstallStep::PrefixCreation => "Creating prefix",
            InstallStep::PreInstallDependencies => "Installing prerequisites",
            InstallStep::OfficeInstallation => "Installing",
            InstallStep::PostInstallRegistry => "Configuring registry",
            InstallStep::FontFixes => "Installing fonts",
            InstallStep::PostInstallDependencies => "Installing components",
        }
    }
}

#[derive(Clone, Debug)]
pub enum ProgressEvent {
    StepStarted(InstallStep),
    StepCompleted(InstallStep),
    SubProgress {
        current: usize,
        total: usize,
        detail: String,
    },
    Log(LogLevel, String),
    Error(String),
    Completed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallationState {
    pub id: String,
    pub current_step: InstallStep,
    pub completed_steps: Vec<InstallStep>,
    pub setup_path: PathBuf,
    pub prefix_path: PathBuf,
    pub product_type: String, // "Office2016", "Project2016", "Visio2016"
    pub failed: bool,
    pub error_message: Option<String>,
}

impl InstallationState {
    pub fn new(setup_path: PathBuf, prefix_path: PathBuf, product_type: ProductType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            current_step: InstallStep::PrefixCreation,
            completed_steps: Vec::new(),
            setup_path,
            prefix_path,
            product_type: product_type.as_str().to_string(),
            failed: false,
            error_message: None,
        }
    }

    pub fn get_product_type(&self) -> ProductType {
        ProductType::from_str(&self.product_type)
    }

    pub fn mark_step_complete(&mut self, step: InstallStep) {
        if !self.completed_steps.contains(&step) {
            self.completed_steps.push(step.clone());
        }
    }

    pub fn set_current_step(&mut self, step: InstallStep) {
        self.current_step = step;
    }

    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.failed = true;
        self.error_message = Some(error.into());
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.current_step, InstallStep::PostInstallDependencies)
            && self
                .completed_steps
                .contains(&InstallStep::PostInstallDependencies)
    }
}

pub struct StateManager;

impl StateManager {
    pub fn get_state_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("vineoffice")
    }

    /// Generates a unique state file path per prefix using UUID stored in the state
    fn get_state_file_for_prefix(prefix_path: &Path) -> PathBuf {
        // Try to load existing state to get its UUID
        let dir = Self::get_state_dir();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(state) = serde_json::from_str::<InstallationState>(&content) {
                            if state.prefix_path == prefix_path {
                                return path;
                            }
                        }
                    }
                }
            }
        }

        // Create new state file with UUID
        let uuid = Uuid::new_v4();
        dir.join(format!("state_{}.json", uuid))
    }

    pub fn save(state: &InstallationState, prefix_path: &Path) -> anyhow::Result<()> {
        let dir = Self::get_state_dir();
        std::fs::create_dir_all(&dir)?;

        let file_path = Self::get_state_file_for_prefix(prefix_path);
        let json = serde_json::to_string_pretty(state)?;
        std::fs::write(&file_path, json)?;

        Ok(())
    }

    pub fn clear(prefix_path: &Path) -> anyhow::Result<()> {
        let dir = Self::get_state_dir();

        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(state) = serde_json::from_str::<InstallationState>(&content) {
                            if state.prefix_path == prefix_path {
                                std::fs::remove_file(path)?;
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Finds and returns all resumable states in the cache directory
    pub fn find_all_resumable() -> Vec<(PathBuf, InstallationState)> {
        let mut result = Vec::new();
        let state_dir = Self::get_state_dir();

        if let Ok(entries) = std::fs::read_dir(&state_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(state) = serde_json::from_str::<InstallationState>(&content) {
                            if !state.failed && !state.is_complete() {
                                result.push((state.prefix_path.clone(), state));
                            }
                        }
                    }
                }
            }
        }

        result
    }

    /// Checks if any installation can be resumed (global check)
    pub fn can_resume_any() -> bool {
        !Self::find_all_resumable().is_empty()
    }

    /// Loads the first resumable state found (for simple resume)
    pub fn load_any_resumable() -> Option<InstallationState> {
        Self::find_all_resumable()
            .into_iter()
            .next()
            .map(|(_, state)| state)
    }
}
