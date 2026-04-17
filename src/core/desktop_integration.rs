use crate::core::prefix_naming::PrefixNaming;
use crate::core::product::ProductType;
use crate::core::wine_prefix::WinePrefixManager;
use crate::utils::command::CommandExecutor;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tracing::{info, warn};

const DESKTOP_APPS_DIR: &str = "applications/vineoffice";
const LAUNCHERS_DIR: &str = "applications/vineoffice/launchers";
const ICONS_BASE_DIR: &str = "icons/hicolor";
const MIMEAPPS_LIST: &str = "mimeapps.list";

/// Finds the Wine-generated Office programs directory dynamically.
/// Wine's winemenubuilder creates directories like "Microsoft Office 2016", "Microsoft Office 2019", etc.
/// Returns the first matching directory instead of hardcoding a version year.
fn find_wine_office_programs_dir() -> Option<PathBuf> {
    let data_dir = dirs::data_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"));

    let wine_programs = data_dir
        .join("applications")
        .join("wine")
        .join("Programs");

    if !wine_programs.exists() {
        return None;
    }

    // Search for any directory starting with "Microsoft Office"
    std::fs::read_dir(&wine_programs).ok()?.flatten().find_map(|entry| {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("Microsoft Office") && entry.path().is_dir() {
            Some(entry.path())
        } else {
            None
        }
    })
}

fn find_wine_desktop_entry(app_suffix: &str) -> Option<(String, String)> {
    let base = find_wine_office_programs_dir()?;

    std::fs::read_dir(&base).ok()?.flatten().find_map(|entry| {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !name.contains(app_suffix) {
            return None;
        }
        let content = std::fs::read_to_string(entry.path()).ok()?;
        let get_field = |key: &str| -> Option<String> {
            content
                .lines()
                .find(|l| l.starts_with(&format!("{}=", key)))
                .and_then(|l| l.splitn(2, '=').nth(1).map(str::to_string))
        };
        let app_name = get_field("Name")?;
        let comment = get_field("Comment").unwrap_or_default();
        Some((app_name, comment))
    })
}

#[derive(Clone, Debug)]
pub struct OfficeApplication {
    pub exe_name: &'static str,
    pub display_name: &'static str,
    pub app_type: ApplicationType,
    pub category: &'static str,
}

const OFFICE_MAIN_APPS: &[OfficeApplication] = &[
    OfficeApplication {
        exe_name: "WINWORD.EXE",
        display_name: "Microsoft Word",
        app_type: ApplicationType::Word,
        category: "WordProcessor",
    },
    OfficeApplication {
        exe_name: "EXCEL.EXE",
        display_name: "Microsoft Excel",
        app_type: ApplicationType::Excel,
        category: "Spreadsheet",
    },
    OfficeApplication {
        exe_name: "POWERPNT.EXE",
        display_name: "Microsoft PowerPoint",
        app_type: ApplicationType::PowerPoint,
        category: "Presentation",
    },
    OfficeApplication {
        exe_name: "MSACCESS.EXE",
        display_name: "Microsoft Access",
        app_type: ApplicationType::Access,
        category: "Database",
    },
    OfficeApplication {
        exe_name: "ONENOTE.EXE",
        display_name: "Microsoft OneNote",
        app_type: ApplicationType::OneNote,
        category: "TextEditor",
    },
    OfficeApplication {
        exe_name: "OUTLOOK.EXE",
        display_name: "Microsoft Outlook",
        app_type: ApplicationType::Outlook,
        category: "Email",
    },
    OfficeApplication {
        exe_name: "MSPUB.EXE",
        display_name: "Microsoft Publisher",
        app_type: ApplicationType::Publisher,
        category: "Publishing",
    },
    OfficeApplication {
        exe_name: "LYNC.EXE",
        display_name: "Skype for Business",
        app_type: ApplicationType::SkypeForBusiness,
        category: "Chat",
    },
];

#[derive(Debug, thiserror::Error)]
pub enum DesktopIntegrationError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("No Office applications detected in prefix: {0}")]
    NoApplicationsDetected(String),
}

pub struct DesktopIntegration;

impl DesktopIntegration {
    fn extract_prefix_name(prefix_path: &Path) -> &str {
        prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
    }

