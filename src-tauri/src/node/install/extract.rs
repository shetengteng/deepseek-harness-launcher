use std::path::{Path, PathBuf};

use crate::error::{LauncherError, Result};

use super::NodeArchiveKind;

pub(super) async fn extract_archive(
    archive_path: &Path,
    destination: &Path,
    kind: NodeArchiveKind,
) -> Result<()> {
    let archive_path = archive_path.to_path_buf();
    let destination = destination.to_path_buf();
    tokio::task::spawn_blocking(move || match kind {
        NodeArchiveKind::TarGz => extract_tar_gz_blocking(&archive_path, &destination),
        NodeArchiveKind::Zip => extract_zip_blocking(&archive_path, &destination),
    })
    .await
    .map_err(|error| LauncherError::NodeDownload(format!("blocking task failed: {error}")))?
}

fn extract_tar_gz_blocking(archive_path: &Path, destination: &Path) -> Result<()> {
    let file = std::fs::File::open(archive_path).map_err(LauncherError::Io)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    for entry in archive
        .entries()
        .map_err(|error| LauncherError::NodeDownload(format!("tar entries failed: {error}")))?
    {
        let mut entry = entry.map_err(|error| {
            LauncherError::NodeDownload(format!("tar entry read failed: {error}"))
        })?;
        let path = entry.path().map_err(|error| {
            LauncherError::NodeDownload(format!("tar entry path failed: {error}"))
        })?;
        let Some(target) = extraction_target(destination, &path, "tar")? else {
            continue;
        };
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(LauncherError::Io)?;
        }
        entry
            .unpack(&target)
            .map_err(|error| LauncherError::NodeDownload(format!("tar unpack failed: {error}")))?;
    }
    re_sign_macos_binaries(destination)
}

fn extract_zip_blocking(archive_path: &Path, destination: &Path) -> Result<()> {
    let file = std::fs::File::open(archive_path).map_err(LauncherError::Io)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| LauncherError::NodeDownload(format!("zip open failed: {error}")))?;
    for index in 0..archive.len() {
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
        std::io::copy(&mut entry, &mut output).map_err(LauncherError::Io)?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode))
                .map_err(LauncherError::Io)?;
        }
    }
    re_sign_macos_binaries(destination)
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

#[cfg(target_os = "macos")]
fn re_sign_macos_binaries(destination: &Path) -> Result<()> {
    use std::process::Command;

    for binary in [destination.join("bin/node"), destination.join("bin/npm")] {
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
fn re_sign_macos_binaries(_: &Path) -> Result<()> {
    Ok(())
}
