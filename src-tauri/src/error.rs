//! Launcher 错误类型。
//!
//! 对应设计 §11.1：所有用户可见错误都要可序列化到前端，结构为 `{ kind, message, data }`。
//! Tauri 命令的返回类型 `Result<T, LauncherError>` 会通过 `impl Serialize` 把错误序列化成 JSON。

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LauncherError {
    /// `state.json` 损坏：JSON 解析失败或字段缺失。
    #[error("state file is corrupt at {path}: {cause}")]
    StateCorrupt { path: PathBuf, cause: String },

    /// `state.json` 的 `schema_version` 不支持（通常意味着更高版本写入过）。
    #[error("unsupported state schema version: {0} (expected {EXPECTED_SCHEMA_VERSION})")]
    UnsupportedSchemaVersion(u8),

    /// `state.json` 在迁移过程中失败。
    #[error("state migration failed at step {step}: {cause}")]
    StateMigration { step: String, cause: String },

    /// 系统 IO 错误（读写文件、目录创建等）。
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON 序列化/反序列化失败。
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// 路径解析失败：用户目录无法确定（极端环境）。
    #[error("could not resolve {what} directory: {cause}")]
    PathResolve { what: &'static str, cause: String },

    /// 日志系统初始化失败。
    #[error("logging initialization failed: {0}")]
    LoggingInit(String),

    /// Host 监管器错误：子进程 spawn 失败、就绪超时、意外退出等。
    #[error("host supervisor error: {0}")]
    Host(String),

    /// 镜像源错误：自定义源校验失败、探活失败等（设计 §M2.1）。
    #[error("mirror error: {0}")]
    Mirror(String),

    /// Node 版本解析失败（设计 §M2.4）。
    #[error("node version error: {0}")]
    NodeVersion(String),

    /// Node 下载/校验错误（设计 §M2.2）。
    #[error("node download error: {0}")]
    NodeDownload(String),

    /// dsh registry 查询错误：HTTP 失败、JSON 解析失败、版本不存在等（设计 §M3.1）。
    #[error("dsh registry error: {0}")]
    DshRegistry(String),

    /// dsh 安装错误：npm install 失败、完整性校验失败、原子切换失败等（设计 §M3.2）。
    #[error("dsh install error: {0}")]
    DshInstall(String),

    /// dsh 版本切换错误：指针写入失败、版本未安装、回滚无 known_good 等（设计 §M3.3）。
    #[error("dsh version error: {0}")]
    DshVersion(String),

    /// dsh 未安装：state.dsh.current 为 None、current 指针不存在、cli entry 文件缺失。
    /// 前端拿到此错误后应提示用户走首启向导 / 安装 dsh 流程。
    #[error("dsh not installed: {reason}")]
    DshNotInstalled { reason: String },

    /// Node 运行时未安装：node-runtime/VERSION 文件不存在。
    /// 前端拿到此错误后应提示用户走首启向导安装 Node。
    #[error("node not installed: {reason}")]
    NodeNotInstalled { reason: String },
}

/// 当前 state.json schema 版本。`AppState::load` 会校验该值。
pub const EXPECTED_SCHEMA_VERSION: u8 = 1;

/// 序列化到前端的错误结构。对应设计 §11.1。
#[derive(Debug, serde::Serialize)]
pub struct SerializableError {
    pub kind: &'static str,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl LauncherError {
    fn kind_str(&self) -> &'static str {
        match self {
            Self::StateCorrupt { .. } => "state_corrupt",
            Self::UnsupportedSchemaVersion(_) => "unsupported_schema_version",
            Self::StateMigration { .. } => "state_migration",
            Self::Io(_) => "io",
            Self::Serialization(_) => "serialization",
            Self::PathResolve { .. } => "path_resolve",
            Self::LoggingInit(_) => "logging_init",
            Self::Host(_) => "host",
            Self::Mirror(_) => "mirror",
            Self::NodeVersion(_) => "node_version",
            Self::NodeDownload(_) => "node_download",
            Self::DshRegistry(_) => "dsh_registry",
            Self::DshInstall(_) => "dsh_install",
            Self::DshVersion(_) => "dsh_version",
            Self::DshNotInstalled { .. } => "dsh_not_installed",
            Self::NodeNotInstalled { .. } => "node_not_installed",
        }
    }

    fn data(&self) -> Option<serde_json::Value> {
        match self {
            Self::StateCorrupt { path, .. } => Some(serde_json::json!({
                "path": path.to_string_lossy(),
            })),
            Self::UnsupportedSchemaVersion(v) => Some(serde_json::json!({
                "version": v,
                "expected": EXPECTED_SCHEMA_VERSION,
            })),
            Self::StateMigration { step, .. } => Some(serde_json::json!({
                "step": step,
            })),
            Self::PathResolve { what, .. } => Some(serde_json::json!({
                "what": what,
            })),
            Self::DshNotInstalled { reason } => Some(serde_json::json!({
                "what": "dsh",
                "reason": reason,
            })),
            Self::NodeNotInstalled { reason } => Some(serde_json::json!({
                "what": "node",
                "reason": reason,
            })),
            Self::Io(_)
            | Self::Serialization(_)
            | Self::LoggingInit(_)
            | Self::Host(_)
            | Self::Mirror(_)
            | Self::NodeVersion(_)
            | Self::NodeDownload(_)
            | Self::DshRegistry(_)
            | Self::DshInstall(_)
            | Self::DshVersion(_) => None,
        }
    }
}

/// 给 Tauri 命令做错误序列化：`Result<T, LauncherError>` 自动转成前端可读的 `{ kind, message, data }`。
impl serde::Serialize for LauncherError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        SerializableError {
            kind: self.kind_str(),
            message: self.to_string(),
            data: self.data(),
        }
        .serialize(serializer)
    }
}

pub type Result<T> = std::result::Result<T, LauncherError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_and_data_round_trip() {
        let err = LauncherError::StateCorrupt {
            path: PathBuf::from("/tmp/state.json"),
            cause: "unexpected token at line 1 column 1".to_string(),
        };
        assert_eq!(err.kind_str(), "state_corrupt");
        let data = err.data().expect("state_corrupt has data");
        assert_eq!(data["path"], "/tmp/state.json");
    }

    #[test]
    fn unsupported_version_carries_expected() {
        let err = LauncherError::UnsupportedSchemaVersion(99);
        let data = err.data().expect("unsupported_schema_version has data");
        assert_eq!(data["version"], 99);
        assert_eq!(data["expected"], EXPECTED_SCHEMA_VERSION);
    }

    #[test]
    fn io_error_serializes_to_json() {
        let err = LauncherError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"));
        let json = serde_json::to_value(&err).expect("serialize");
        assert_eq!(json["kind"], "io");
        assert!(json["message"].as_str().unwrap().contains("io error"));
        assert!(json["data"].is_null());
    }
}
