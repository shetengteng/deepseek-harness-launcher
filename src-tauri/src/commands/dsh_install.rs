use serde::Serialize;
use tauri::{Emitter, State};

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
    operations: State<'_, crate::dsh::DshInstallOperations>,
    operation_id: Option<String>,
    expected_version: Option<String>,
) -> Result<String> {
    use crate::dsh::{
        default_client, fetch_package_metadata, install_dsh_cancellable, options_from_manifest,
        promote_to_current, RegistryCache,
    };
    let active_install = operation_id
        .as_deref()
        .map(|operation_id| operations.register(operation_id))
        .transpose()?;
    let cancellation = active_install.as_ref().map(|active| active.cancellation());
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
    if let Some(cancellation) = cancellation.as_ref() {
        cancellation.check()?;
    }
    let version = selected_install_version(bootstrap_plan.as_ref(), expected_version.as_deref())?;
    let manifest = metadata.manifest_for(&version)?;
    ensure_node_compatible(
        bootstrap_plan.as_ref(),
        &manifest,
        &node_state.version,
        &version,
    )?;
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
    install_dsh_cancellable(&options, cancellation.as_ref()).await?;
    let _ = app.emit(
        "dsh-install-progress",
        DshInstallProgressPayload { stage: "verifying" },
    );
    promote_to_current(&mut state, &dsh_dir, &version)?;
    state.bootstrap_plan = None;
    state.save()?;
    Ok(version)
}

fn ensure_node_compatible(
    bootstrap_plan: Option<&crate::state::BootstrapPlan>,
    manifest: &crate::dsh::PackageManifest,
    node_version: &str,
    dsh_version: &str,
) -> Result<()> {
    if let Some(plan) = bootstrap_plan {
        let manifest_engines =
            (!manifest.engines.node.trim().is_empty()).then_some(manifest.engines.node.as_str());
        if manifest_engines != plan.engines_node.as_deref() {
            return Err(LauncherError::DshRegistry(format!(
                "frozen dsh {dsh_version} manifest engines.node changed after bootstrap resolution"
            )));
        }
        if let Some(requirement) = plan.engines_node.as_deref() {
            if !crate::node::current_node_satisfies(Some(node_version), requirement)? {
                return Err(LauncherError::NodeVersion(format!(
                    "installed Node {} does not satisfy frozen dsh requirement {requirement}",
                    node_version
                )));
            }
        }
    } else if !manifest.engines.node.trim().is_empty()
        && !crate::node::current_node_satisfies(Some(node_version), &manifest.engines.node)?
    {
        return Err(LauncherError::NodeVersion(format!(
            "dsh {dsh_version} requires Node {}, current version is {node_version}",
            manifest.engines.node
        )));
    }
    Ok(())
}

fn selected_install_version(
    bootstrap_plan: Option<&crate::state::BootstrapPlan>,
    expected_version: Option<&str>,
) -> Result<String> {
    match bootstrap_plan {
        Some(plan) => {
            if let Some(expected_version) = expected_version {
                if expected_version != plan.dsh_version {
                    return Err(LauncherError::DshRegistry(format!(
                        "expected dsh {expected_version} does not match frozen bootstrap version {}",
                        plan.dsh_version
                    )));
                }
            }
            Ok(plan.dsh_version.clone())
        }
        None => expected_version.map(str::to_owned).ok_or_else(|| {
            LauncherError::DshRegistry(
                "select a displayed dsh version before starting an update".to_string(),
            )
        }),
    }
}

#[tauri::command]
pub fn cancel_dsh_install_command(
    operations: State<'_, crate::dsh::DshInstallOperations>,
    operation_id: String,
) -> bool {
    operations.cancel(&operation_id)
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

    #[test]
    fn update_requires_the_displayed_version() {
        let error = selected_install_version(None, None).unwrap_err();
        assert!(error.to_string().contains("select a displayed"));
        assert_eq!(
            selected_install_version(None, Some("0.2.0")).unwrap(),
            "0.2.0"
        );
    }

    #[test]
    fn bootstrap_reuses_its_frozen_version() {
        let plan = crate::state::BootstrapPlan {
            dsh_version: "0.2.0".to_string(),
            registry: "https://registry.npmjs.org".to_string(),
            engines_node: None,
            node_version: "22.0.0".to_string(),
            requirement_source: "launcher-verified-fallback".to_string(),
            resolved_at: chrono::Utc::now(),
            phase: "resolved".to_string(),
        };

        assert_eq!(
            selected_install_version(Some(&plan), None).unwrap(),
            "0.2.0"
        );
        assert!(selected_install_version(Some(&plan), Some("0.3.0")).is_err());
    }

    #[test]
    fn incompatible_node_rejects_an_update_before_current_is_promoted() {
        let manifest = crate::dsh::PackageManifest {
            version: "0.2.0".to_string(),
            engines: crate::dsh::EnginesField {
                node: ">=24.0.0".to_string(),
                ..Default::default()
            },
            dist: crate::dsh::DistInfo {
                integrity: "sha512-test".to_string(),
                tarball: "https://example.test/dsh-0.2.0.tgz".to_string(),
            },
        };
        let mut state = AppState::new();
        state.dsh.current = Some("0.1.0".to_string());

        let error = ensure_node_compatible(None, &manifest, "22.19.0", "0.2.0").unwrap_err();

        assert!(matches!(error, LauncherError::NodeVersion(_)));
        assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
    }
}
