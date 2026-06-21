//! Default location for the metadata SQLite database.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("based")
        .join("metadata.db")
}

pub fn ensure_parent(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}
