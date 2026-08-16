//! 镜像源表 + 探活（设计 §M2.1）。
//!
//! 内置三源（设计 §8.4）：
//!  - `nodejs.org/dist`（官方）
//!  - `npmmirror.com/mirrors/node`（阿里）
//!  - `mirrors.tuna.tsinghua.edu.cn/nodejs-release`（清华）
//!
//! `pick_default_mirror()` 按 OS locale 启发式选默认（zh* → npmmirror，否则 → nodejs.org）。
//! 探活依次 ping `/index.json`，首个 200 胜出，失败回退下一个。

use std::time::Duration;

use reqwest::Client;

use crate::error::LauncherError;

/// 镜像源 ID。内置三源 + 用户自定义。
/// 用 `&'static str` 避免生命周期噪音，自定义源用 `Box::leak` 或 `String` clone。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MirrorId {
    /// `nodejs.org/dist`
    NodejsOrg,
    /// `npmmirror.com/mirrors/node`
    Npmmirror,
    /// `mirrors.tuna.tsinghua.edu.cn/nodejs-release`
    Tuna,
    /// 用户自定义。`trusted: false`。
    Custom(String),
}

impl MirrorId {
    pub fn as_str(&self) -> &str {
        match self {
            Self::NodejsOrg => "nodejs.org",
            Self::Npmmirror => "npmmirror.com",
            Self::Tuna => "tuna",
            Self::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for MirrorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 单个镜像源。`base_url` 不带尾斜杠（拼接时统一处理）。
#[derive(Debug, Clone)]
pub struct Mirror {
    pub id: MirrorId,
    pub name: &'static str,
    pub base_url: &'static str,
    /// 内置源为 true，用户自定义为 false。前端展示时区分 UI。
    pub trusted: bool,
}

impl Mirror {
    /// 拼接 `{base_url}/index.json`。探活用。
    pub fn index_url(&self) -> String {
        join_url(self.base_url, "index.json")
    }

    /// 拼接 `{base_url}/v{version}/node-v{version}-{platform}-{arch}.tar.gz`。
    /// 平台和架构用 Node 官方命名（`darwin-arm64` / `linux-x64` / `win-x64`）。
    pub fn archive_url(&self, version: &str, platform: &str, arch: &str) -> String {
        let filename = format!("node-v{version}-{platform}-{arch}.tar.gz");
        join_url(self.base_url, &format!("v{version}/{filename}"))
    }

    /// 拼接 `{base_url}/v{version}/SHASUMS256.txt`。
    pub fn shasums_url(&self, version: &str) -> String {
        join_url(self.base_url, &format!("v{version}/SHASUMS256.txt"))
    }
}

/// 内置镜像源表。顺序即探活顺序。
pub const BUILTIN_MIRRORS: &[Mirror] = &[
    Mirror {
        id: MirrorId::Npmmirror,
        name: "阿里云 npmmirror",
        base_url: "https://npmmirror.com/mirrors/node",
        trusted: true,
    },
    Mirror {
        id: MirrorId::NodejsOrg,
        name: "Node.js 官方",
        base_url: "https://nodejs.org/dist",
        trusted: true,
    },
    Mirror {
        id: MirrorId::Tuna,
        name: "清华大学 TUNA",
        base_url: "https://mirrors.tuna.tsinghua.edu.cn/nodejs-release",
        trusted: true,
    },
];

/// 探活路径。所有镜像源都用同一份 `/index.json`（Node 官方维护的版本清单）。
pub const DEFAULT_NODE_INDEX_PATH: &str = "index.json";

/// 镜像源相关错误。对应测试设计 §PR-006：
/// - 自定义源非 https → `NotHttps`
/// - 自定义源非 `https://` 前缀 → `InvalidUrl`
/// - 探活 404 / 5xx → `ProbeFailed`
/// - 探活超时 → `ProbeTimeout`
/// - 所有镜像源都失败 → `AllMirrorsFailed`
#[derive(Debug, thiserror::Error)]
pub enum MirrorError {
    #[error("custom mirror URL must start with https://: {0}")]
    NotHttps(String),

    #[error("custom mirror URL is invalid: {0}")]
    InvalidUrl(String),

    #[error("probe mirror {mirror} failed: {status} {body}")]
    ProbeFailed {
        mirror: String,
        status: u16,
        body: String,
    },

    #[error("probe mirror {mirror} timed out after {timeout:?}")]
    ProbeTimeout { mirror: String, timeout: Duration },

    #[error("probe mirror {mirror} network error: {cause}")]
    ProbeNetwork { mirror: String, cause: String },

    #[error("all mirrors failed (tried {})", tried.join(", "))]
    AllMirrorsFailed { tried: Vec<String> },
}

impl From<MirrorError> for LauncherError {
    fn from(e: MirrorError) -> Self {
        LauncherError::Mirror(e.to_string())
    }
}

/// 校验自定义镜像源 URL。
///
/// 规则（测试设计 §PR-006）：
/// 1. 必须以 `https://` 开头（拒绝 `http://`、裸域名）
/// 2. 必须能解析为合法 URL
/// 3. 不允许带 query 或 fragment（避免拼接时产生歧义）
/// 4. 尾斜杠可选（拼接时统一处理）
pub fn validate_custom_mirror(raw: &str) -> Result<Mirror, MirrorError> {
    if !raw.starts_with("https://") {
        return Err(MirrorError::NotHttps(raw.to_string()));
    }
    let url = url::Url::parse(raw).map_err(|e| MirrorError::InvalidUrl(e.to_string()))?;
    if url.scheme() != "https" {
        return Err(MirrorError::NotHttps(raw.to_string()));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(MirrorError::InvalidUrl(format!(
            "custom mirror URL must not contain query or fragment: {raw}"
        )));
    }
    // 归一化：去掉尾斜杠，拼接时统一加
    let base = raw.trim_end_matches('/').to_string();
    Ok(Mirror {
        id: MirrorId::Custom(base.clone()),
        name: "自定义",
        base_url: Box::leak(base.into_boxed_str()),
        trusted: false,
    })
}

/// 按 OS locale 启发式选默认镜像源。
///
/// 规则（设计 §M2.1）：
/// - locale 以 `zh` 开头 → `npmmirror.com`（国内更快）
/// - 其他 → `nodejs.org`（官方）
///
/// 通过 `LANG` / `LC_ALL` 环境变量判断 locale，Windows 上回退到 `GetUserDefaultLocaleName`
/// 简化为读 `LANG` 环境变量，Windows 用户通常也能访问官方源。
pub fn pick_default_mirror() -> &'static Mirror {
    let locale = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .unwrap_or_default();
    if locale.to_lowercase().starts_with("zh") {
        &BUILTIN_MIRRORS[0] // npmmirror
    } else {
        &BUILTIN_MIRRORS[1] // nodejs.org
    }
}

/// 探活单个镜像源：`GET {base_url}/index.json`，期望 200。
///
/// 超时由 `timeout` 控制（测试设计 §PR-006 要求可配 < 1s 以模拟超时）。
pub async fn probe_mirror(
    client: &Client,
    mirror: &Mirror,
    timeout: Duration,
) -> Result<(), MirrorError> {
    let url = mirror.index_url();
    tracing::debug!(mirror = %mirror.id, url = %url, "probing mirror");
    let req = client.get(&url).timeout(timeout);
    let resp = req.send().await.map_err(|e| {
        if e.is_timeout() {
            MirrorError::ProbeTimeout {
                mirror: mirror.id.to_string(),
                timeout,
            }
        } else {
            MirrorError::ProbeNetwork {
                mirror: mirror.id.to_string(),
                cause: e.to_string(),
            }
        }
    })?;
    let status = resp.status().as_u16();
    if status == 200 {
        tracing::debug!(mirror = %mirror.id, "probe ok");
        return Ok(());
    }
    let body = resp.text().await.unwrap_or_default();
    Err(MirrorError::ProbeFailed {
        mirror: mirror.id.to_string(),
        status,
        body: body.chars().take(200).collect(),
    })
}

/// 依次探活镜像源列表，返回首个 200 的。
///
/// 设计 §M2.1：失败回退下一个，全部失败返回 `AllMirrorsFailed`。
pub async fn probe_mirrors(
    client: &Client,
    mirrors: &[Mirror],
    timeout: Duration,
) -> Result<Mirror, MirrorError> {
    let mut tried = Vec::new();
    for m in mirrors {
        let id = m.id.to_string();
        match probe_mirror(client, m, timeout).await {
            Ok(()) => return Ok(m.clone()),
            Err(e) => {
                tracing::warn!(mirror = %id, error = %e, "probe failed, trying next");
                tried.push(id);
            }
        }
    }
    Err(MirrorError::AllMirrorsFailed { tried })
}

/// 拼接 URL：`base` 不带尾斜杠，`path` 不带前导斜杠。
/// 例：`join_url("https://nodejs.org/dist", "index.json")` → `https://nodejs.org/dist/index.json`
fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{base}/{path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_mirrors_have_three_entries() {
        assert_eq!(BUILTIN_MIRRORS.len(), 3);
        // 第一个是 npmmirror（国内优先）
        assert_eq!(BUILTIN_MIRRORS[0].id, MirrorId::Npmmirror);
        assert!(BUILTIN_MIRRORS.iter().all(|m| m.trusted));
    }

    #[test]
    fn mirror_index_url_no_double_slash() {
        let m = &BUILTIN_MIRRORS[1]; // nodejs.org
        assert_eq!(m.index_url(), "https://nodejs.org/dist/index.json");
    }

    #[test]
    fn mirror_archive_url_format() {
        let m = &BUILTIN_MIRRORS[1];
        let url = m.archive_url("22.19.0", "darwin-arm64", "arm64");
        assert_eq!(
            url,
            "https://nodejs.org/dist/v22.19.0/node-v22.19.0-darwin-arm64-arm64.tar.gz"
        );
    }

    #[test]
    fn mirror_shasums_url() {
        let m = &BUILTIN_MIRRORS[0];
        assert_eq!(
            m.shasums_url("22.19.0"),
            "https://npmmirror.com/mirrors/node/v22.19.0/SHASUMS256.txt"
        );
    }

    #[test]
    fn join_url_handles_trailing_slash() {
        assert_eq!(
            join_url("https://x.com/", "/index.json"),
            "https://x.com/index.json"
        );
        assert_eq!(
            join_url("https://x.com", "index.json"),
            "https://x.com/index.json"
        );
    }

    #[test]
    fn validate_custom_mirror_rejects_http() {
        let err = validate_custom_mirror("http://example.com").unwrap_err();
        assert!(matches!(err, MirrorError::NotHttps(_)));
    }

    #[test]
    fn validate_custom_mirror_rejects_bare_domain() {
        let err = validate_custom_mirror("example.com").unwrap_err();
        assert!(matches!(err, MirrorError::NotHttps(_)));
    }

    #[test]
    fn validate_custom_mirror_rejects_query_fragment() {
        let err = validate_custom_mirror("https://x.com/?foo=bar").unwrap_err();
        assert!(matches!(err, MirrorError::InvalidUrl(_)));
        let err = validate_custom_mirror("https://x.com/#frag").unwrap_err();
        assert!(matches!(err, MirrorError::InvalidUrl(_)));
    }

    #[test]
    fn validate_custom_mirror_accepts_https() {
        let m = validate_custom_mirror("https://my-mirror.com/node").unwrap();
        assert!(!m.trusted);
        assert_eq!(m.base_url, "https://my-mirror.com/node");
        assert!(matches!(m.id, MirrorId::Custom(_)));
    }

    #[test]
    fn validate_custom_mirror_strips_trailing_slash() {
        let m = validate_custom_mirror("https://my-mirror.com/node/").unwrap();
        assert_eq!(m.base_url, "https://my-mirror.com/node");
    }

    #[test]
    fn pick_default_mirror_returns_one_of_builtin() {
        let m = pick_default_mirror();
        assert!(BUILTIN_MIRRORS.iter().any(|b| b.id == m.id));
    }

    #[test]
    fn pick_default_mirror_prefers_npmmirror_for_zh_locale() {
        // 备份并设置 zh locale
        let old = std::env::var("LANG").ok();
        std::env::set_var("LANG", "zh_CN.UTF-8");
        let m = pick_default_mirror();
        assert_eq!(m.id, MirrorId::Npmmirror);
        // 恢复
        match old {
            Some(v) => std::env::set_var("LANG", v),
            None => std::env::remove_var("LANG"),
        }
    }

    #[test]
    fn pick_default_mirror_prefers_nodejs_org_for_en_locale() {
        let old = std::env::var("LANG").ok();
        std::env::set_var("LANG", "en_US.UTF-8");
        let m = pick_default_mirror();
        assert_eq!(m.id, MirrorId::NodejsOrg);
        match old {
            Some(v) => std::env::set_var("LANG", v),
            None => std::env::remove_var("LANG"),
        }
    }

    #[test]
    fn mirror_id_display() {
        assert_eq!(MirrorId::NodejsOrg.to_string(), "nodejs.org");
        assert_eq!(MirrorId::Custom("foo".to_string()).to_string(), "foo");
    }
}
