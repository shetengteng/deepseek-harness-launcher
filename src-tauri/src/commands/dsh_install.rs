use serde::Serialize;
use tauri::Emitter;

use crate::commands::bootstrap::latest_dsh_version;
use crate::error::{LauncherError, Result};
use crate::state::{AppState, StateStatus};

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub struct DshInstallProgressPayload {
    pub stage: &'static str,
}

fn dsh_install_stage_from_npm_log(line: &str) -> Option<&'static str> {
    if line.contains("npm http fetch") {
        Some("downloading")
    } else if line.contains("npm info run") {
        Some("installing")
    } else {
        None
    }
}

#[tauri::command]
pub async fn install_dsh_command(
    app: tauri::AppHandle,
    defer_activation: Option<bool>,
) -> Result<String> {
    use crate::dsh::{
        default_client, fetch_package_metadata, install_dsh, options_from_manifest,
        promote_to_current, set_pending, RegistryCache,
    };
    let mut state = match AppState::load()? {
        StateStatus::FirstRun => {
            return Err(LauncherError::NodeNotInstalled {
                reason: "state.json not found; complete first-run wizard first".to_string(),
            })
        }
        StateStatus::Loaded(state) => *state,
    };
    let node_state = state
        .node
        .clone()
        .ok_or_else(|| LauncherError::NodeNotInstalled {
            reason: "state.node is None; complete first-run wizard first".to_string(),
        })?;
    let node_dir = crate::node::install::current_node_dir()?;
    let node_executable = crate::node::install::node_bin_path(&node_dir);
    if !node_executable.exists() {
        return Err(LauncherError::NodeNotInstalled {
            reason: format!(
                "node binary not found: {} (VERSION says {} but binary missing)",
                node_executable.display(),
                node_state.version
            ),
        });
    }
    let bootstrap_plan = state.bootstrap_plan.clone();
    let registry = bootstrap_plan
        .as_ref()
        .map(|plan| plan.registry.clone())
        .unwrap_or_else(|| state.dsh.registry.clone());
    let client = default_client();
    let _ = app.emit(
        "dsh-install-progress",
        DshInstallProgressPayload { stage: "resolving" },
    );
    let metadata = fetch_package_metadata(&registry, &RegistryCache::new(), &client).await?;
    let version = match bootstrap_plan.as_ref() {
        Some(plan) => plan.dsh_version.clone(),
        None => latest_dsh_version(&metadata)?,
    };
    let manifest = metadata.manifest_for(&version)?;
    if let Some(plan) = bootstrap_plan.as_ref() {
        let manifest_engines =
            (!manifest.engines.node.trim().is_empty()).then_some(manifest.engines.node.as_str());
        if manifest_engines != plan.engines_node.as_deref() {
            return Err(LauncherError::DshRegistry(format!(
                "frozen dsh {version} manifest engines.node changed after bootstrap resolution"
            )));
        }
        if let Some(requirement) = plan.engines_node.as_deref() {
            if !crate::node::current_node_satisfies(Some(&node_state.version), requirement)? {
                return Err(LauncherError::NodeVersion(format!(
                    "installed Node {} does not satisfy frozen dsh requirement {requirement}",
                    node_state.version
                )));
            }
        }
    } else if !manifest.engines.node.trim().is_empty()
        && !crate::node::current_node_satisfies(Some(&node_state.version), &manifest.engines.node)?
    {
        return Err(LauncherError::NodeVersion(format!(
            "dsh {version} requires Node {}, current version is {}",
            manifest.engines.node, node_state.version
        )));
    }
    let _ = app.emit(
        "dsh-install-progress",
        DshInstallProgressPayload {
            stage: "downloading",
        },
    );
    let dsh_dir = crate::paths::dsh_dir()?;
    let mut options = options_from_manifest(&manifest, &registry, &dsh_dir, &node_dir);
    options.node_executable = node_executable;
    options.npm_script = Some(crate::node::install::node_npm_path(&node_dir));
    let app_for_log = app.clone();
    options.log = Some(std::sync::Arc::new(move |line| {
        tracing::info!(target: "dsh_install", "{line}");
        if let Some(stage) = dsh_install_stage_from_npm_log(line) {
            let _ = app_for_log.emit("dsh-install-progress", DshInstallProgressPayload { stage });
        }
    }));
    install_dsh(&options).await?;
    let _ = app.emit(
        "dsh-install-progress",
        DshInstallProgressPayload { stage: "verifying" },
    );
    if defer_activation.unwrap_or(false) {
        set_pending(&mut state, &version);
    } else {
        promote_to_current(&mut state, &dsh_dir, &version)?;
        state.bootstrap_plan = None;
    }
    state.save()?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn npm_output_maps_to_progress_stages() {
        assert_eq!(
            dsh_install_stage_from_npm_log("npm http fetch GET"),
            Some("downloading")
        );
        assert_eq!(
            dsh_install_stage_from_npm_log("npm info run prepare"),
            Some("installing")
        );
        assert_eq!(dsh_install_stage_from_npm_log("npm ok"), None);
    }
}
