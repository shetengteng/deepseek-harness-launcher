//! 版本解析 + engines 校验（设计 §M2.4）。
//!
//! 默认目标版本：与 dsh `engines.node` 对齐。
//! 测试用例覆盖：`parse_node_version` 接受/拒绝、`satisfies_engines` 匹配。

use semver::{Version, VersionReq};
use serde::Deserialize;

use crate::error::{LauncherError, Result};

/// 默认目标 Node 版本。与 dsh `package.json.engines.node` 对齐。
/// 每发布前确认（设计 §M2.4）。
pub const DEFAULT_NODE_VERSION: &str = "22.19.0";

/// 解析 Node 版本字符串。接受以下形式：
/// - `22.19.0`
/// - `v22.19.0`
/// - `22.19.0-rc.1`（预发布）
///
/// 拒绝：
/// - 空串
/// - `latest` / `lts`（不支持别名，必须显式版本）
/// - `22.x` / `>=22`（不接 range，调用方应先调用 `satisfies_engines`）
pub fn parse_node_version(s: &str) -> Result<Version> {
    let s = s.trim();
    if s.is_empty() {
        return Err(LauncherError::NodeVersion(
            "empty version string".to_string(),
        ));
    }
    let stripped = s.strip_prefix('v').unwrap_or(s);
    Version::parse(stripped)
        .map_err(|e| LauncherError::NodeVersion(format!("invalid node version '{s}': {e}")))
}

/// 校验 Node 版本是否满足 dsh `engines.node` 范围。
///
/// `engines_node` 取自 dsh `package.json`，常见值：`>=22.0.0`、`^22.19.0`、`22.x`。
/// npm 用 `semver` 解析 range，shell 用 `semver::VersionReq`。
pub fn satisfies_engines(node_version: &Version, engines_node: &str) -> Result<bool> {
    let req = VersionReq::parse(engines_node).map_err(|e| {
        LauncherError::NodeVersion(format!("invalid engines.node range '{engines_node}': {e}"))
    })?;
    Ok(req.matches(node_version))
}

/// 当前托管 Node 是否满足 dsh engines.node。
/// `state.node.version` 是 `Option<String>`，None 时返回 false（未安装）。
pub fn current_node_satisfies(
    state_node_version: Option<&str>,
    engines_node: &str,
) -> Result<bool> {
    match state_node_version {
        Some(v) => {
            let parsed = parse_node_version(v)?;
            satisfies_engines(&parsed, engines_node)
        }
        None => Ok(false),
    }
}

/// 从 dsh `engines.node` 解析首启应下载的 Node 版本。
///
/// 默认版本满足约束时直接复用；否则从 Node 官方发布索引中选择最新、可用于当前
/// 平台与架构的正式版本。缺失 `engines.node` 不是官方约束，回退到已验证默认版本。
pub async fn resolve_bootstrap_node_target(
    engines_node: Option<&str>,
    platform: &str,
    arch: &str,
    client: &reqwest::Client,
) -> Result<(String, String)> {
    let Some(engines_node) = engines_node.filter(|value| !value.trim().is_empty()) else {
        return Ok((
            DEFAULT_NODE_VERSION.to_string(),
            "launcher-verified-fallback".to_string(),
        ));
    };

    let default = parse_node_version(DEFAULT_NODE_VERSION)?;
    if satisfies_engines(&default, engines_node)? {
        return Ok((DEFAULT_NODE_VERSION.to_string(), "dsh-engines".to_string()));
    }

    let requirement = VersionReq::parse(engines_node).map_err(|error| {
        LauncherError::NodeVersion(format!("invalid engines.node '{engines_node}': {error}"))
    })?;
    let releases = client
        .get("https://nodejs.org/dist/index.json")
        .send()
        .await
        .map_err(|error| {
            LauncherError::NodeDownload(format!("fetch Node release index failed: {error}"))
        })?
        .error_for_status()
        .map_err(|error| {
            LauncherError::NodeDownload(format!("Node release index returned an error: {error}"))
        })?
        .json::<Vec<NodeRelease>>()
        .await
        .map_err(|error| {
            LauncherError::NodeDownload(format!("parse Node release index failed: {error}"))
        })?;

    let artifact = node_archive_artifact(platform, arch)?;
    let target = releases
        .into_iter()
        .filter_map(|release| {
            let version = parse_node_version(&release.version).ok()?;
            if !version.pre.is_empty()
                || !requirement.matches(&version)
                || !release.files.iter().any(|file| file == artifact)
            {
                return None;
            }
            Some(version)
        })
        .max()
        .ok_or_else(|| {
            LauncherError::NodeVersion(format!(
                "no published Node release for {platform}/{arch} satisfies '{engines_node}'"
            ))
        })?;

    Ok((target.to_string(), "dsh-engines".to_string()))
}

