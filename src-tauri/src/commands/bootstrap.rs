use serde::Serialize;

use crate::commands::host::host_platform_arch;
use crate::error::{LauncherError, Result};
use crate::node::{probe_mirrors, validate_custom_mirror, Mirror, BUILTIN_MIRRORS};
use crate::state::{AppState, StateStatus};

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct MirrorInfo {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub trusted: bool,
}

impl From<&Mirror> for MirrorInfo {
    fn from(mirror: &Mirror) -> Self {
        Self {
            id: mirror.id.to_string(),
            name: mirror.name.to_string(),
            base_url: mirror.base_url.to_string(),
            trusted: mirror.trusted,
        }
    }
}

#[tauri::command]
pub async fn list_mirrors() -> Result<Vec<MirrorInfo>> {
    Ok(BUILTIN_MIRRORS.iter().map(MirrorInfo::from).collect())
}

#[tauri::command]
pub async fn probe_mirrors_command(custom_urls: Option<Vec<String>>) -> Result<MirrorInfo> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| LauncherError::Mirror(error.to_string()))?;
    let mut mirrors = Vec::new();
    if let Some(urls) = custom_urls {
        for url in urls {
            match validate_custom_mirror(&url) {
                Ok(mirror) => mirrors.push(mirror),
                Err(error) => tracing::warn!(%url, %error, "skipping invalid custom mirror"),
            }
        }
    }
    mirrors.extend(BUILTIN_MIRRORS.iter().cloned());
    let picked = probe_mirrors(&client, &mirrors, std::time::Duration::from_secs(10))
        .await
        .map_err(|error| LauncherError::Mirror(error.to_string()))?;
    Ok(MirrorInfo::from(&picked))
}

#[tauri::command]
pub async fn validate_custom_mirror_command(url: String) -> Result<MirrorInfo> {
    validate_custom_mirror(&url)
        .map(|mirror| MirrorInfo::from(&mirror))
        .map_err(|error| LauncherError::Mirror(error.to_string()))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BootstrapPlanInfo {
    pub dsh_version: String,
    pub registry: String,
    pub engines_node: Option<String>,
    pub node_version: String,
    pub requirement_source: String,
    pub resolved_at: String,
    pub phase: String,
}

impl From<&crate::state::BootstrapPlan> for BootstrapPlanInfo {
    fn from(plan: &crate::state::BootstrapPlan) -> Self {
        Self {
            dsh_version: plan.dsh_version.clone(),
            registry: plan.registry.clone(),
            engines_node: plan.engines_node.clone(),
            node_version: plan.node_version.clone(),
            requirement_source: plan.requirement_source.clone(),
            resolved_at: plan.resolved_at.to_rfc3339(),
            phase: plan.phase.clone(),
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct LatestDshVersion {
    pub latest_version: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct DshUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
}

pub(super) fn latest_dsh_version(metadata: &crate::dsh::PackageMetadata) -> Result<String> {
    let version = metadata.dist_tags.latest.clone();
    metadata.manifest_for(&version)?;
    Ok(version)
}

#[tauri::command]
pub async fn get_latest_dsh_version_command() -> Result<LatestDshVersion> {
    use crate::dsh::{default_client, fetch_package_metadata, RegistryCache};
    let state = match AppState::load()? {
        StateStatus::FirstRun => AppState::new(),
        StateStatus::Loaded(state) => *state,
    };
    let client = default_client();
    let metadata =
        fetch_package_metadata(&state.dsh.registry, &RegistryCache::new(), &client).await?;
    Ok(LatestDshVersion {
        latest_version: latest_dsh_version(&metadata)?,
    })
}

#[tauri::command]
pub async fn check_dsh_update_command() -> Result<Option<DshUpdateInfo>> {
    use crate::dsh::{default_client, fetch_package_metadata, RegistryCache};

    let mut state = match AppState::load()? {
        StateStatus::FirstRun => return Ok(None),
        StateStatus::Loaded(state) => *state,
    };
    let Some(current_version) = state.dsh.current.clone() else {
        return Ok(None);
    };
    let client = default_client();
    let metadata =
        fetch_package_metadata(&state.dsh.registry, &RegistryCache::new(), &client).await?;
    let latest_version = latest_dsh_version(&metadata)?;
    let Some(update) = update_to_notify(
        &current_version,
        state.dsh.last_notified.as_deref(),
        &latest_version,
    ) else {
        return Ok(None);
    };

    state.dsh.last_notified = Some(update.latest_version.clone());
    state.save()?;
    Ok(Some(update))
}

fn update_to_notify(
    current_version: &str,
    last_notified: Option<&str>,
    latest_version: &str,
) -> Option<DshUpdateInfo> {
    (current_version != latest_version && last_notified != Some(latest_version)).then(|| {
        DshUpdateInfo {
            current_version: current_version.to_string(),
            latest_version: latest_version.to_string(),
        }
    })
}

#[tauri::command]
pub async fn resolve_bootstrap_plan_command() -> Result<BootstrapPlanInfo> {
    use crate::dsh::{default_client, fetch_package_metadata, RegistryCache};
    let mut state = match AppState::load()? {
        StateStatus::FirstRun => AppState::new(),
        StateStatus::Loaded(state) => *state,
    };
    if let Some(plan) = state.bootstrap_plan.as_ref() {
        return Ok(BootstrapPlanInfo::from(plan));
    }
    let registry = state.dsh.registry.clone();
    let client = default_client();
    let metadata = fetch_package_metadata(&registry, &RegistryCache::new(), &client).await?;
    let version = latest_dsh_version(&metadata)?;
    let manifest = metadata.manifest_for(&version)?;
    let engines_node =
        (!manifest.engines.node.trim().is_empty()).then(|| manifest.engines.node.clone());
    let (platform, arch) = host_platform_arch();
    let (node_version, requirement_source) = crate::node::resolve_bootstrap_node_target(
        engines_node.as_deref(),
        platform,
        arch,
        &client,
    )
    .await?;
    let plan = crate::state::BootstrapPlan {
        dsh_version: version,
        registry,
        engines_node,
        node_version,
        requirement_source,
        resolved_at: chrono::Utc::now(),
        phase: "resolved".to_string(),
    };
    state.bootstrap_plan = Some(plan.clone());
    state.save()?;
    Ok(BootstrapPlanInfo::from(&plan))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builtin_mirrors_are_exposed() {
        let mirrors = futures::executor::block_on(list_mirrors()).expect("mirrors");
        assert_eq!(mirrors.len(), BUILTIN_MIRRORS.len());
        assert!(mirrors.iter().all(|mirror| mirror.trusted));
    }
    #[test]
    fn latest_version_must_have_manifest() {
        let metadata: crate::dsh::PackageMetadata = serde_json::from_value(serde_json::json!({"name":"@deepseek-ai/dsh","dist-tags":{"latest":"0.2.0"},"versions":{}})).expect("metadata");
        assert!(latest_dsh_version(&metadata).is_err());
    }

    #[test]
    fn update_check_only_returns_unnotified_new_versions() {
        assert_eq!(update_to_notify("0.1.0", None, "0.1.0"), None);
        assert_eq!(update_to_notify("0.1.0", Some("0.2.0"), "0.2.0"), None);
        assert_eq!(
            update_to_notify("0.1.0", Some("0.2.0"), "0.3.0"),
            Some(DshUpdateInfo {
                current_version: "0.1.0".to_string(),
                latest_version: "0.3.0".to_string(),
            })
        );
    }
}
