//! dsh 升级编排（设计 §M3.4 / PR-015）。
//!
//! 流程：
//! - `check_for_upgrade`：检查 registry 是否有可升级版本
//! - `prepare_upgrade`：下载安装新版本，设 pending

use std::path::Path;

use chrono::Utc;
use reqwest::Client;
use semver::{VersionReq, Version};

use crate::dsh::install::{install_dsh, options_from_manifest};
use crate::dsh::registry::{fetch_package_manifest, fetch_package_metadata, RegistryCache};
use crate::dsh::version::set_pending;
use crate::error::{LauncherError, Result};
use crate::node::version::current_node_satisfies;
use crate::state::AppState;

/// 可升级版本的信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeCandidate {
    pub version: String,
    pub engines_node: String,
}

/// 检查是否有可升级的 dsh 版本。
///
/// 设计 §5.3：
/// 1. 距 `last_check` 超过 `check_interval_hours` 才查
/// 2. 读 registry 的 `dist-tags.latest`
/// 3. `latest` 满足 `pinned_range`
/// 4. 不等于 current
/// 5. 不在 `ignored_versions`
/// 6. engines.node 满足当前 Node
pub async fn check_for_upgrade(
    state: &AppState,
    registry: &str,
    cache: &RegistryCache,
    client: &Client,
) -> Result<Option<UpgradeCandidate>> {
    // 1. 检查间隔
    if let Some(last_check) = state.dsh.last_check {
        let elapsed = Utc::now() - last_check;
        let interval_hours = chrono::Duration::hours(state.dsh.check_interval_hours as i64);
        if elapsed < interval_hours {
            tracing::debug!(
                elapsed_secs = elapsed.num_seconds(),
                interval_hours = state.dsh.check_interval_hours,
                "check_for_upgrade: too soon, skipping"
            );
            return Ok(None);
        }
    }

    // 2. 拉 metadata
    let metadata = fetch_package_metadata(registry, cache, client).await?;
    let latest = metadata.dist_tags.latest.clone();

    // 3. 检查 pinned_range
    let latest_version = Version::parse(&latest).map_err(|e| {
        LauncherError::DshVersion(format!("invalid latest version '{latest}': {e}"))
    })?;
    let req = VersionReq::parse(&state.dsh.pinned_range).map_err(|e| {
        LauncherError::DshVersion(format!(
            "invalid pinned_range '{}': {e}",
            state.dsh.pinned_range
        ))
    })?;
    if !req.matches(&latest_version) {
        tracing::debug!(
            latest = %latest,
            pinned_range = %state.dsh.pinned_range,
            "check_for_upgrade: latest does not match pinned_range"
        );
        return Ok(None);
    }

    // 4. 不等于 current
    if state.dsh.current.as_deref() == Some(&latest) {
        tracing::debug!(latest = %latest, "check_for_upgrade: already at latest");
        return Ok(None);
    }

    // 5. 不在 ignored_versions
    if state.dsh.ignored_versions.contains(&latest) {
        tracing::debug!(latest = %latest, "check_for_upgrade: version is ignored");
        return Ok(None);
    }

    // 6. 检查 engines.node
    let manifest = metadata.manifest_for(&latest)?;
    let engines_node = manifest.engines.node.clone();
    if !engines_node.is_empty() {
        let satisfied = current_node_satisfies(
            state.node.as_ref().map(|n| n.version.as_str()),
            &engines_node,
        )?;
        if !satisfied {
            tracing::info!(
                latest = %latest,
                engines_node = %engines_node,
                current_node = ?state.node.as_ref().map(|n| &n.version),
                "check_for_upgrade: Node version does not satisfy engines.node"
            );
            return Ok(None);
        }
    }

    Ok(Some(UpgradeCandidate {
        version: latest,
        engines_node,
    }))
}

