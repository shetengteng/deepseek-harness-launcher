use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::error::{LauncherError, Result};
use crate::node::download::DownloadCancellation;

use super::NodeArchiveKind;

pub(super) async fn extract_archive(
    archive_path: &Path,
    destination: &Path,
    kind: NodeArchiveKind,
    cancellation: Option<&DownloadCancellation>,
) -> Result<()> {
    let archive_path = archive_path.to_path_buf();
    let destination = destination.to_path_buf();
    let cancellation = cancellation.cloned();
    tokio::task::spawn_blocking(move || match kind {
        NodeArchiveKind::TarGz => {
            extract_tar_gz_blocking(&archive_path, &destination, cancellation.as_ref())
        }
        NodeArchiveKind::Zip => {
            extract_zip_blocking(&archive_path, &destination, cancellation.as_ref())
        }
    })
    .await
    .map_err(|error| LauncherError::NodeDownload(format!("blocking task failed: {error}")))?
}

fn extract_tar_gz_blocking(
    archive_path: &Path,
    destination: &Path,
    cancellation: Option<&DownloadCancellation>,
) -> Result<()> {
    check_cancelled(cancellation)?;
    let file = std::fs::File::open(archive_path).map_err(LauncherError::Io)?;
    let reader = CancellableReader { file, cancellation };
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(reader));
    for entry in archive
        .entries()
        .map_err(|error| cancellation_error_or(error, cancellation, "tar entries failed"))?
    {
        check_cancelled(cancellation)?;
        let mut entry = entry
            .map_err(|error| cancellation_error_or(error, cancellation, "tar entry read failed"))?;
        let path = entry
            .path()
            .map_err(|error| cancellation_error_or(error, cancellation, "tar entry path failed"))?;
        let Some(target) = extraction_target(destination, &path, "tar")? else {
            continue;
        };
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(LauncherError::Io)?;
        }
        entry
            .unpack(&target)
            .map_err(|error| cancellation_error_or(error, cancellation, "tar unpack failed"))?;
        check_cancelled(cancellation)?;
    }
    re_sign_macos_binaries(destination, cancellation)
}

fn extract_zip_blocking(
    archive_path: &Path,
    destination: &Path,
    cancellation: Option<&DownloadCancellation>,
) -> Result<()> {
    check_cancelled(cancellation)?;
    let file = std::fs::File::open(archive_path).map_err(LauncherError::Io)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| LauncherError::NodeDownload(format!("zip open failed: {error}")))?;
    for index in 0..archive.len() {
        check_cancelled(cancellation)?;
        let mut entry = archive.by_index(index).map_err(|error| {
            LauncherError::NodeDownload(format!("zip entry {index} failed: {error}"))
        })?;
        let name = entry.name().to_string();
        let Some(target) = extraction_target(destination, Path::new(&name), "zip")? else {
            continue;
        };
        if entry.is_dir() {
            std::fs::create_dir_all(target).map_err(LauncherError::Io)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(LauncherError::Io)?;
        }
        let mut output = std::fs::File::create(&target).map_err(LauncherError::Io)?;
        copy_cancellable(&mut entry, &mut output, cancellation)?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode))
                .map_err(LauncherError::Io)?;
        }
    }
    re_sign_macos_binaries(destination, cancellation)
}

fn copy_cancellable(
    reader: &mut impl Read,
    writer: &mut impl Write,
    cancellation: Option<&DownloadCancellation>,
) -> Result<()> {
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancelled(cancellation)?;
        let count = reader.read(&mut buffer).map_err(LauncherError::Io)?;
        if count == 0 {
            return Ok(());
        }
        writer
            .write_all(&buffer[..count])
            .map_err(LauncherError::Io)?;
    }
}

fn extraction_target(
    destination: &Path,
    path: &Path,
    archive_kind: &str,
) -> Result<Option<PathBuf>> {
    let relative = path.components().skip(1).collect::<PathBuf>();
    if relative.as_os_str().is_empty() {
        return Ok(None);
    }
    let target = destination.join(relative);
    if !target.starts_with(destination) {
        return Err(LauncherError::NodeDownload(format!(
            "{archive_kind} entry path escapes dest_dir: {}",
            path.display()
        )));
    }
    Ok(Some(target))
}

struct CancellableReader<'a> {
    file: std::fs::File,
    cancellation: Option<&'a DownloadCancellation>,
}

impl Read for CancellableReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        check_cancelled(self.cancellation).map_err(|error| {
            std::io::Error::new(std::io::ErrorKind::Interrupted, error.to_string())
        })?;
        self.file.read(buffer)
    }
}

fn check_cancelled(cancellation: Option<&DownloadCancellation>) -> Result<()> {
    cancellation.map_or(Ok(()), DownloadCancellation::check)
}

fn cancellation_error_or(
    error: impl std::fmt::Display,
    cancellation: Option<&DownloadCancellation>,
    context: &str,
) -> LauncherError {
    cancellation
        .and_then(|cancellation| cancellation.check().err())
        .unwrap_or_else(|| LauncherError::NodeDownload(format!("{context}: {error}")))
}

#[cfg(target_os = "macos")]
fn re_sign_macos_binaries(
    destination: &Path,
    cancellation: Option<&DownloadCancellation>,
) -> Result<()> {
    use std::process::Command;

    for binary in [destination.join("bin/node"), destination.join("bin/npm")] {
        check_cancelled(cancellation)?;
        if !binary.exists() {
            continue;
        }
        let output = Command::new("codesign")
            .args(["--force", "--sign", "-"])
            .arg(&binary)
            .output()
            .map_err(|error| {
                LauncherError::NodeDownload(format!(
                    "failed to invoke codesign for {}: {error}",
                    binary.display()
                ))
            })?;
        check_cancelled(cancellation)?;
        if !output.status.success() {
            return Err(LauncherError::NodeDownload(format!(
                "codesign failed for {}: {} (exit code: {})",
                binary.display(),
                String::from_utf8_lossy(&output.stderr).trim(),
                output.status
            )));
        }
        tracing::debug!(target = %binary.display(), "re-signed binary for macOS AMFI");
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn re_sign_macos_binaries(_: &Path, cancellation: Option<&DownloadCancellation>) -> Result<()> {
    check_cancelled(cancellation)
}
