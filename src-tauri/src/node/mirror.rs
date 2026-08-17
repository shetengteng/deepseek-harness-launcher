//! 镜像源表 + 探活（设计 §M2.1）。

mod probe;
mod validation;

use std::time::Duration;

pub use probe::{probe_mirror, probe_mirrors};
pub use validation::validate_custom_mirror;

/// 镜像源 ID。内置三源 + 用户自定义。
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
            Self::Custom(value) => value,
        }
    }
}

impl std::fmt::Display for MirrorId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
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
        join_url(self.base_url, DEFAULT_NODE_INDEX_PATH)
    }

    /// 拼接 `{base_url}/v{version}/node-v{version}-{platform}-{arch}.{tar.gz|zip}`。
    pub fn archive_url(&self, version: &str, platform: &str, arch: &str) -> String {
        let filename = super::install::node_archive_filename(version, platform, arch);
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

/// 探活路径。所有镜像源都用同一份 `/index.json`。
pub const DEFAULT_NODE_INDEX_PATH: &str = "index.json";

/// 镜像源相关错误。
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

impl From<MirrorError> for crate::error::LauncherError {
    fn from(error: MirrorError) -> Self {
        Self::Mirror(error.to_string())
    }
}

/// 按 OS locale 启发式选默认镜像源。
pub fn pick_default_mirror() -> &'static Mirror {
    let locale = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .unwrap_or_default();
    if locale.to_lowercase().starts_with("zh") {
        &BUILTIN_MIRRORS[0]
    } else {
        &BUILTIN_MIRRORS[1]
    }
}

pub(super) fn join_url(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_trusted_and_ordered() {
        assert_eq!(BUILTIN_MIRRORS.len(), 3);
        assert_eq!(BUILTIN_MIRRORS[0].id, MirrorId::Npmmirror);
        assert!(BUILTIN_MIRRORS.iter().all(|mirror| mirror.trusted));
    }

    #[test]
    fn mirror_urls_are_normalized() {
        let mirror = &BUILTIN_MIRRORS[1];
        assert_eq!(mirror.index_url(), "https://nodejs.org/dist/index.json");
        assert_eq!(
            mirror.archive_url("22.19.0", "darwin", "arm64"),
            "https://nodejs.org/dist/v22.19.0/node-v22.19.0-darwin-arm64.tar.gz"
        );
        assert_eq!(
            mirror.archive_url("22.19.0", "win", "x64"),
            "https://nodejs.org/dist/v22.19.0/node-v22.19.0-win-x64.zip"
        );
        assert_eq!(
            join_url("https://x.com/", "/index.json"),
            "https://x.com/index.json"
        );
    }

    #[test]
    fn default_mirror_is_builtin() {
        let mirror = pick_default_mirror();
        assert!(BUILTIN_MIRRORS
            .iter()
            .any(|builtin| builtin.id == mirror.id));
    }

    #[test]
    fn mirror_id_displays_stable_values() {
        assert_eq!(MirrorId::NodejsOrg.to_string(), "nodejs.org");
        assert_eq!(MirrorId::Custom("foo".to_string()).to_string(), "foo");
    }
}
