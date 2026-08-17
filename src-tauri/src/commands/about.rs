use std::path::PathBuf;

use serde::Serialize;

use crate::{
    error::Result,
    state::{AppState, StateStatus},
};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct AboutInfo {
    pub launcher_version: String,
    pub dsh_version: Option<String>,
    pub node_version: Option<String>,
    pub data_directory: String,
}

#[tauri::command]
pub async fn get_about_info() -> Result<AboutInfo> {
    Ok(build_about_info(
        AppState::load()?,
        crate::paths::data_dir()?,
    ))
}

fn build_about_info(state: StateStatus, data_directory: PathBuf) -> AboutInfo {
    let (dsh_version, node_version) = match state {
        StateStatus::FirstRun => (None, None),
        StateStatus::Loaded(state) => (
            state.dsh.current.clone(),
            state.node.as_ref().map(|node| node.version.clone()),
        ),
    };

    AboutInfo {
        launcher_version: env!("CARGO_PKG_VERSION").to_string(),
        dsh_version,
        node_version,
        data_directory: data_directory.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, NodeState};

    #[test]
    fn includes_managed_runtime_details() {
        let mut state = AppState::new();
        state.dsh.current = Some("0.3.0".to_string());
        state.node = Some(NodeState {
            version: "22.19.0".to_string(),
            installed_at: chrono::Utc::now(),
            mirror: "https://nodejs.org/dist".to_string(),
        });

        let info = build_about_info(
            StateStatus::Loaded(Box::new(state)),
            PathBuf::from("/tmp/deepseek-harness-launcher"),
        );

        assert_eq!(info.dsh_version.as_deref(), Some("0.3.0"));
        assert_eq!(info.node_version.as_deref(), Some("22.19.0"));
        assert_eq!(info.data_directory, "/tmp/deepseek-harness-launcher");
    }

    #[test]
    fn first_run_has_no_managed_runtime_details() {
        let info = build_about_info(StateStatus::FirstRun, PathBuf::from("/tmp/data"));

        assert_eq!(info.dsh_version, None);
        assert_eq!(info.node_version, None);
    }
}
