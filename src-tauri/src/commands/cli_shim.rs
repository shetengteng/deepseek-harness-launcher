use serde::Serialize;

use crate::error::Result;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DshCliInstallResult {
    pub command_path: String,
    pub path_instruction: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DshCliStatusState {
    Installed,
    NotInstalled,
    Conflict,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DshCliStatus {
    pub state: DshCliStatusState,
    pub command_path: String,
    pub path_instruction: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DshCliUninstallResult {
    pub command_path: String,
    pub removed: bool,
}

#[tauri::command]
pub async fn install_dsh_cli_command() -> Result<DshCliInstallResult> {
    crate::cli_shim::install_default()
}

#[tauri::command]
pub async fn get_dsh_cli_status() -> Result<DshCliStatus> {
    crate::cli_shim::status_default()
}

#[tauri::command]
pub async fn uninstall_dsh_cli_command() -> Result<DshCliUninstallResult> {
    crate::cli_shim::uninstall_default()
}
