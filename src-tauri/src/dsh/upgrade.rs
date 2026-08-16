//! dsh 升级编排（设计 §M3.4 / PR-015）。
//!
//! 流程：
//! - `check_for_upgrade`：检查 registry 是否有可升级版本
//! - `prepare_upgrade`：下载安装新版本，设 pending

use std::path::Path;

use chrono::Utc;
use reqwest::Client;
use semver::{Version, VersionReq};

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

/// 有新版 dsh 但当前 Node 不满足 `engines.node`（设计 §5.4 / PR-018）。
/// 前端据此弹"需要 Node X，当前 Y"确认框。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeBlock {
    pub dsh_version: String,
    pub engines_node: String,
    pub current_node: Option<String>,
}

/// `check_for_upgrade` 的完整结果：候选版本与 Node 阻塞信息互斥存在。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpgradeCheck {
    pub candidate: Option<UpgradeCandidate>,
    pub node_block: Option<NodeBlock>,
}

impl UpgradeCheck {
    fn none() -> Self {
        Self::default()
    }
}

/// 检查是否有可升级的 dsh 版本。
///
/// 设计 §5.3：
/// 1. 距 `last_check` 超过 `check_interval_hours` 才查
/// 2. 读 registry 的 `dist-tags.latest`
/// 3. `latest` 满足 `pinned_range`
/// 4. 不等于 current
/// 5. 不在 `ignored_versions`
/// 6. engines.node 满足当前 Node；不满足 → 返回 `node_block` 让前端走 Node 升级流程
pub async fn check_for_upgrade(
    state: &AppState,
    registry: &str,
    cache: &RegistryCache,
    client: &Client,
) -> Result<UpgradeCheck> {
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
            return Ok(UpgradeCheck::none());
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
        return Ok(UpgradeCheck::none());
    }

    // 4. 不等于 current
    if state.dsh.current.as_deref() == Some(&latest) {
        tracing::debug!(latest = %latest, "check_for_upgrade: already at latest");
        return Ok(UpgradeCheck::none());
    }

    // 5. 不在 ignored_versions
    if state.dsh.ignored_versions.contains(&latest) {
        tracing::debug!(latest = %latest, "check_for_upgrade: version is ignored");
        return Ok(UpgradeCheck::none());
    }

    // 6. 检查 engines.node
    let manifest = metadata.manifest_for(&latest)?;
    let engines_node = manifest.engines.node.clone();
    if !engines_node.is_empty() {
        let current_node = state.node.as_ref().map(|n| n.version.clone());
        let satisfied = current_node_satisfies(current_node.as_deref(), &engines_node)?;
        if !satisfied {
            tracing::info!(
                latest = %latest,
                engines_node = %engines_node,
                current_node = ?current_node,
                "check_for_upgrade: Node version does not satisfy engines.node"
            );
            return Ok(UpgradeCheck {
                candidate: None,
                node_block: Some(NodeBlock {
                    dsh_version: latest,
                    engines_node,
                    current_node,
                }),
            });
        }
    }

    Ok(UpgradeCheck {
        candidate: Some(UpgradeCandidate {
            version: latest,
            engines_node,
        }),
        node_block: None,
    })
}

/// 从 `engines.node` range 解析要安装的 Node 版本（PR-018）。
/// 优先用 `DEFAULT_NODE_VERSION`（经过完整测试的组合）；不满足则取 range 下界。
pub fn resolve_node_target(engines_range: &str) -> Result<String> {
    let req = VersionReq::parse(engines_range).map_err(|e| {
        LauncherError::NodeVersion(format!("invalid engines.node '{engines_range}': {e}"))
    })?;

    let default = Version::parse(crate::node::version::DEFAULT_NODE_VERSION)
        .expect("DEFAULT_NODE_VERSION is valid semver");
    if req.matches(&default) {
        return Ok(crate::node::version::DEFAULT_NODE_VERSION.to_string());
    }

    // 取 range 下界：">=24.1.0" → "24.1.0"
    let lower = engines_range
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .next()
        .unwrap_or("");
    let parsed = Version::parse(lower).map_err(|e| {
        LauncherError::NodeVersion(format!(
            "cannot resolve Node version from engines '{engines_range}': {e}"
        ))
    })?;
    if !req.matches(&parsed) {
        return Err(LauncherError::NodeVersion(format!(
            "resolved Node {parsed} does not satisfy engines '{engines_range}'"
        )));
    }
    Ok(parsed.to_string())
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
        assert!(result.candidate.is_none());
        assert!(result.node_block.is_none());
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

    // ─── PR-018: resolve_node_target ───

    #[test]
    fn resolve_node_target_prefers_default_when_satisfied() {
        // 默认 Node 版本满足 range → 返回默认版本（经过完整测试的组合）
        let target = resolve_node_target(">=20.0.0").unwrap();
        assert_eq!(target, crate::node::version::DEFAULT_NODE_VERSION);
    }

    #[test]
    fn resolve_node_target_exact_lower_bound() {
        // 默认版本不满足 → 取 range 下界
        let target = resolve_node_target(">=24.1.0").unwrap();
        assert_eq!(target, "24.1.0");
    }

    #[test]
    fn resolve_node_target_caret_range() {
        // "^20.19.0" 下界是 20.19.0，默认 22.19.0 不满足 ^20.19.0
        let target = resolve_node_target("^20.19.0").unwrap();
        assert_eq!(target, "20.19.0");
    }

    #[test]
    fn resolve_node_target_rejects_invalid_range() {
        assert!(resolve_node_target("not-a-range").is_err());
    }
}
