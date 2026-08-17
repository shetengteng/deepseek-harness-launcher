use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{LauncherError, Result};

const MAX_DSH_SESSION_LOGS: usize = 3;

#[tauri::command]
pub async fn export_diagnostics(dest: String) -> Result<u64> {
    export_diagnostics_to(&PathBuf::from(dest))
}

pub fn export_diagnostics_to(dest: &Path) -> Result<u64> {
    let state_file = crate::paths::state_file().ok();
    let launcher_log_dir = crate::paths::log_dir().ok();
    let dsh_log_dir = crate::paths::dsh_log_dir().ok();
    export_diagnostics_from(
        dest,
        state_file.as_deref(),
        launcher_log_dir.as_deref(),
        dsh_log_dir.as_deref(),
    )
}

fn export_diagnostics_from(
    dest: &Path,
    state_file: Option<&Path>,
    launcher_log_dir: Option<&Path>,
    dsh_log_dir: Option<&Path>,
) -> Result<u64> {
    use zip::write::SimpleFileOptions;

    let file = std::fs::File::create(dest)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut entries = 0_usize;
    if let Some(path) = state_file.filter(|path| path.exists()) {
        entries += usize::from(add_file_to_zip(&mut zip, options, "state.json", path)?);
    }
    if let Some(directory) = launcher_log_dir {
        entries += add_launcher_logs(&mut zip, options, directory)?;
    }
    if let Some(directory) = dsh_log_dir {
        entries += add_recent_dsh_logs(&mut zip, options, directory)?;
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

fn add_launcher_logs<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    options: zip::write::SimpleFileOptions,
    directory: &Path,
) -> Result<usize> {
    let mut entries = 0;
    for path in regular_files(directory) {
        if is_dsh_session_log(&path) {
            continue;
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        entries += usize::from(add_file_to_zip(
            zip,
            options,
            &format!("launcher-logs/{name}"),
            path,
        )?);
    }
    Ok(entries)
}

fn add_recent_dsh_logs<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    options: zip::write::SimpleFileOptions,
    directory: &Path,
) -> Result<usize> {
    let mut entries = 0;
    for path in dsh_session_logs(directory)
        .into_iter()
        .take(MAX_DSH_SESSION_LOGS)
    {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        entries += usize::from(add_file_to_zip(
            zip,
            options,
            &format!("dsh-logs/{name}"),
            path,
        )?);
    }
    Ok(entries)
}

fn dsh_session_logs(directory: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = regular_files(directory)
        .into_iter()
        .filter(|path| is_dsh_session_log(path))
        .collect();
    paths.sort_unstable_by(|left, right| right.file_name().cmp(&left.file_name()));
    paths
}

fn regular_files(directory: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect()
}

fn is_dsh_session_log(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("dsh-") && name.ends_with(".log"))
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

    fn zip_entries(path: &Path) -> Vec<String> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).expect("zip")).unwrap();
        let mut names = (0..zip.len())
            .map(|index| zip.by_index(index).unwrap().name().to_string())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names
    }

    #[test]
    fn export_creates_a_readable_zip() {
        let temp = tempfile::tempdir().expect("tempdir");
        let destination = temp.path().join("diagnostics.zip");
        assert!(export_diagnostics_to(&destination).expect("export") > 0);
        zip::ZipArchive::new(std::fs::File::open(destination).expect("zip"))
            .expect("valid archive");
    }

    #[test]
    fn diagnostics_only_collect_newest_three_dsh_session_logs() {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("diagnostics.zip");
        let state = temp.path().join("state.json");
        let launcher_logs = temp.path().join("launcher");
        let dsh_logs = temp.path().join("dsh");
        std::fs::create_dir_all(&launcher_logs).unwrap();
        std::fs::create_dir_all(&dsh_logs).unwrap();
        std::fs::write(&state, "{}\n").unwrap();
        std::fs::write(launcher_logs.join("app.log"), "launcher\n").unwrap();
        for suffix in ["01", "02", "03", "04"] {
            std::fs::write(
                dsh_logs.join(format!("dsh-20260817T0504{suffix}.000Z.log")),
                suffix,
            )
            .unwrap();
        }
        std::fs::write(dsh_logs.join("dsh-not-a-session.txt"), "ignored").unwrap();

        export_diagnostics_from(
            &destination,
            Some(&state),
            Some(&launcher_logs),
            Some(&dsh_logs),
        )
        .unwrap();

        assert_eq!(
            zip_entries(&destination),
            vec![
                "dsh-logs/dsh-20260817T050402.000Z.log",
                "dsh-logs/dsh-20260817T050403.000Z.log",
                "dsh-logs/dsh-20260817T050404.000Z.log",
                "launcher-logs/app.log",
                "state.json",
            ],
        );
    }
}
