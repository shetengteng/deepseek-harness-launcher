use serde::Serialize;

use crate::error::Result;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DshCliInstallResult {
    pub command_path: String,
    pub path_instruction: String,
}

#[tauri::command]
pub async fn install_dsh_cli_command() -> Result<DshCliInstallResult> {
    crate::cli_shim::install_default()
}