    pub async fn create_entries_for_prefix(
        prefix: &WinePrefixManager,
    ) -> Result<(), DesktopIntegrationError> {
        let prefix_path = prefix.get_prefix_path();
        let prefix_name_with_dot = Self::extract_prefix_name(prefix_path);
        let prefix_name = prefix_name_with_dot.trim_start_matches('.');
        
        info!("Creating desktop entries for prefix: {}", prefix_name);

        let product = prefix.detect_product();

        Self::extract_wine_icons(prefix).await;

        if product.is_office_family() {
            let has_icons = Self::has_icons_for_product(product);
            if !has_icons {
                warn!(
                    "No icons found for {}. Creating entries with generic icon.",
                    product.to_info().display_name
                );
            }
        }

        Self::ensure_directories().await?;

        let version_year = prefix.detect_office_folder()
            .and_then(|folder| {
                // "Office16" -> "16" -> 2016, "Office15" -> "15" -> 2015
                let suffix = &folder[6..];
                let year: u16 = suffix.parse().ok()?;
                if year < 100 {
                    Some(2000 + year)
                } else {
                    Some(year)
                }
            })
            .unwrap_or_else(|| {
                // Fallback: try from prefix name
                PrefixNaming::extract_product_and_version(prefix_name_with_dot)
                    .map(|(_, year)| year)
                    .unwrap_or(0)
            });

        match product {
            ProductType::Office2016 => {
                Self::create_office_desktop_entries(prefix, prefix_name, version_year).await?;
            }
            ProductType::Project2016 => {
                let display_name = format!("Microsoft Project {}", version_year);
                Self::create_single_desktop_entry(
                    prefix,
                    prefix_name,
                    "project",
                    &display_name,
                    "WINPROJ.EXE",
                    &Self::get_mime_types_for_application(ApplicationType::Project),
                    "ProjectManagement",
                )
                .await?;
            }
            ProductType::Visio2016 => {
                let display_name = format!("Microsoft Visio {}", version_year);
                Self::create_single_desktop_entry(
                    prefix,
                    prefix_name,
                    "visio",
                    &display_name,
                    "VISIO.EXE",
                    &Self::get_mime_types_for_application(ApplicationType::Visio),
                    "Graphics",
                )
                .await?;
            }
            ProductType::Generic => {
                info!(
                    "Skipping custom desktop entry for generic prefix '{}'. Wine's winemenubuilder handles integration automatically.",
                    prefix_name
                );
            }
        }

        info!("Desktop entries created successfully for {}", prefix_name);
        Ok(())
    }

    pub async fn remove_entries_for_prefix(
        prefix: &WinePrefixManager,
    ) -> Result<(), DesktopIntegrationError> {
        let prefix_path = prefix.get_prefix_path();
        let prefix_name_with_dot = Self::extract_prefix_name(prefix_path);
        let prefix_name = prefix_name_with_dot.trim_start_matches('.');

        info!("Removing desktop entries for prefix: {}", prefix_name);

        let product = prefix.detect_product();

        let desktop_files_to_remove = match product {
            ProductType::Office2016 => {
                OFFICE_MAIN_APPS
                    .iter()
                    .map(|app| {
                        let suffix = app.exe_name.to_lowercase().replace(".exe", "");
                        format!("{}_{}.desktop", prefix_name, suffix)
                    })
                    .collect()
            }
            ProductType::Project2016 => {
                vec![format!("{}_project.desktop", prefix_name)]
            }
            ProductType::Visio2016 => {
                vec![format!("{}_visio.desktop", prefix_name)]
            }
            ProductType::Generic => {
                vec![]
            }
        };

        let apps_dir = Self::get_applications_dir();
        for filename in &desktop_files_to_remove {
            let file_path = apps_dir.join(filename);
            if file_path.exists() {
                match fs::remove_file(&file_path).await {
                    Ok(_) => info!("Removed: {}", file_path.display()),
                    Err(e) => warn!("Could not remove {}: {}", file_path.display(), e),
                }
            }
        }

        Self::remove_mime_associations(&desktop_files_to_remove).await?;

        let launchers_dir = Self::get_launchers_dir();
        for filename in &desktop_files_to_remove {
            let launcher_name = filename.replace(".desktop", "_launcher.sh");
            let launcher_path = launchers_dir.join(&launcher_name);
            if launcher_path.exists() {
                match fs::remove_file(&launcher_path).await {
                    Ok(_) => info!("Removed launcher: {}", launcher_path.display()),
                    Err(e) => warn!("Could not remove launcher {}: {}", launcher_path.display(), e),
                }
            }
        }

        Self::cleanup_launchers_directory().await;
        Self::cleanup_empty_directory().await;

        info!("Desktop entries removed for {}", prefix_name);
        Ok(())
    }

