use crate::core::wine_prefix::WinePrefixManager;
use crate::utils::command::CommandExecutor;
use std::time::Duration;

pub struct RegistryManager<'a> {
    prefix: &'a WinePrefixManager,
}

impl<'a> RegistryManager<'a> {
    pub fn new(prefix: &'a WinePrefixManager) -> Self {
        Self { prefix }
    }

    pub async fn disable_hardware_acceleration(&self) -> Result<String, crate::utils::command::CommandError> {
        CommandExecutor::execute_wine_command(
            &self.prefix.get_prefix_path().to_string_lossy(),
            self.prefix.get_arch(),
            "reg",
            &[
                "add",
                "HKCU\\Software\\Microsoft\\Office\\16.0\\Common\\Graphics",
                "/v", "DisableHardwareAcceleration",
                "/t", "REG_DWORD",
                "/d", "1",
                "/f",
            ],
            Duration::from_secs(30),
        )
        .await
    }

    pub async fn set_max_version_gl(&self) -> Result<String, crate::utils::command::CommandError> {
        CommandExecutor::execute_wine_command(
            &self.prefix.get_prefix_path().to_string_lossy(),
            self.prefix.get_arch(),
            "reg",
            &[
                "add",
                "HKCU\\Software\\Wine\\Direct3D",
                "/v", "MaxVersionGL",
                "/t", "REG_DWORD",
                "/d", "0x30002",
                "/f",
            ],
            Duration::from_secs(30),
        )
        .await
    }

    pub async fn set_max_version_factory(&self) -> Result<String, crate::utils::command::CommandError> {
        CommandExecutor::execute_wine_command(
            &self.prefix.get_prefix_path().to_string_lossy(),
            self.prefix.get_arch(),
            "reg",
            &[
                "add",
                "HKCU\\Software\\Wine\\Direct2D",
                "/v", "max_version_factory",
                "/t", "REG_DWORD",
                "/d", "0",
                "/f",
            ],
            Duration::from_secs(30),
        )
        .await
    }

    pub async fn register_font(&self, font_name: &str, font_file: &str) -> Result<String, crate::utils::command::CommandError> {
        CommandExecutor::execute_wine_command(
            &self.prefix.get_prefix_path().to_string_lossy(),
            self.prefix.get_arch(),
            "reg",
            &[
                "add",
                "HKLM\\Software\\Microsoft\\Windows NT\\CurrentVersion\\Fonts",
                "/v", font_name,
                "/t", "REG_SZ",
                "/d", font_file,
                "/f",
            ],
            Duration::from_secs(30),
        )
        .await
    }
}
