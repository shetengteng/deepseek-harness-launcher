//! Node 跨平台解压 + 原子切换（设计 §M2.3）。

mod extract;
mod paths;
mod verify;

use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use tokio::sync::mpsc;

use crate::error::{LauncherError, Result};
use crate::node::download::{DownloadCancellation, ProgressEvent};
use crate::paths as app_paths;

pub use paths::{
    current_node_dir, current_node_dir_in, current_node_version, current_node_version_in,
    node_bin_path, node_npm_path,
};
pub use verify::verify_node_binary;

/// Archive 类型。macOS/Linux 用 tar.gz，Windows 用 zip。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeArchiveKind {
    /// `.tar.gz`
    TarGz,
    /// `.zip`
    Zip,
}

impl NodeArchiveKind {
    pub fn for_platform(platform: &str) -> Self {
        if platform == "win" {
            Self::Zip
        } else {
            Self::TarGz
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::TarGz => "tar.gz",
            Self::Zip => "zip",
        }
    }
}

pub fn node_archive_filename(version: &str, platform: &str, arch: &str) -> String {
    format!(
        "node-v{version}-{platform}-{arch}.{}",
        NodeArchiveKind::for_platform(platform).extension()
    )
}

/// 推断当前平台的 archive 类型。
pub fn archive_kind_for_current_platform() -> NodeArchiveKind {
    #[cfg(target_os = "windows")]
    {
        NodeArchiveKind::Zip
    }
    #[cfg(not(target_os = "windows"))]
    {
        NodeArchiveKind::TarGz
    }
}

#[derive(Debug)]
pub(crate) struct InstalledNode {
    pub(crate) target: PathBuf,
    created: bool,
    previous_version: Option<Vec<u8>>,
    version: String,
}

const VERSION_FILE_NAME: &str = "VERSION";
static VERSION_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 安装 Node 到指定 `runtime_dir`：解压 archive 到 `runtime_dir/node-v{version}/`，更新 VERSION。
///
/// 原子性：先解压到 `runtime_dir/.staging-v{version}/`，成功后 rename 到 `node-v{version}/`。
/// 若目标目录已存在，跳过解压（旧版本保留，支持回滚）。
pub async fn install_node_to(
    archive_path: &Path,
    version: &str,
    kind: NodeArchiveKind,
    runtime_dir: &Path,
    progress_tx: Option<&mpsc::Sender<ProgressEvent>>,
) -> Result<PathBuf> {
    std::fs::create_dir_all(runtime_dir).map_err(LauncherError::Io)?;
    let target_dir = runtime_dir.join(format!("node-v{version}"));
    let created = !target_dir.exists();
    if created {
        let staging_dir = runtime_dir.join(format!(".staging-v{version}"));
        if staging_dir.exists() {
            std::fs::remove_dir_all(&staging_dir).map_err(LauncherError::Io)?;
        }
        std::fs::create_dir_all(&staging_dir).map_err(LauncherError::Io)?;
        send_extract_progress(progress_tx, None);

        let extract_result = async {
            extract::extract_archive(archive_path, &staging_dir, kind, None).await?;
            std::fs::rename(&staging_dir, &target_dir).map_err(|error| {
                LauncherError::NodeDownload(format!("rename staging to target failed: {error}"))
            })?;
            Ok(())
        }
        .await;
        if let Err(error) = extract_result {
            let _ = std::fs::remove_dir_all(&staging_dir);
            let _ = std::fs::remove_dir_all(&target_dir);
            return Err(error);
        }
    }

    if let Err(error) = activate_installed_node(runtime_dir, version, progress_tx).await {
        if created {
            let _ = std::fs::remove_dir_all(&target_dir);
        }
        return Err(error);
    }
    tracing::info!(version, target = %target_dir.display(), "node installed");
    Ok(target_dir)
}