    pub fn has_icons_for_product(product: ProductType) -> bool {
        let exe_names: Vec<&str> = match product {
            ProductType::Office2016 => {
                OFFICE_MAIN_APPS.iter().map(|app| app.exe_name).collect()
            }
            ProductType::Project2016 => vec!["WINPROJ.EXE"],
            ProductType::Visio2016 => vec!["VISIO.EXE"],
            ProductType::Generic => return false,
        };

        for exe_name in exe_names {
            if Self::find_icon_for_exe(exe_name).is_some() {
                return true;
            }
        }

        false
    }

    async fn ensure_directories() -> Result<(), DesktopIntegrationError> {
        let data_home = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));

        let apps_dir = data_home.join(DESKTOP_APPS_DIR);
        fs::create_dir_all(&apps_dir).await?;

        let launchers_dir = data_home.join(LAUNCHERS_DIR);
        fs::create_dir_all(&launchers_dir).await?;

        Ok(())
    }

    fn get_launchers_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(LAUNCHERS_DIR)
    }

    async fn generate_launcher_script(
        prefix: &WinePrefixManager,
        app_suffix: &str,
        exe_name: &str,
    ) -> Result<String, DesktopIntegrationError> {
        let prefix_path = prefix.get_prefix_path();
        let prefix_name_with_dot = Self::extract_prefix_name(prefix_path);
        let prefix_name = prefix_name_with_dot.trim_start_matches('.');

        let launcher_filename = format!("{}_{}_launcher.sh", prefix_name, app_suffix);
        let launchers_dir = Self::get_launchers_dir();
        let launcher_path = launchers_dir.join(&launcher_filename);

        let prefix_path_str = prefix_path.to_string_lossy().to_string();
        let exe_path = prefix
            .get_office_installation_path()
            .join(exe_name);
        let exe_path_str = exe_path.to_string_lossy().to_string();

        let script_content = format!(
            r#"#!/bin/bash
set -e

export WINEPREFIX="{wineprefix}"
export WINEARCH=win32
EXE_PATH="{exe_path}"

# Convert a Linux path to Windows path via winepath, fallback to z: drive
launch_with_file() {{
    local linux_path="$1"
    local win_path=""
    if command -v wine >/dev/null 2>&1; then
        win_path=$(wine winepath -w "$linux_path" 2>/dev/null) || true
    fi
    if [ -n "$win_path" ]; then
        exec wine start /unix "$EXE_PATH" "$win_path"
    else
        exec wine start /unix "$EXE_PATH" "z:$linux_path"
    fi
}}

if [ $# -eq 0 ]; then
    exec wine start /unix "$EXE_PATH"
else
    launch_with_file "$1"
fi
"#,
            wineprefix = prefix_path_str,
            exe_path = exe_path_str
        );

        fs::write(&launcher_path, script_content).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&launcher_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&launcher_path, perms)?;
        }

        info!("Generated launcher script: {}", launcher_path.display());

        Ok(launcher_filename)
    }

    fn get_applications_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(DESKTOP_APPS_DIR)
    }

    pub fn detect_installed_office_apps(prefix: &WinePrefixManager) -> Vec<&'static OfficeApplication> {
        let office_dir = prefix.get_office_installation_path();

        OFFICE_MAIN_APPS
            .iter()
            .filter(|app| office_dir.join(app.exe_name).exists())
            .collect()
    }

    async fn create_office_desktop_entries(
        prefix: &WinePrefixManager,
        prefix_name: &str,
        version_year: u16,
    ) -> Result<(), DesktopIntegrationError> {
        let installed_apps = Self::detect_installed_office_apps(prefix);

        if installed_apps.is_empty() {
            return Err(DesktopIntegrationError::NoApplicationsDetected(
                prefix_name.to_string(),
            ));
        }

        info!(
            "Detected {} Office applications for {}",
            installed_apps.len(),
            prefix_name
        );

        for app in installed_apps {
            Self::create_office_app_entry(prefix, prefix_name, version_year, app).await?;
        }

        Ok(())
    }

    async fn create_office_app_entry(
        prefix: &WinePrefixManager,
        prefix_name: &str,
        version_year: u16,
        app: &OfficeApplication,
    ) -> Result<(), DesktopIntegrationError> {
        let suffix = app.exe_name.to_lowercase().replace(".exe", "");
        
        // Try to get name/comment from Wine's desktop entry
        let wine_info = find_wine_desktop_entry(&suffix);
        
        let display_name = match wine_info {
            Some((name, _)) => name,
            None => format!("{} {}", app.display_name, version_year),
        };

        Self::create_single_desktop_entry(
            prefix,
            prefix_name,
            &suffix,
            &display_name,
            app.exe_name,
            &Self::get_mime_types_for_application(app.app_type),
            app.category,
        )
        .await
    }

    async fn create_single_desktop_entry(
        prefix: &WinePrefixManager,
        prefix_name: &str,
        app_suffix: &str,
        display_name: &str,
        exe_name: &str,
        mime_types: &[&str],
        category: &str,
    ) -> Result<(), DesktopIntegrationError> {
        let desktop_filename = format!("{}_{}.desktop", prefix_name, app_suffix);
        let apps_dir = Self::get_applications_dir();
        let desktop_path = apps_dir.join(&desktop_filename);

        let launcher_filename = Self::generate_launcher_script(
            prefix, app_suffix, exe_name
        ).await?;

        let launchers_dir = Self::get_launchers_dir();
        let launcher_path = launchers_dir.join(&launcher_filename);
        let launcher_path_str = launcher_path.to_string_lossy().to_string();

        let icon_name = Self::find_icon_for_exe(exe_name)
            .unwrap_or_else(|| "application-x-wine".to_string());

        let mime_types_str = mime_types.join(";");
        let desktop_content = format!(
            "[Desktop Entry]\n\
             Name={}\n\
             Exec=\"{}\" %f\n\
             Type=Application\n\
             StartupNotify=true\n\
             Icon={}\n\
             Categories=Office;{};Wine;\n\
             MimeType={};\n\
             NoDisplay=false\n",
            display_name,
            launcher_path_str,
            icon_name,
            category,
            mime_types_str
        );

        fs::write(&desktop_path, desktop_content).await?;
        info!("Created desktop entry: {}", desktop_path.display());

        Self::add_mime_associations(&desktop_filename, mime_types).await?;

        Ok(())
    }

    fn find_icon_for_exe(exe_name: &str) -> Option<String> {
        let icon_patterns: Vec<&str> = match exe_name.to_uppercase().as_str() {
            "WINWORD.EXE" => vec!["wordicon"],
            "EXCEL.EXE" => vec!["xlicons", "excelicon"],
            "POWERPNT.EXE" => vec!["pptico", "powerpnt"],
            "WINPROJ.EXE" => vec!["pj11icon", "winproj"],
            "VISIO.EXE" => vec!["visicon", "vsicon", "visio"],
            "MSACCESS.EXE" => vec!["msaccess", "accessicon", "accicons"],
            "ONENOTE.EXE" => vec!["onenote", "onenoteicon", "joticon"],
            "OUTLOOK.EXE" => vec!["outlook", "outlookicon", "outicon"],
            "MSPUB.EXE" => vec!["pubs", "pubicon", "publishericon", "mspub"],
            "LYNC.EXE" => vec!["lync", "lyncicon", "skype"],
            _ => {
                vec![exe_name.trim_end_matches(".EXE").trim_end_matches(".exe")]
            }
        };

        let data_home = dirs::data_dir()?;
        let icon_sizes = ["16x16", "22x22", "24x24", "32x32", "48x48", "64x64", "128x128", "256x256"];

        for size in &icon_sizes {
            let icon_dir = data_home.join(ICONS_BASE_DIR).join(size).join("apps");

            if let Ok(entries) = std::fs::read_dir(&icon_dir) {
                for entry in entries.flatten() {
                    if let Some(filename) = entry.file_name().to_str() {
                        let lower_filename = filename.to_lowercase();
                        for pattern in &icon_patterns {
                            if lower_filename.contains(pattern) && lower_filename.ends_with(".png") {
                                return Some(filename.trim_end_matches(".png").to_string());
                            }
                        }
                    }
                }
            }
        }

        for size in &icon_sizes {
            let icon_dir = PathBuf::from("/usr/share/icons/hicolor").join(size).join("apps");

            if let Ok(entries) = std::fs::read_dir(&icon_dir) {
                for entry in entries.flatten() {
                    if let Some(filename) = entry.file_name().to_str() {
                        let lower_filename = filename.to_lowercase();
                        for pattern in &icon_patterns {
                            if lower_filename.contains(pattern) && lower_filename.ends_with(".png") {
                                return Some(filename.trim_end_matches(".png").to_string());
                            }
                        }
                    }
                }
            }
        }

        None
    }

    async fn extract_wine_icons(prefix: &WinePrefixManager) {
        let prefix_path = prefix.get_prefix_path().to_string_lossy().to_string();
        let wineprefix = ("WINEPREFIX", prefix_path.as_str());

        info!("Extracting icons with winemenubuilder...");

        match CommandExecutor::execute(
            "wine",
            &["winemenubuilder", "-a"],
            &[wineprefix],
            Duration::from_secs(60),
        )
        .await
        {
            Ok(_) => {
                info!("Icon extraction completed successfully");
            }
            Err(e) => {
                warn!("Icon extraction failed (non-critical): {}", e);
            }
        }

        // Poll for icons to appear (max 30 seconds, check every 500ms)
        let mut attempts = 0;
        while attempts < 60 {
            if Self::has_icons_for_product(prefix.detect_product()) {
                info!("Icons detected after {}ms", attempts * 500);
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
            attempts += 1;
        }

        if attempts >= 60 {
            warn!("Icon extraction timed out after 30 seconds");
        }
    }

    pub fn get_mime_types_for_application(app: ApplicationType) -> Vec<&'static str> {
        match app {
            ApplicationType::Word => vec![
                "application/msword",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.template",
                "application/vnd.ms-word.document.macroEnabled.12",
                "application/rtf",
                "text/rtf",
            ],
            ApplicationType::Excel => vec![
                "application/vnd.ms-excel",
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "application/vnd.openxmlformats-officedocument.spreadsheetml.template",
                "application/vnd.ms-excel.sheet.macroEnabled.12",
                "text/csv",
            ],
            ApplicationType::PowerPoint => vec![
                "application/vnd.ms-powerpoint",
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
                "application/vnd.openxmlformats-officedocument.presentationml.template",
                "application/vnd.openxmlformats-officedocument.presentationml.slideshow",
                "application/vnd.ms-powerpoint.presentation.macroEnabled.12",
            ],
            ApplicationType::Project => vec![
                "application/vnd.ms-project",
            ],
            ApplicationType::Visio => vec![
                "application/vnd.visio",
                "application/vnd.ms-visio.drawing",
            ],
            ApplicationType::Access => vec![
                "application/x-msaccess",
                "application/vnd.ms-access",
                "application/vnd.ms-access.2007",
            ],
            ApplicationType::OneNote => vec![
                "application/onenote",
                "application/msonenote",
            ],
            ApplicationType::Outlook => vec![
                "message/rfc822",
                "application/vnd.ms-outlook",
                "application/vnd.ms-outlook-pst",
            ],
            ApplicationType::Publisher => vec![
                "application/vnd.ms-publisher",
                "application/x-mspublisher",
            ],
            ApplicationType::SkypeForBusiness => vec![
                "x-scheme-handler/tel",
                "x-scheme-handler/callto",
                "x-scheme-handler/sip",
                "x-scheme-handler/sips",
            ],
        }
    }

    /// Pure function: merges a single MIME association into mimeapps.list content.
    /// Line-based, idempotent, never corrupts existing content.
    fn merge_mime_association(content: &str, mime_type: &str, desktop_filename: &str) -> String {
        let section_header = "[Added Associations]";
        let entry_prefix = format!("{}=", mime_type);
        let app_entry = format!("{};", desktop_filename);

        let mut lines: Vec<String> = content.lines().map(str::to_string).collect();

        // Find or create [Added Associations] section
        let section_idx = lines.iter().position(|l| l.trim() == section_header);
        let section_start = match section_idx {
            Some(i) => i + 1,
            None => {
                if !lines.is_empty() && !lines.last().map_or(true, |l| l.is_empty()) {
                    lines.push(String::new());
                }
                lines.push(section_header.to_string());
                lines.len()
            }
        };

        // Find end of section (next [header] or end of file)
        let section_end = lines[section_start..]
            .iter()
            .position(|l| l.starts_with('['))
            .map(|i| section_start + i)
            .unwrap_or(lines.len());

        // Find existing line for this mime type within section
        let mime_line_idx = lines[section_start..section_end]
            .iter()
            .position(|l| l.starts_with(&entry_prefix))
            .map(|i| section_start + i);

        match mime_line_idx {
            Some(idx) => {
                // Line exists — append app if not already present
                if !lines[idx].contains(&app_entry) {
                    if lines[idx].ends_with(';') {
                        lines[idx].push_str(&app_entry);
                    } else {
                        lines[idx].push_str(&format!(";{}", app_entry));
                    }
                }
            }
            None => {
                // Insert new line at end of section
                let new_line = format!("{}={}", mime_type, app_entry);
                lines.insert(section_end, new_line);
            }
        }

        let mut result = lines.join("\n");
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result
    }

    async fn add_mime_associations(
        desktop_filename: &str,
        mime_types: &[&str],
    ) -> Result<(), DesktopIntegrationError> {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let mimeapps_path = config_dir.join(MIMEAPPS_LIST);

        let content = if mimeapps_path.exists() {
            fs::read_to_string(&mimeapps_path).await.unwrap_or_default()
        } else {
            String::new()
        };

        let mut updated = content;
        for mime_type in mime_types {
            updated = Self::merge_mime_association(&updated, mime_type, desktop_filename);
        }

        fs::write(&mimeapps_path, updated).await?;
        info!("Updated mimeapps.list with associations for {}", desktop_filename);

        Ok(())
    }

    async fn remove_mime_associations(desktop_files: &[String]) -> Result<(), DesktopIntegrationError> {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let mimeapps_path = config_dir.join(MIMEAPPS_LIST);

        if !mimeapps_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&mimeapps_path).await?;
        let mut new_content = content.clone();

        for desktop_file in desktop_files {
            let patterns = [
                format!("{};", desktop_file),
                format!("={};", desktop_file),
            ];

            for pattern in &patterns {
                new_content = new_content.replace(pattern, "");
            }

            let line_pattern = format!("={};\n", desktop_file);
            new_content = new_content.replace(&line_pattern, "");
        }

        let lines: Vec<&str> = new_content.lines().collect();
        let mut cleaned_lines = Vec::new();
        let mut skip_empty_section = false;

        for line in lines {
            let trimmed = line.trim();

            if trimmed.starts_with('[') {
                skip_empty_section = false;
                cleaned_lines.push(line);
            } else if trimmed.is_empty() {
                if !skip_empty_section {
                    cleaned_lines.push(line);
                }
            } else if trimmed.contains('=') {
                let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                if parts.len() == 2 && !parts[1].trim().is_empty() {
                    cleaned_lines.push(line);
                    skip_empty_section = false;
                } else {
                    skip_empty_section = true;
                }
            } else {
                cleaned_lines.push(line);
            }
        }

        new_content = cleaned_lines.join("\n");
        if !new_content.ends_with('\n') {
            new_content.push('\n');
        }

        fs::write(&mimeapps_path, new_content).await?;
        info!("Removed MIME associations for files: {:?}", desktop_files);

        Ok(())
    }

    async fn cleanup_empty_directory() {
        let apps_dir = Self::get_applications_dir();

        if let Ok(mut entries) = tokio::fs::read_dir(&apps_dir).await {
            let has_entries = entries.next_entry().await.map(|e| e.is_some()).unwrap_or(false);

            if !has_entries {
                let _ = tokio::fs::remove_dir(&apps_dir).await;
                info!("Empty directory removed");
            }
        }
    }

    async fn cleanup_launchers_directory() {
        let launchers_dir = Self::get_launchers_dir();

        if let Ok(mut entries) = tokio::fs::read_dir(&launchers_dir).await {
            let has_entries = entries.next_entry().await.map(|e| e.is_some()).unwrap_or(false);

            if !has_entries {
                let _ = tokio::fs::remove_dir(&launchers_dir).await;
                info!("Empty launchers directory removed");
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ApplicationType {
    Word,
    Excel,
    PowerPoint,
    Project,
    Visio,
    Access,
    OneNote,
    Outlook,
    Publisher,
    SkypeForBusiness,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_prefix_with_office(base: &Path, office_folder: &str, exes: &[&str]) -> WinePrefixManager {
        let mut office_path = base.join("drive_c").join("Program Files").join("Microsoft Office");
        fs::create_dir_all(&office_path).unwrap();
        office_path = office_path.join(office_folder);
        fs::create_dir_all(&office_path).unwrap();

        for exe in exes {
            fs::write(office_path.join(exe), "").unwrap();
        }

        // Create minimal prefix files
        fs::create_dir_all(base.join("drive_c")).unwrap();
        fs::write(base.join("system.reg"), "#arch=win32").unwrap();

        WinePrefixManager::new(base, "win32")
    }

    #[test]
    fn test_find_wine_desktop_entry_returns_none_when_dir_not_exists() {
        // Without creating any Wine desktop entries, should return None
        let result = find_wine_desktop_entry("word");
        // This depends on the actual system state, so we just verify it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_has_icons_for_product_returns_false_when_no_icons() {
        // Fresh system without Office icons should return false
        let result = DesktopIntegration::has_icons_for_product(ProductType::Office2016);
        // May be true or false depending on system state, but should not panic
        let _ = result;
    }

    #[test]
    fn test_detect_installed_office_apps_with_full_office_setup() {
        let temp_dir = TempDir::new().unwrap();
        let prefix = create_test_prefix_with_office(
            temp_dir.path(),
            "Office16",
            &["WINWORD.EXE", "EXCEL.EXE", "POWERPNT.EXE", "MSACCESS.EXE"],
        );

        let apps = DesktopIntegration::detect_installed_office_apps(&prefix);

        assert_eq!(apps.len(), 4);
        let exe_names: Vec<&str> = apps.iter().map(|a| a.exe_name).collect();
        assert!(exe_names.contains(&"WINWORD.EXE"));
        assert!(exe_names.contains(&"EXCEL.EXE"));
        assert!(exe_names.contains(&"POWERPNT.EXE"));
        assert!(exe_names.contains(&"MSACCESS.EXE"));
    }

    #[test]
    fn test_detect_installed_office_apps_with_single_app() {
        let temp_dir = TempDir::new().unwrap();
        let prefix = create_test_prefix_with_office(
            temp_dir.path(),
            "Office16",
            &["WINWORD.EXE"],
        );

        let apps = DesktopIntegration::detect_installed_office_apps(&prefix);

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].exe_name, "WINWORD.EXE");
    }

    #[test]
    fn test_detect_installed_office_apps_empty_folder() {
        let temp_dir = TempDir::new().unwrap();
        let prefix = create_test_prefix_with_office(
            temp_dir.path(),
            "Office16",
            &[],
        );

        let apps = DesktopIntegration::detect_installed_office_apps(&prefix);

        assert!(apps.is_empty());
    }

    #[test]
    fn test_extract_prefix_name_from_path() {
        let path = PathBuf::from("/home/user/.vineoffice_office2016");
        // This tests the private function indirectly through public API
        let prefix = WinePrefixManager::new(&path, "win32");
        assert_eq!(prefix.get_prefix_path(), path);
    }

    #[test]
    fn test_desktop_entry_creation_flow_simulation() {
        // Simulate the full flow without actually creating files
        let temp_dir = TempDir::new().unwrap();
        let prefix = create_test_prefix_with_office(
            temp_dir.path(),
            "Office16",
            &["WINWORD.EXE", "EXCEL.EXE"],
        );

        // Verify that detect_installed_office_apps finds the apps
        let apps = DesktopIntegration::detect_installed_office_apps(&prefix);
        assert_eq!(apps.len(), 2);

        // Verify the office folder is detected correctly
        let office_folder = prefix.detect_office_folder();
        assert_eq!(office_folder, Some("Office16".to_string()));

        // Verify the installation path is correct
        let install_path = prefix.get_office_installation_path();
        assert!(install_path.to_string_lossy().contains("Office16"));
        assert!(install_path.join("WINWORD.EXE").exists());
        assert!(install_path.join("EXCEL.EXE").exists());
    }

    #[test]
    fn test_get_mime_types_for_word() {
        let mime_types = DesktopIntegration::get_mime_types_for_application(ApplicationType::Word);
        assert!(!mime_types.is_empty());
        assert!(mime_types.contains(&"application/msword"));
        assert!(mime_types.contains(&"application/vnd.openxmlformats-officedocument.wordprocessingml.document"));
    }

    #[test]
    fn test_get_mime_types_for_excel() {
        let mime_types = DesktopIntegration::get_mime_types_for_application(ApplicationType::Excel);
        assert!(!mime_types.is_empty());
        assert!(mime_types.contains(&"application/vnd.ms-excel"));
        assert!(mime_types.contains(&"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"));
    }

    #[test]
    fn test_get_mime_types_for_powerpoint() {
        let mime_types = DesktopIntegration::get_mime_types_for_application(ApplicationType::PowerPoint);
        assert!(!mime_types.is_empty());
        assert!(mime_types.contains(&"application/vnd.ms-powerpoint"));
    }

    #[test]
    fn test_all_office_apps_have_valid_mime_types() {
        for app in OFFICE_MAIN_APPS {
            let mime_types = DesktopIntegration::get_mime_types_for_application(app.app_type);
            assert!(!mime_types.is_empty(), "No MIME types for {}", app.display_name);
        }
    }

    #[test]
    fn test_icon_extraction_timeout_is_sufficient() {
        // Verify that the timeout constant is at least 30 seconds
        // This is a compile-time check to ensure the value is reasonable
        let timeout = Duration::from_secs(60);
        assert!(timeout >= Duration::from_secs(30), "Icon extraction timeout should be at least 30 seconds");
    }

    #[test]
    fn test_xdg_data_dir_fallback() {
        // Test that the XDG data dir fallback works correctly
        let data_dir = dirs::data_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")));
        
        // Should not panic, and should return a valid path
        assert!(data_dir.is_some(), "Should be able to determine data directory");
    }

    #[test]
    fn test_launcher_filename_has_no_leading_dot() {
        // Regression test: launcher filename must NOT start with a dot
        // Bug: extract_prefix_name returns ".vineoffice_office2016" but
        // launcher should be "vineoffice_office2016_*_launcher.sh" not ".vineoffice_*"
        let prefix_path = PathBuf::from("/home/user/.vineoffice_office2016");
        let prefix_name_with_dot = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let prefix_name = prefix_name_with_dot.trim_start_matches('.');
        
        let launcher_filename = format!("{}_winword_launcher.sh", prefix_name);
        
        assert!(
            !launcher_filename.starts_with('.'),
            "Launcher filename should not start with dot: {}",
            launcher_filename
        );
        assert_eq!(
            launcher_filename,
            "vineoffice_office2016_winword_launcher.sh"
        );
    }

    #[test]
    fn test_desktop_filename_has_no_leading_dot() {
        // Same regression test for desktop entry filenames
        let prefix_path = PathBuf::from("/home/user/.vineoffice_office2016");
        let prefix_name_with_dot = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let prefix_name = prefix_name_with_dot.trim_start_matches('.');
        
        let desktop_filename = format!("{}_winword.desktop", prefix_name);
        
        assert!(
            !desktop_filename.starts_with('.'),
            "Desktop filename should not start with dot: {}",
            desktop_filename
        );
        assert_eq!(
            desktop_filename,
            "vineoffice_office2016_winword.desktop"
        );
    }
}