#[derive(Debug, Deserialize)]
struct NodeRelease {
    version: String,
    files: Vec<String>,
}

fn node_archive_artifact(platform: &str, arch: &str) -> Result<&'static str> {
    match (platform, arch) {
        ("darwin", "arm64") => Ok("osx-arm64-tar"),
        ("darwin", "x64") => Ok("osx-x64-tar"),
        ("linux", "arm64") => Ok("linux-arm64-tar"),
        ("linux", "x64") => Ok("linux-x64-tar"),
        ("win", "x64") => Ok("win-x64-zip"),
        _ => Err(LauncherError::NodeVersion(format!(
            "unsupported Node platform/architecture: {platform}/{arch}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_bare_version() {
        let v = parse_node_version("22.19.0").unwrap();
        assert_eq!(v.major, 22);
        assert_eq!(v.minor, 19);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn parse_accepts_v_prefix() {
        let v = parse_node_version("v22.19.0").unwrap();
        assert_eq!(v.major, 22);
    }

    #[test]
    fn parse_accepts_prerelease() {
        let v = parse_node_version("22.19.0-rc.1").unwrap();
        assert_eq!(v.major, 22);
        assert!(v.pre.as_str() == "rc.1");
    }

    #[test]
    fn parse_rejects_empty() {
        let err = parse_node_version("").unwrap_err();
        assert!(matches!(err, LauncherError::NodeVersion(_)));
    }

    #[test]
    fn parse_rejects_latest_alias() {
        let err = parse_node_version("latest").unwrap_err();
        assert!(matches!(err, LauncherError::NodeVersion(_)));
    }

    #[test]
    fn parse_rejects_range() {
        let err = parse_node_version("22.x").unwrap_err();
        assert!(matches!(err, LauncherError::NodeVersion(_)));
    }

    #[test]
    fn parse_trims_whitespace() {
        let v = parse_node_version("  22.19.0  ").unwrap();
        assert_eq!(v.major, 22);
    }

    #[test]
    fn satisfies_engines_exact_match() {
        let v = parse_node_version("22.19.0").unwrap();
        assert!(satisfies_engines(&v, "22.19.0").unwrap());
    }

    #[test]
    fn satisfies_engines_caret_range() {
        let v = parse_node_version("22.19.1").unwrap();
        assert!(satisfies_engines(&v, "^22.19.0").unwrap());
        let v_old = parse_node_version("22.18.0").unwrap();
        assert!(!satisfies_engines(&v_old, "^22.19.0").unwrap());
    }

    #[test]
    fn satisfies_engines_greater_equal() {
        let v = parse_node_version("23.0.0").unwrap();
        assert!(satisfies_engines(&v, ">=22.0.0").unwrap());
        let v_old = parse_node_version("21.0.0").unwrap();
        assert!(!satisfies_engines(&v_old, ">=22.0.0").unwrap());
    }

    #[test]
    fn satisfies_engines_x_range() {
        let v = parse_node_version("22.5.0").unwrap();
        assert!(satisfies_engines(&v, "22.x").unwrap());
        let v_other = parse_node_version("23.0.0").unwrap();
        assert!(!satisfies_engines(&v_other, "22.x").unwrap());
    }

    #[test]
    fn satisfies_engines_rejects_invalid_range() {
        let v = parse_node_version("22.19.0").unwrap();
        let err = satisfies_engines(&v, "not-a-range").unwrap_err();
        assert!(matches!(err, LauncherError::NodeVersion(_)));
    }

    #[test]
    fn current_node_satisfies_handles_none() {
        assert!(!current_node_satisfies(None, ">=22.0.0").unwrap());
    }

    #[test]
    fn current_node_satisfies_handles_some() {
        assert!(current_node_satisfies(Some("22.19.0"), ">=22.0.0").unwrap());
        assert!(!current_node_satisfies(Some("21.0.0"), ">=22.0.0").unwrap());
    }

    #[test]
    fn default_node_version_is_parseable_and_satisfies_engines() {
        let v = parse_node_version(DEFAULT_NODE_VERSION).unwrap();
        assert!(satisfies_engines(&v, ">=22.0.0").unwrap());
        assert!(satisfies_engines(&v, "^22.19.0").unwrap());
    }
}
