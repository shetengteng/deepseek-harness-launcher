//! dsh 安装协调。

#[path = "install/cancellation.rs"]
mod cancellation;
#[path = "install/process.rs"]
mod process;

use std::path::{Path, PathBuf};

use crate::error::{LauncherError, Result};
use crate::node::install::node_bin_path;

use super::integrity::verify_entry_exists;
use super::registry::PackageManifest;

pub use cancellation::{DshInstallCancellation, DshInstallOperations};
pub use process::{install_dsh, install_dsh_cancellable, run_npm_install};

pub const DSH_PACKAGE_NAME: &str = "@deepseek-ai/dsh";

pub type LogCallback = std::sync::Arc<dyn Fn(&str) + Send + Sync>;

#[derive(Clone)]
pub struct InstallDshOptions {
    pub version: String,
    pub registry: String,
    pub dsh_dir: PathBuf,
    pub node_executable: PathBuf,
    pub npm_script: Option<PathBuf>,
    pub npm_cache: Option<PathBuf>,
    pub npm_userconfig: Option<PathBuf>,
    pub log: Option<LogCallback>,
    pub timeout_secs: u64,
}

impl std::fmt::Debug for InstallDshOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallDshOptions")
            .field("version", &self.version)
            .field("registry", &self.registry)
            .field("dsh_dir", &self.dsh_dir)
            .field("node_executable", &self.node_executable)
            .field("npm_script", &self.npm_script)
            .field("npm_cache", &self.npm_cache)
            .field("npm_userconfig", &self.npm_userconfig)
            .field("log", &self.log.as_ref().map(|_| "<callback>"))
            .field("timeout_secs", &self.timeout_secs)
            .finish()
    }
}

impl InstallDshOptions {
    pub fn version_dir(&self) -> PathBuf {
        self.dsh_dir.join(&self.version)
    }

    pub fn package_json_path(&self) -> PathBuf {
        self.version_dir().join("package.json")
    }

    pub fn dsh_module_dir(&self) -> PathBuf {
        self.version_dir()
            .join("node_modules")
            .join("@deepseek-ai")
            .join("dsh")
    }

    pub fn cli_entry_path(&self) -> PathBuf {
        self.dsh_module_dir().join(super::integrity::DSH_ENTRY_REL)
    }
}

pub fn default_log() -> LogCallback {
    std::sync::Arc::new(|line: &str| tracing::info!(target: "dsh_install", "{line}"))
}

pub fn write_package_json(opts: &InstallDshOptions) -> Result<()> {
    std::fs::create_dir_all(opts.version_dir()).map_err(LauncherError::Io)?;
    let package = serde_json::json!({
        "name": "deepseek-harness-launcher-host",
        "version": "0.0.0",
        "private": true,
        "dependencies": { DSH_PACKAGE_NAME: opts.version }
    });
    std::fs::write(
        opts.package_json_path(),
        serde_json::to_vec_pretty(&package)?,
    )
    .map_err(LauncherError::Io)
}

pub fn verify_install(opts: &InstallDshOptions) -> Result<()> {
    verify_entry_exists(&opts.dsh_module_dir())
}

pub fn cli_entry(version_dir: &Path) -> PathBuf {
    version_dir
        .join("node_modules")
        .join("@deepseek-ai")
        .join("dsh")
        .join(super::integrity::DSH_ENTRY_REL)
}

pub fn read_cli_entry(version_dir: &Path) -> Result<PathBuf> {
    let entry = cli_entry(version_dir);
    if entry.exists() {
        Ok(entry)
    } else {
        Err(LauncherError::DshInstall(format!(
            "dsh cli entry not found: {} (is dsh installed?)",
            entry.display()
        )))
    }
}

pub fn options_from_manifest(
    manifest: &PackageManifest,
    registry: &str,
    dsh_dir: &Path,
    node_dir: &Path,
) -> InstallDshOptions {
    InstallDshOptions {
        version: manifest.version.clone(),
        registry: registry.to_string(),
        dsh_dir: dsh_dir.to_path_buf(),
        node_executable: node_bin_path(node_dir),
        npm_script: Some(crate::node::install::node_npm_path(node_dir)),
        npm_cache: Some(managed_npm_cache_dir(node_dir)),
        npm_userconfig: Some(managed_npm_userconfig(node_dir)),
        log: None,
        timeout_secs: 0,
    }
}

fn managed_runtime_dir(node_dir: &Path) -> PathBuf {
    node_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| node_dir.to_path_buf())
}

pub(crate) fn managed_npm_cache_dir(node_dir: &Path) -> PathBuf {
    managed_runtime_dir(node_dir).join("npm-cache")
}

pub(crate) fn managed_npm_userconfig(node_dir: &Path) -> PathBuf {
    managed_runtime_dir(node_dir).join("npmrc")
}

#[cfg(test)]
#[path = "install/tests.rs"]
mod tests;