pub(crate) async fn install_node_to_cancellable(
    archive_path: &Path,
    version: &str,
    kind: NodeArchiveKind,
    runtime_dir: &Path,
    progress_tx: Option<&mpsc::Sender<ProgressEvent>>,
    cancellation: &DownloadCancellation,
) -> Result<InstalledNode> {
    cancellation.check()?;
    std::fs::create_dir_all(runtime_dir).map_err(LauncherError::Io)?;
    let target_dir = runtime_dir.join(format!("node-v{version}"));
    let previous_version = std::fs::read(runtime_dir.join("VERSION")).ok();
    let created = !target_dir.exists();
    if created {
        let staging_dir = runtime_dir.join(format!(".staging-v{version}"));
        if staging_dir.exists() {
            std::fs::remove_dir_all(&staging_dir).map_err(LauncherError::Io)?;
        }
        std::fs::create_dir_all(&staging_dir).map_err(LauncherError::Io)?;
        send_extract_progress(progress_tx, None);

        let install_result = async {
            extract::extract_archive(archive_path, &staging_dir, kind, Some(cancellation)).await?;
            cancellation.check()?;
            std::fs::rename(&staging_dir, &target_dir).map_err(|error| {
                LauncherError::NodeDownload(format!("rename staging to target failed: {error}"))
            })?;
            Ok(())
        }
        .await;

        if let Err(error) = install_result {
            let _ = std::fs::remove_dir_all(&staging_dir);
            if matches!(error, LauncherError::NodeInstallCancelled { .. }) {
                cleanup_created_runtime(
                    runtime_dir,
                    &target_dir,
                    version,
                    previous_version.as_deref(),
                )?;
            }
            return Err(error);
        }
    }

    if let Err(error) = cancellation.check() {
        if created {
            cleanup_created_runtime(
                runtime_dir,
                &target_dir,
                version,
                previous_version.as_deref(),
            )?;
        }
        return Err(error);
    }
    if let Err(error) = activate_installed_node(runtime_dir, version, progress_tx).await {
        if created {
            cleanup_created_runtime(
                runtime_dir,
                &target_dir,
                version,
                previous_version.as_deref(),
            )?;
        }
        return Err(error);
    }
    let installed = InstalledNode {
        target: target_dir,
        created,
        previous_version,
        version: version.to_string(),
    };
    if let Err(error) = cancellation.check() {
        cleanup_cancelled_install(runtime_dir, &installed)?;
        return Err(error);
    }
    tracing::info!(version, target = %installed.target.display(), "node installed");
    Ok(installed)
}

pub(crate) fn cleanup_cancelled_install(
    runtime_dir: &Path,
    installed: &InstalledNode,
) -> Result<()> {
    restore_previous_selection(
        runtime_dir,
        &installed.version,
        installed.previous_version.as_deref(),
    )?;
    if installed.created {
        std::fs::remove_dir_all(&installed.target).map_err(LauncherError::Io)?;
    }
    Ok(())
}

fn cleanup_created_runtime(
    runtime_dir: &Path,
    target_dir: &Path,
    version: &str,
    previous_version: Option<&[u8]>,
) -> Result<()> {
    restore_previous_selection(runtime_dir, version, previous_version)?;
    std::fs::remove_dir_all(target_dir).map_err(LauncherError::Io)
}

fn restore_previous_selection(
    runtime_dir: &Path,
    version: &str,
    previous_version: Option<&[u8]>,
) -> Result<()> {
    let version_path = runtime_dir.join(VERSION_FILE_NAME);
    let current = match std::fs::read(&version_path) {
        Ok(current) => current,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(LauncherError::Io(error)),
    };
    if current != version.as_bytes() {
        return Ok(());
    }
    match previous_version {
        Some(previous_version) => write_version_pointer(runtime_dir, previous_version),
        None => match std::fs::remove_file(version_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(LauncherError::Io(error)),
        },
    }
}

fn write_version_pointer(runtime_dir: &Path, version: &[u8]) -> Result<()> {
    let version_path = runtime_dir.join(VERSION_FILE_NAME);
    let (temporary_path, mut temporary_file) = create_version_temporary_file(runtime_dir)?;
    let write_result = (|| -> std::io::Result<()> {
        temporary_file.write_all(version)?;
        temporary_file.sync_all()?;
        drop(temporary_file);
        std::fs::rename(&temporary_path, version_path)
    })();
    if let Err(error) = write_result {
        if let Err(cleanup_error) = std::fs::remove_file(&temporary_path) {
            if cleanup_error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(path = %temporary_path.display(), %cleanup_error, "failed to remove incomplete Node version pointer");
            }
        }
        return Err(LauncherError::Io(error));
    }
    Ok(())
}

fn create_version_temporary_file(runtime_dir: &Path) -> Result<(PathBuf, File)> {
    for _ in 0..16 {
        let sequence = VERSION_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = runtime_dir.join(format!(
            ".{VERSION_FILE_NAME}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(LauncherError::Io(error)),
        }
    }
    Err(LauncherError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a temporary Node version pointer",
    )))
}

/// 安装 Node 到默认 `<data>/node-runtime/`。
pub async fn install_node(
    archive_path: &Path,
    version: &str,
    kind: NodeArchiveKind,
    progress_tx: Option<&mpsc::Sender<ProgressEvent>>,
) -> Result<PathBuf> {
    install_node_to(
        archive_path,
        version,
        kind,
        &app_paths::node_runtime_dir()?,
        progress_tx,
    )
    .await
}