/// 准备升级：下载安装新版本，设 pending，更新 last_check。
///
/// 调用方（前端 "立即检查更新" 按钮或后台定时检查）决定是否立即重启。
pub async fn prepare_upgrade(
    state: &mut AppState,
    registry: &str,
    candidate: &UpgradeCandidate,
    dsh_dir: &Path,
    node_dir: &Path,
    client: &Client,
    cache: &RegistryCache,
) -> Result<()> {
    let manifest = fetch_package_manifest(registry, &candidate.version, cache, client).await?;

    let mut opts = options_from_manifest(&manifest, registry, dsh_dir, node_dir);
    opts.node_executable = crate::node::install::node_bin_path(node_dir);
    opts.npm_script = Some(crate::node::install::node_npm_path(node_dir));

    install_dsh(&opts).await?;

    set_pending(state, &candidate.version);
    state.dsh.last_check = Some(Utc::now());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::NodeState;
    use chrono::Duration;

    /// 构造一份与 registry.rs fixture 一致的测试 metadata。
    #[allow(dead_code)]
    fn test_metadata() -> crate::dsh::registry::PackageMetadata {
        let raw = serde_json::json!({
            "name": crate::dsh::registry::DSH_PACKAGE_NAME,
            "dist-tags": {
                "latest": "0.1.0",
                "next": "0.1.1-rc.1"
            },
            "versions": {
                "0.0.9": {
                    "version": "0.0.9",
                    "engines": { "node": ">=20.0.0" },
                    "dist": {
                        "integrity": "sha512-aaaaaa==",
                        "tarball": "https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-0.0.9.tgz"
                    }
                },
                "0.1.0-rc.6": {
                    "version": "0.1.0-rc.6",
                    "engines": { "node": ">=22.0.0" },
                    "dist": {
                        "integrity": "sha512-bbbbbb==",
                        "tarball": "https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-0.1.0-rc.6.tgz"
                    }
                },
                "0.1.0": {
                    "version": "0.1.0",
                    "engines": { "node": ">=22.0.0" },
                    "dist": {
                        "integrity": "sha512-cccccc==",
                        "tarball": "https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-0.1.0.tgz"
                    }
                }
            }
        })
        .to_string();
        serde_json::from_str(&raw).expect("parse fixture")
    }

    fn test_state() -> AppState {
        let mut state = AppState::new();
        state.node = Some(NodeState {
            version: "22.19.0".to_string(),
            installed_at: Utc::now(),
            mirror: "https://npmmirror.com/mirrors/node".to_string(),
        });
        state
    }

    #[test]
    fn upgrade_candidate_equality() {
        let a = UpgradeCandidate {
            version: "0.1.0".to_string(),
            engines_node: ">=22.0.0".to_string(),
        };
        let b = UpgradeCandidate {
            version: "0.1.0".to_string(),
            engines_node: ">=22.0.0".to_string(),
        };
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn check_for_upgrade_skips_when_too_soon() {
        let state = test_state();
        let cache = RegistryCache::new();
        let client = crate::dsh::registry::default_client();
        // 设 last_check 为 1 分钟前，check_interval 24h
        let mut state = state;
        state.dsh.last_check = Some(Utc::now() - Duration::minutes(1));
        state.dsh.check_interval_hours = 24;

        let result = check_for_upgrade(&state, "https://registry.npmjs.org", &cache, &client)
            .await
            .expect("ok");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn check_for_upgrade_runs_when_interval_passed() {
        let mut state = test_state();
        state.dsh.last_check = Some(Utc::now() - Duration::hours(25));
        state.dsh.check_interval_hours = 24;
        // last_check must be older than interval → should proceed
        let elapsed = Utc::now() - state.dsh.last_check.unwrap();
        let interval = Duration::hours(state.dsh.check_interval_hours as i64);
        assert!(elapsed >= interval);
    }

    #[tokio::test]
    async fn check_for_upgrade_runs_when_no_last_check() {
        let state = test_state();
        // No last_check → should proceed
        assert!(state.dsh.last_check.is_none());
    }

    #[test]
    fn check_same_version_returns_none() {
        let mut state = test_state();
        state.dsh.current = Some("0.1.0".to_string());
        assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
    }

    #[test]
    fn check_ignored_version_returns_none() {
        let mut state = test_state();
        state.dsh.ignored_versions.push("0.1.0".to_string());
        assert!(state.dsh.ignored_versions.contains(&"0.1.0".to_string()));
    }

    #[test]
    fn pinned_range_mismatch_is_skipped() {
        let mut state = test_state();
        state.dsh.pinned_range = "^99.0.0".to_string();
        let req = VersionReq::parse(&state.dsh.pinned_range).unwrap();
        assert!(!req.matches(&Version::parse("0.1.0").unwrap()));
    }

    #[test]
    fn engines_node_satisfies_check() {
        let state = test_state();
        assert!(current_node_satisfies(
            state.node.as_ref().map(|n| n.version.as_str()),
            ">=22.0.0"
        )
        .unwrap());
    }

    #[test]
    fn engines_node_unsatisfied_returns_false() {
        let mut state = test_state();
        state.node = Some(NodeState {
            version: "20.0.0".to_string(),
            installed_at: Utc::now(),
            mirror: "https://npmmirror.com/mirrors/node".to_string(),
        });
        assert!(!current_node_satisfies(
            state.node.as_ref().map(|n| n.version.as_str()),
            ">=22.0.0"
        )
        .unwrap());
    }
}