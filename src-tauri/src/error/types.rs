use std::path::PathBuf;

use thiserror::Error;

pub const EXPECTED_SCHEMA_VERSION: u8 = 3;

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error("state file is corrupt at {path}: {cause}")]
    StateCorrupt { path: PathBuf, cause: String },
    #[error("unsupported state schema version: {0} (expected {EXPECTED_SCHEMA_VERSION})")]
    UnsupportedSchemaVersion(u8),
    #[error("state migration failed at step {step}: {cause}")]
    StateMigration { step: String, cause: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("could not resolve {what} directory: {cause}")]
    PathResolve { what: &'static str, cause: String },
    #[error("logging initialization failed: {0}")]
    LoggingInit(String),
    #[error("host supervisor error: {0}")]
    Host(String),
    #[error("mirror error: {0}")]
    Mirror(String),
    #[error("node version error: {0}")]
    NodeVersion(String),
    #[error("node download error: {0}")]
    NodeDownload(String),
    #[error("node installation cancelled: {operation_id}")]
    NodeInstallCancelled { operation_id: String },
    #[error("dsh registry error: {0}")]
    DshRegistry(String),
    #[error("dsh install error: {0}")]
    DshInstall(String),
    #[error("dsh version error: {0}")]
    DshVersion(String),
    #[error("dsh CLI command error: {0}")]
    DshCli(String),
    #[error("dsh not installed: {reason}")]
    DshNotInstalled { reason: String },
    #[error("node not installed: {reason}")]
    NodeNotInstalled { reason: String },
    #[error("dsh {dsh_version} requires Node {engines_node}, current version is {current_node}")]
    NodeUpgradeRequired {
        dsh_version: String,
        current_node: String,
        engines_node: String,
        suggested_node: String,
    },
}

#[derive(Debug, serde::Serialize)]
pub struct SerializableError {
    pub kind: &'static str,
    pub message: String,
    pub user_message: String,
    pub data: Option<serde_json::Value>,
}

impl LauncherError {
    pub fn user_message(&self) -> String {
        super::messages::user_message(self)
    }
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
            Self::NodeInstallCancelled { .. } => "node_install_cancelled",
            Self::DshRegistry(_) => "dsh_registry",
            Self::DshInstall(_) => "dsh_install",
            Self::DshVersion(_) => "dsh_version",
            Self::DshCli(_) => "dsh_cli",
            Self::DshNotInstalled { .. } => "dsh_not_installed",
            Self::NodeNotInstalled { .. } => "node_not_installed",
            Self::NodeUpgradeRequired { .. } => "node_upgrade_required",
        }
    }
    fn data(&self) -> Option<serde_json::Value> {
        match self {
            Self::StateCorrupt { path, .. } => {
                Some(serde_json::json!({"path": path.to_string_lossy()}))
            }
            Self::UnsupportedSchemaVersion(version) => {
                Some(serde_json::json!({"version": version, "expected": EXPECTED_SCHEMA_VERSION}))
            }
            Self::StateMigration { step, .. } => Some(serde_json::json!({"step": step})),
            Self::PathResolve { what, .. } => Some(serde_json::json!({"what": what})),
            Self::DshNotInstalled { reason } => {
                Some(serde_json::json!({"what": "dsh", "reason": reason}))
            }
            Self::NodeNotInstalled { reason } => {
                Some(serde_json::json!({"what": "node", "reason": reason}))
            }
            Self::NodeUpgradeRequired {
                dsh_version,
                current_node,
                engines_node,
                suggested_node,
            } => Some(serde_json::json!({
                "dsh_version": dsh_version,
                "current_node": current_node,
                "engines_node": engines_node,
                "suggested_node": suggested_node,
            })),
            _ => None,
        }
    }
}

impl serde::Serialize for LauncherError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        SerializableError {
            kind: self.kind_str(),
            message: self.to_string(),
            user_message: self.user_message(),
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
    fn serialization_preserves_kind_and_context() {
        let error = LauncherError::StateCorrupt {
            path: "/tmp/state.json".into(),
            cause: "invalid json".to_string(),
        };
        let value = serde_json::to_value(error).expect("serialize");
        assert_eq!(value["kind"], "state_corrupt");
        assert_eq!(value["data"]["path"], "/tmp/state.json");
    }

    #[test]
    fn cancellation_serializes_with_a_distinct_kind() {
        let value = serde_json::to_value(LauncherError::NodeInstallCancelled {
            operation_id: "operation-1".to_string(),
        })
        .expect("serialize");
        assert_eq!(value["kind"], "node_install_cancelled");
        assert_eq!(value["message"], "node installation cancelled: operation-1");
    }

    #[test]
    fn node_upgrade_required_serializes_the_version_gap() {
        let value = serde_json::to_value(LauncherError::NodeUpgradeRequired {
            dsh_version: "0.2.0".to_string(),
            current_node: "22.19.0".to_string(),
            engines_node: ">=24.0.0".to_string(),
            suggested_node: "24.4.0".to_string(),
        })
        .expect("serialize");
        assert_eq!(value["kind"], "node_upgrade_required");
        assert_eq!(value["data"]["dsh_version"], "0.2.0");
        assert_eq!(value["data"]["current_node"], "22.19.0");
        assert_eq!(value["data"]["engines_node"], ">=24.0.0");
        assert_eq!(value["data"]["suggested_node"], "24.4.0");
    }
}
