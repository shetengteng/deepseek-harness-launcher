use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use tauri::{AppHandle, Emitter};
use tokio::process::Command;

use crate::error::{LauncherError, Result};

use super::inventory::{list_profiles, read_profile, validate_available_profile};
use super::types::{
    MarketplaceOperation, MarketplaceOperationEvent, MarketplaceOperationKind,
    MarketplaceOperationPhase,
};

pub struct MarketplaceOperations {
    active: AtomicBool,
}

impl Default for MarketplaceOperations {
    fn default() -> Self {
        Self {
            active: AtomicBool::new(false),
        }
    }
}

impl MarketplaceOperations {
    pub fn acquire(&self) -> Result<MarketplaceOperationGuard<'_>> {
        self.active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| MarketplaceOperationGuard { operations: self })
            .map_err(|_| {
                LauncherError::Marketplace(
                    "another plugin operation is already running".to_string(),
                )
            })
    }
}

pub struct MarketplaceOperationGuard<'a> {
    operations: &'a MarketplaceOperations,
}

impl Drop for MarketplaceOperationGuard<'_> {
    fn drop(&mut self) {
        self.operations.active.store(false, Ordering::Release);
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CustomInstallPreview {
    pub profile: String,
    pub source: String,
    pub dsh_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCustomInstall {
    pub profile: String,
    pub source: String,
}

pub fn parse_custom_install(command: &str, profiles: &[String]) -> Result<ParsedCustomInstall> {
    let command = command.trim();
    if command.is_empty() || command.chars().any(char::is_control) {
        return Err(custom_command_error());
    }
    let parts = command
        .split(|character| character == ' ' || character == '\t')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let ["dsh", "plugin", "--profile", profile, "add", source] = parts.as_slice() else {
        return Err(custom_command_error());
    };
    validate_available_profile(profile, profiles)
        .map_err(|_| LauncherError::Marketplace(format!("profile is unavailable: {profile}")))?;
    validate_source(source)?;
    Ok(ParsedCustomInstall {
        profile: (*profile).to_string(),
        source: (*source).to_string(),
    })
}

pub fn custom_preview(command: &str) -> Result<CustomInstallPreview> {
    let profiles = list_profiles()?;
    let parsed = parse_custom_install(command, &profiles)?;
    Ok(CustomInstallPreview {
        profile: parsed.profile,
        source: parsed.source,
        dsh_version: current_dsh_version()?,
    })
}

pub async fn install_catalog_plugin(
    app: &AppHandle,
    plugin_id: String,
    profile: String,
    source: String,
) -> Result<MarketplaceOperation> {
    run_dsh_operation(
        app,
        MarketplaceOperationKind::Install,
        plugin_id,
        profile,
        "add",
        source,
        true,
    )
    .await
}

pub async fn install_custom_plugin(app: &AppHandle, command: &str) -> Result<MarketplaceOperation> {
    let parsed = parse_custom_install(command, &list_profiles()?)?;
    let plugin_id = parsed.source.clone();
    run_dsh_operation(
        app,
        MarketplaceOperationKind::CustomInstall,
        plugin_id,
        parsed.profile,
        "add",
        parsed.source,
        true,
    )
    .await
}

pub async fn remove_plugin(
    app: &AppHandle,
    installation_id: String,
    profile: String,
) -> Result<MarketplaceOperation> {
    let inventory = read_profile(&profile)?;
    let installed = inventory
        .into_iter()
        .find(|plugin| plugin.installation_id == installation_id)
        .ok_or_else(|| {
            LauncherError::Marketplace(
                "plugin installation is no longer present in this profile".to_string(),
            )
        })?;
    run_dsh_operation(
        app,
        MarketplaceOperationKind::Remove,
        installed.installation_id,
        profile,
        "remove",
        installed.name,
        false,
    )
    .await
}

async fn run_dsh_operation(
    app: &AppHandle,
    kind: MarketplaceOperationKind,
    plugin_id: String,
    profile: String,
    action: &str,
    source: String,
    expect_present: bool,
) -> Result<MarketplaceOperation> {
    validate_available_profile(&profile, &list_profiles()?)?;
    let log_path = create_log_path()?;
    let mut operation = MarketplaceOperation {
        id: format!("marketplace-{}", Utc::now().timestamp_millis()),
        kind,
        plugin_id,
        profile: profile.clone(),
        phase: MarketplaceOperationPhase::Preparing,
        message: "准备中".to_string(),
        log_path: Some(log_path.to_string_lossy().into_owned()),
    };
    emit(app, &operation);
    operation.phase = MarketplaceOperationPhase::Running;
    operation.message = if kind == MarketplaceOperationKind::Remove {
        "正在卸载"
    } else {
        "正在安装"
    }
    .to_string();
    emit(app, &operation);
    let output = match run_dsh_plugin(&profile, action, &source).await {
        Ok(output) => output,
        Err(error) => {
            operation.phase = MarketplaceOperationPhase::Failed;
            operation.message = "操作失败".to_string();
            emit(app, &operation);
            return Err(error);
        }
    };
    write_log(&log_path, &output.stdout, &output.stderr)?;
    if !output.status.success() {
        let summary = sanitized_summary(&output.stderr);
        operation.phase = MarketplaceOperationPhase::Failed;
        operation.message = "操作失败".to_string();
        emit(app, &operation);
        return Err(LauncherError::Marketplace(format!(
            "plugin command failed: {summary}"
        )));
    }
    operation.phase = MarketplaceOperationPhase::Verifying;
    operation.message = "正在核验".to_string();
    emit(app, &operation);
    let inventory = read_profile(&profile)?;
    let verified = if expect_present {
        inventory.iter().any(|plugin| plugin.source == source)
    } else {
        inventory
            .iter()
            .all(|plugin| plugin.installation_id != source)
    };
    if !verified {
        operation.phase = MarketplaceOperationPhase::Failed;
        operation.message = "未能核验操作结果".to_string();
        emit(app, &operation);
        return Err(LauncherError::Marketplace(
            "plugin command completed but the profile could not confirm the expected state"
                .to_string(),
        ));
    }
    operation.phase = MarketplaceOperationPhase::Succeeded;
    operation.message = match kind {
        MarketplaceOperationKind::Remove => "已卸载",
        _ => "已安装",
    }
    .to_string();
    emit(app, &operation);
    Ok(operation)
}

async fn run_dsh_plugin(profile: &str, action: &str, source: &str) -> Result<std::process::Output> {
    let data_dir = crate::paths::data_dir()?;
    let node_dir = crate::node::install::current_node_dir()?;
    let node = crate::node::install::node_bin_path(&node_dir);
    if !node.is_file() {
        return Err(LauncherError::NodeNotInstalled {
            reason: "managed Node binary is missing".to_string(),
        });
    }
    let version = current_dsh_version()?;
    let dsh_dir = crate::paths::dsh_dir()?;
    let cwd = dsh_dir.join(&version);
    let entry = crate::dsh::cli_entry(&cwd);
    if !entry.is_file() {
        return Err(LauncherError::DshNotInstalled {
            reason: "managed dsh entry is missing".to_string(),
        });
    }
    let managed_bin = crate::cli_shim::prepare_runtime_support(&data_dir)?;
    let mut environment = crate::host::filtered_env();
    let existing_path = environment.get("PATH").cloned().unwrap_or_default();
    let path = std::env::join_paths(
        std::iter::once(managed_bin).chain(std::env::split_paths(&existing_path)),
    )
    .map_err(|error| {
        LauncherError::Marketplace(format!("could not prepare plugin PATH: {error}"))
    })?;
    environment.insert("PATH".to_string(), path.to_string_lossy().into_owned());
    let entry = entry.to_str().ok_or_else(|| {
        LauncherError::Marketplace("managed dsh entry path is invalid".to_string())
    })?;
    let mut command = Command::new(node);
    command.current_dir(cwd).env_clear().args([
        "--expose-internals",
        entry,
        "plugin",
        "--profile",
        profile,
        action,
        source,
    ]);
    for (key, value) in environment {
        command.env(key, value);
    }
    command.output().await.map_err(LauncherError::Io)
}

fn current_dsh_version() -> Result<String> {
    crate::dsh::read_current_pointer(&crate::paths::dsh_dir()?)?.ok_or_else(|| {
        LauncherError::DshNotInstalled {
            reason: "dsh/current pointer is not set".to_string(),
        }
    })
}

fn validate_source(source: &str) -> Result<()> {
    let invalid = source.is_empty()
        || source.len() > 2_048
        || source.starts_with('-')
        || source.bytes().any(|byte| {
            byte.is_ascii_whitespace()
                || byte.is_ascii_control()
                || matches!(
                    byte,
                    b'\''
                        | b'"'
                        | b'`'
                        | b'$'
                        | b'|'
                        | b';'
                        | b'<'
                        | b'>'
                        | b'('
                        | b')'
                        | b'{'
                        | b'}'
                        | b'['
                        | b']'
                )
        });
    (!invalid).then_some(()).ok_or_else(custom_command_error)
}

fn custom_command_error() -> LauncherError {
    LauncherError::Marketplace("custom command must be `dsh plugin --profile <profile> add <source>` with one safe source argument".to_string())
}

fn create_log_path() -> Result<PathBuf> {
    let directory = crate::paths::dsh_log_dir()?;
    std::fs::create_dir_all(&directory)?;
    Ok(directory.join(format!(
        "marketplace-{}.log",
        Utc::now().format("%Y%m%dT%H%M%S%.3fZ")
    )))
}

fn write_log(path: &Path, stdout: &[u8], stderr: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut log = std::fs::File::create(path)?;
    log.write_all(b"[stdout]\n")?;
    log.write_all(stdout)?;
    log.write_all(b"\n[stderr]\n")?;
    log.write_all(stderr)?;
    Ok(())
}

fn sanitized_summary(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let output = text
        .chars()
        .filter(|character| !character.is_control() || *character == '\n')
        .collect::<String>();
    let output = output.trim().replace('\n', " ");
    if output.is_empty() {
        "dsh exited with an error".to_string()
    } else {
        output.chars().take(600).collect()
    }
}

fn emit(app: &AppHandle, operation: &MarketplaceOperation) {
    let _ = app.emit(
        "marketplace://operation",
        MarketplaceOperationEvent {
            operation: operation.clone(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_command_accepts_a_single_source_without_rewriting_it() {
        let profiles = vec!["web".to_string()];
        let parsed = parse_custom_install(
            "dsh plugin --profile web add github:owner/repo#path:plugin",
            &profiles,
        )
        .unwrap();
        assert_eq!(parsed.source, "github:owner/repo#path:plugin");
    }

    #[test]
    fn custom_command_rejects_shell_syntax_and_unknown_profiles() {
        let profiles = vec!["web".to_string()];
        for command in [
            "dsh plugin --profile web add repo --save-dev",
            "dsh plugin --profile web add repo | sh",
            "dsh plugin --profile web add 'repo'",
            "dsh plugin --profile other add repo",
        ] {
            assert!(
                parse_custom_install(command, &profiles).is_err(),
                "{command}"
            );
        }
    }

    #[test]
    fn single_operation_lock_rejects_concurrent_mutations() {
        let operations = MarketplaceOperations::default();
        let guard = operations.acquire().unwrap();
        assert!(operations.acquire().is_err());
        drop(guard);
        assert!(operations.acquire().is_ok());
    }

    #[test]
    fn stderr_summary_is_bounded_and_strips_control_characters() {
        let summary = sanitized_summary(b"\x1b[31mfailed\n");
        assert!(!summary.contains('\x1b'));
        assert!(summary.contains("failed"));
    }
}
