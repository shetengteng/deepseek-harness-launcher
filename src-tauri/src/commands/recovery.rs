use std::sync::Arc;

use serde::Serialize;

use crate::commands::host::{build_spawn_options, SharedState};
use crate::host::{HostExitDetail, HostSupervisor};
use crate::state::{AppState, StateStatus};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CrashLimitPayload {
    pub crash_counter: u32,
    pub retry_limit: u32,
    pub exit_code: Option<i32>,
    pub exit_signal: Option<i32>,
    pub known_good: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct HostRestartedPayload {
    pub attempt: u32,
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CrashAction {
    Restarted { attempt: u32, origin: String },
    PromptUser(CrashLimitPayload),
}

pub async fn decide_after_crash(
    supervisor: &Arc<HostSupervisor>,
    exit_detail: HostExitDetail,
) -> CrashAction {
    let mut state = match AppState::load() {
        Ok(StateStatus::Loaded(state)) => *state,
        _ => {
            return CrashAction::PromptUser(CrashLimitPayload {
                crash_counter: crate::host::CRASH_RETRY_LIMIT,
                retry_limit: crate::host::CRASH_RETRY_LIMIT,
                exit_code: exit_detail.code,
                exit_signal: exit_detail.signal,
                known_good: None,
            })
        }
    };
    let decision = crate::host::record_crash(&mut state, chrono::Utc::now());
    let counter = state.crash_counter;
    let known_good = state.dsh.known_good.clone();
    if let Err(error) = state.save() {
        tracing::warn!(%error, "failed to save crash counter");
    }
    let prompt = || CrashLimitPayload {
        crash_counter: counter,
        retry_limit: crate::host::CRASH_RETRY_LIMIT,
        exit_code: exit_detail.code,
        exit_signal: exit_detail.signal,
        known_good: known_good.clone(),
    };
    if decision != crate::host::CrashDecision::RestartCurrent {
        return CrashAction::PromptUser(prompt());
    }
    match build_spawn_options().await {
        Ok(options) => match supervisor.start(&options).await {
            Ok(origin) => CrashAction::Restarted {
                attempt: counter,
                origin: origin.as_str().to_string(),
            },
            Err(error) => {
                tracing::error!(%error, attempt = counter, "auto-restart failed");
                CrashAction::PromptUser(prompt())
            }
        },
        Err(error) => {
            tracing::error!(%error, "failed to build crash recovery options");
            CrashAction::PromptUser(prompt())
        }
    }
}

pub fn spawn_crash_recovery(app: tauri::AppHandle, detail: HostExitDetail) {
    crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Recovering);
    tokio::spawn(async move {
        use tauri::{Emitter, Manager};
        let state = app.state::<SharedState>();
        state.navigation.clear_dsh_origin();
        let supervisor = state.supervisor.clone();
        match decide_after_crash(&supervisor, detail).await {
            CrashAction::Restarted { attempt, origin } => {
                app.state::<SharedState>()
                    .navigation
                    .activate_dsh_origin(&origin);
                crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Running);
                let _ = app.emit("host-restarted", &HostRestartedPayload { attempt, origin });
            }
            CrashAction::PromptUser(payload) => {
                crate::tray::set_host_status(&app, crate::tray::HostTrayStatus::Crashed);
                let _ = app.emit("host-crash-limit", &payload);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crash_payload_uses_frontend_field_names() {
        let payload = CrashLimitPayload {
            crash_counter: 3,
            retry_limit: 3,
            exit_code: Some(1),
            exit_signal: None,
            known_good: Some("0.1.0".to_string()),
        };
        let value = serde_json::to_value(payload).expect("serialize");
        assert_eq!(value["crash_counter"], 3);
        assert_eq!(value["known_good"], "0.1.0");
    }
}
