use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{LauncherError, Result};

#[tauri::command]
pub async fn export_diagnostics(dest: String) -> Result<u64> {
    export_diagnostics_to(&PathBuf::from(dest))
}

pub fn export_diagnostics_to(dest: &Path) -> Result<u64> {
    use zip::write::SimpleFileOptions;

    let file = std::fs::File::create(dest)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut entries = 0_usize;
    if let Some((name, path)) = state_json_entry() {
        entries += usize::from(add_file_to_zip(&mut zip, options, &name, path)?);
    }
    for (prefix, directory) in log_dirs() {
        if let Ok(read_dir) = std::fs::read_dir(directory) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let file_name = path
                        .file_name()
                        .map(|name| name.to_string_lossy())
                        .unwrap_or_default();
                    entries += usize::from(add_file_to_zip(
                        &mut zip,
                        options,
                        &format!("{prefix}/{file_name}"),
                        path,
                    )?);
                }
            }
        }
    }
    zip.finish().map_err(|error| {
        LauncherError::Io(std::io::Error::other(format!("zip finish failed: {error}")))
    })?;
    let size = std::fs::metadata(dest)
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    tracing::info!(dest = %dest.display(), entries, size, "diagnostics exported");
    Ok(size)
}

fn state_json_entry() -> Option<(String, PathBuf)> {
    let path = crate::paths::state_file().ok()?;
    path.exists().then_some(("state.json".to_string(), path))
}

fn log_dirs() -> Vec<(&'static str, PathBuf)> {
    let mut directories = Vec::new();
    if let Ok(path) = crate::paths::log_dir() {
        directories.push(("launcher-logs", path));
    }
    if let Ok(path) = crate::paths::dsh_log_dir() {
        directories.push(("dsh-logs", path));
    }
    directories
}

fn add_file_to_zip<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    options: zip::write::SimpleFileOptions,
    name: &str,
    path: impl AsRef<Path>,
) -> Result<bool> {
    let path = path.as_ref();
    let content = match std::fs::read(path) {
        Ok(content) => content,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "skipping unreadable diagnostics file");
            return Ok(false);
        }
    };
    zip.start_file(name, options).map_err(|error| {
        LauncherError::Io(std::io::Error::other(format!("zip start_file: {error}")))
    })?;
    zip.write_all(&content)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_creates_a_readable_zip() {
        let temp = tempfile::tempdir().expect("tempdir");
        let destination = temp.path().join("diagnostics.zip");
        assert!(export_diagnostics_to(&destination).expect("export") > 0);
        zip::ZipArchive::new(std::fs::File::open(destination).expect("zip"))
            .expect("valid archive");
    }
    #[test]
    fn log_sources_cover_launcher_and_dsh_logs() {
        assert_eq!(log_dirs().len(), 2);
    }
}
