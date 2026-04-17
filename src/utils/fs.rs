use std::path::{Path, PathBuf};

pub struct FileSystem;

impl FileSystem {
    pub fn list_directory(path: &Path) -> Vec<DirEntry> {
        let mut entries = Vec::new();

        if let Ok(reader) = std::fs::read_dir(path) {
            for entry in reader.filter_map(|e| e.ok()) {
                let file_type = entry.file_type().ok();

                entries.push(DirEntry {
                    path: entry.path(),
                    name: entry.file_name().to_string_lossy().to_string(),
                    is_dir: file_type.map(|ft| ft.is_dir()).unwrap_or(false),
                });
            }
        }

        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        entries
    }

    pub fn get_parent_dir(path: &Path) -> Option<PathBuf> {
        path.parent().map(|p| p.to_path_buf())
    }

    pub fn get_home_dir() -> PathBuf {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
    }

    pub fn get_downloads_dir() -> PathBuf {
        dirs::download_dir().unwrap_or_else(|| Self::get_home_dir().join("Downloads"))
    }
}

#[derive(Clone, Debug)]
pub struct DirEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}
