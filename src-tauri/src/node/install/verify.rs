use std::path::Path;
use std::process::Stdio;

use crate::error::{LauncherError, Result};
use crate::node::parse_node_version;

use super::paths::node_bin_path;

pub async fn verify_node_binary(node_dir: &Path, expected_version: &str) -> Result<()> {
    let binary = node_bin_path(node_dir);
    if !binary.is_file() {
        return Err(LauncherError::NodeVersion(format!(
            "node binary missing after install: {}",
            binary.display()
        )));
    }

    let mut command = tokio::process::Command::new(&binary);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let output = command.output().await.map_err(|error| {
        LauncherError::NodeVersion(format!(
            "failed to execute {} --version: {error}",
            binary.display()
        ))
    })?;
    if !output.status.success() {
        return Err(LauncherError::NodeVersion(format!(
            "{} --version failed: {}",
            binary.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let reported = String::from_utf8_lossy(&output.stdout);
    let actual = parse_node_version(reported.trim())?;
    let expected = parse_node_version(expected_version)?;
    if actual != expected {
        return Err(LauncherError::NodeVersion(format!(
            "installed node --version is {actual}, expected {expected}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn write_fake_node(node_dir: &Path, version_line: &str) {
        use std::os::unix::fs::PermissionsExt;
        let bin = node_dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let path = bin.join("node");
        std::fs::write(&path, format!("#!/bin/sh\necho {version_line}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[tokio::test]
    async fn missing_binary_fails() {
        let directory = tempfile::tempdir().unwrap();
        let error = verify_node_binary(directory.path(), "22.19.0")
            .await
            .unwrap_err();
        assert!(matches!(error, LauncherError::NodeVersion(_)));
        assert!(error.to_string().contains("missing"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn matching_version_passes() {
        let directory = tempfile::tempdir().unwrap();
        write_fake_node(directory.path(), "v22.19.0");
        verify_node_binary(directory.path(), "22.19.0")
            .await
            .unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn mismatched_version_fails() {
        let directory = tempfile::tempdir().unwrap();
        write_fake_node(directory.path(), "v20.0.0");
        let error = verify_node_binary(directory.path(), "22.19.0")
            .await
            .unwrap_err();
        assert!(matches!(error, LauncherError::NodeVersion(_)));
        assert!(error.to_string().contains("expected 22.19.0"));
    }
}