async fn activate_installed_node(
    runtime_dir: &Path,
    version: &str,
    progress_tx: Option<&mpsc::Sender<ProgressEvent>>,
) -> Result<()> {
    select_node_version_in(runtime_dir, version).await?;
    send_extract_progress(progress_tx, Some(0));
    Ok(())
}

/// 验证并切换到已经存在的 Node 版本目录。
pub async fn select_node_version_in(runtime_dir: &Path, version: &str) -> Result<()> {
    let target_dir = runtime_dir.join(format!("node-v{version}"));
    verify_node_binary(&target_dir, version).await?;
    write_version_pointer(runtime_dir, version.as_bytes())?;
    Ok(())
}

fn send_extract_progress(progress_tx: Option<&mpsc::Sender<ProgressEvent>>, total: Option<u64>) {
    if let Some(sender) = progress_tx {
        let _ = sender.try_send(ProgressEvent {
            stage: "extract".to_string(),
            bytes: 0,
            total,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::download::NodeDownloadOperations;

    #[cfg(unix)]
    fn write_fake_node(node_dir: &Path, version_line: &str) {
        use std::os::unix::fs::PermissionsExt;
        let bin = node_dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let path = bin.join("node");
        std::fs::write(&path, format!("#!/bin/sh\necho {version_line}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn archive_kind_for_current_platform_is_consistent() {
        #[cfg(target_os = "windows")]
        assert_eq!(archive_kind_for_current_platform(), NodeArchiveKind::Zip);
        #[cfg(not(target_os = "windows"))]
        assert_eq!(archive_kind_for_current_platform(), NodeArchiveKind::TarGz);
    }

    #[test]
    fn archive_filename_matches_official_node_dist_names() {
        assert_eq!(
            node_archive_filename("22.19.0", "darwin", "arm64"),
            "node-v22.19.0-darwin-arm64.tar.gz"
        );
        assert_eq!(
            node_archive_filename("22.19.0", "linux", "x64"),
            "node-v22.19.0-linux-x64.tar.gz"
        );
        assert_eq!(
            node_archive_filename("22.19.0", "win", "x64"),
            "node-v22.19.0-win-x64.zip"
        );
        assert_eq!(NodeArchiveKind::for_platform("win"), NodeArchiveKind::Zip);
        assert_eq!(
            NodeArchiveKind::for_platform("darwin"),
            NodeArchiveKind::TarGz
        );
    }

    #[test]
    fn version_pointer_replaces_the_previous_selection_without_temp_files() {
        let runtime = tempfile::tempdir().unwrap();
        let version_path = runtime.path().join(VERSION_FILE_NAME);
        std::fs::write(&version_path, "22.19.0").unwrap();

        write_version_pointer(runtime.path(), b"24.0.0").unwrap();

        assert_eq!(std::fs::read_to_string(version_path).unwrap(), "24.0.0");
        let temporary_files = std::fs::read_dir(runtime.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(".VERSION."))
            .count();
        assert_eq!(temporary_files, 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn selecting_previous_runtime_restores_a_valid_node_pointer() {
        let runtime = tempfile::tempdir().unwrap();
        let previous = runtime.path().join("node-v22.19.0");
        let upgraded = runtime.path().join("node-v24.4.0");
        write_fake_node(&previous, "v22.19.0");
        write_fake_node(&upgraded, "v24.4.0");
        std::fs::write(runtime.path().join(VERSION_FILE_NAME), "24.4.0").unwrap();

        select_node_version_in(runtime.path(), "22.19.0")
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(runtime.path().join(VERSION_FILE_NAME)).unwrap(),
            "22.19.0"
        );
        assert_eq!(current_node_dir_in(runtime.path()).unwrap(), previous);
    }

    #[tokio::test]
    async fn cancellation_cleans_staging_and_preserves_the_active_runtime() {
        let runtime = tempfile::tempdir().unwrap();
        let existing = runtime.path().join("node-v20.0.0");
        std::fs::create_dir_all(&existing).unwrap();
        std::fs::write(runtime.path().join("VERSION"), "20.0.0").unwrap();
        let archive = runtime.path().join("node.tar.gz");
        std::fs::write(&archive, b"not used").unwrap();
        let operations = NodeDownloadOperations::default();
        let active = operations.register("operation-1").unwrap();
        let cancellation = active.cancellation();
        assert!(operations.cancel("operation-1"));

        let error = install_node_to_cancellable(
            &archive,
            "22.0.0",
            NodeArchiveKind::TarGz,
            runtime.path(),
            None,
            &cancellation,
        )
        .await
        .unwrap_err();

        assert!(matches!(error, LauncherError::NodeInstallCancelled { .. }));
        assert!(existing.exists());
        assert_eq!(
            std::fs::read_to_string(runtime.path().join("VERSION")).unwrap(),
            "20.0.0"
        );
        assert!(!runtime.path().join(".staging-v22.0.0").exists());
        assert!(!runtime.path().join("node-v22.0.0").exists());
    }

    #[test]
    fn cancellation_after_runtime_switch_restores_the_prior_selection() {
        let runtime = tempfile::tempdir().unwrap();
        let existing = runtime.path().join("node-v20.0.0");
        let created = runtime.path().join("node-v22.0.0");
        std::fs::create_dir_all(&existing).unwrap();
        std::fs::create_dir_all(&created).unwrap();
        std::fs::write(runtime.path().join("VERSION"), "22.0.0").unwrap();

        cleanup_cancelled_install(
            runtime.path(),
            &InstalledNode {
                target: created.clone(),
                created: true,
                previous_version: Some(b"20.0.0".to_vec()),
                version: "22.0.0".to_string(),
            },
        )
        .unwrap();

        assert!(existing.exists());
        assert!(!created.exists());
        assert_eq!(
            std::fs::read_to_string(runtime.path().join("VERSION")).unwrap(),
            "20.0.0"
        );
    }

    #[test]
    fn cancellation_after_switching_to_an_existing_runtime_restores_the_prior_selection() {
        let runtime = tempfile::tempdir().unwrap();
        let existing = runtime.path().join("node-v20.0.0");
        let selected = runtime.path().join("node-v22.0.0");
        std::fs::create_dir_all(&existing).unwrap();
        std::fs::create_dir_all(&selected).unwrap();
        std::fs::write(runtime.path().join("VERSION"), "22.0.0").unwrap();

        cleanup_cancelled_install(
            runtime.path(),
            &InstalledNode {
                target: selected.clone(),
                created: false,
                previous_version: Some(b"20.0.0".to_vec()),
                version: "22.0.0".to_string(),
            },
        )
        .unwrap();

        assert!(existing.exists());
        assert!(selected.exists());
        assert_eq!(
            std::fs::read_to_string(runtime.path().join("VERSION")).unwrap(),
            "20.0.0"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn switching_runtimes_updates_version_and_keeps_the_previous_install() {
        let runtime = tempfile::tempdir().unwrap();
        let previous = runtime.path().join("node-v22.19.0");
        let next = runtime.path().join("node-v24.0.0");
        write_fake_node(&previous, "v22.19.0");
        write_fake_node(&next, "v24.0.0");
        std::fs::write(runtime.path().join("VERSION"), "22.19.0").unwrap();
        let archive = runtime.path().join("node.tar.gz");
        std::fs::write(&archive, b"not used").unwrap();
        let operations = NodeDownloadOperations::default();
        let active = operations.register("switch").unwrap();

        let installed = install_node_to_cancellable(
            &archive,
            "24.0.0",
            NodeArchiveKind::TarGz,
            runtime.path(),
            None,
            &active.cancellation(),
        )
        .await
        .unwrap();

        assert_eq!(installed.target, next);
        assert!(!installed.created);
        assert!(previous.exists());
        assert!(next.exists());
        assert_eq!(
            std::fs::read_to_string(runtime.path().join("VERSION")).unwrap(),
            "24.0.0"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_binary_check_does_not_switch_the_active_runtime() {
        let runtime = tempfile::tempdir().unwrap();
        let previous = runtime.path().join("node-v22.19.0");
        let next = runtime.path().join("node-v24.0.0");
        write_fake_node(&previous, "v22.19.0");
        write_fake_node(&next, "v20.0.0");
        std::fs::write(runtime.path().join("VERSION"), "22.19.0").unwrap();
        let archive = runtime.path().join("node.tar.gz");
        std::fs::write(&archive, b"not used").unwrap();
        let operations = NodeDownloadOperations::default();
        let active = operations.register("mismatch").unwrap();

        let error = install_node_to_cancellable(
            &archive,
            "24.0.0",
            NodeArchiveKind::TarGz,
            runtime.path(),
            None,
            &active.cancellation(),
        )
        .await
        .unwrap_err();

        assert!(matches!(error, LauncherError::NodeVersion(_)));
        assert!(previous.exists());
        assert!(next.exists());
        assert_eq!(
            std::fs::read_to_string(runtime.path().join("VERSION")).unwrap(),
            "22.19.0"
        );
    }
}
