use serde::Serialize;

use crate::error::{LauncherError, Result};
use crate::node::BUILTIN_MIRRORS;
use crate::state::{AppState, StateStatus, ThemeMode};

fn native_theme(mode: ThemeMode) -> tauri::Theme {
    match mode {
        ThemeMode::Light => tauri::Theme::Light,
        ThemeMode::Dark => tauri::Theme::Dark,
    }
}

pub(crate) fn apply_window_theme(window: &tauri::WebviewWindow, mode: ThemeMode) {
    if let Err(error) = window.set_theme(Some(native_theme(mode))) {
        tracing::warn!(%error, "failed to apply window theme");
    }
}

pub(crate) fn apply_persisted_window_theme(window: &tauri::WebviewWindow) {
    let mode = match AppState::load() {
        Ok(StateStatus::Loaded(state)) => state.theme,
        Ok(StateStatus::FirstRun) | Err(_) => ThemeMode::default(),
    };
    apply_window_theme(window, mode);
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DshStateSnapshot {
    pub current: Option<String>,
    pub known_good: Option<String>,
    pub node_mirror: String,
    pub registry: String,
    pub installed: Vec<InstalledDshInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct InstalledDshInfo {
    pub version: String,
    pub installed_at: String,
    pub status: String,
}

#[tauri::command]
pub async fn get_dsh_state() -> Result<DshStateSnapshot> {
    match AppState::load()? {
        StateStatus::FirstRun => Ok(DshStateSnapshot {
            current: None,
            known_good: None,
            node_mirror: crate::node::pick_default_mirror().base_url.to_string(),
            registry: "https://registry.npmjs.org".to_string(),
            installed: vec![],
        }),
        StateStatus::Loaded(state) => Ok(DshStateSnapshot {
            current: state.dsh.current,
            known_good: state.dsh.known_good,
            node_mirror: state
                .node_mirror
                .clone()
                .or_else(|| state.node.as_ref().map(|node| node.mirror.clone()))
                .unwrap_or_else(|| crate::node::pick_default_mirror().base_url.to_string()),
            registry: state.dsh.registry,
            installed: state
                .dsh
                .installed
                .iter()
                .map(|item| InstalledDshInfo {
                    version: item.version.clone(),
                    installed_at: item.installed_at.to_rfc3339(),
                    status: item.status.clone(),
                })
                .collect(),
        }),
    }
}

#[tauri::command]
pub async fn get_theme_command() -> Result<ThemeMode> {
    Ok(match AppState::load()? {
        StateStatus::FirstRun => ThemeMode::default(),
        StateStatus::Loaded(state) => state.theme,
    })
}

fn validated_theme(theme: &str) -> Result<ThemeMode> {
    match theme {
        "light" => Ok(ThemeMode::Light),
        "dark" => Ok(ThemeMode::Dark),
        _ => Err(LauncherError::Theme("请选择受支持的主题".to_string())),
    }
}

fn validated_node_mirror(mirror: &str) -> Result<String> {
    BUILTIN_MIRRORS
        .iter()
        .find(|candidate| candidate.base_url == mirror)
        .map(|candidate| candidate.base_url.to_string())
        .ok_or_else(|| LauncherError::Mirror("请选择受支持的 Node.js 下载源".to_string()))
}

fn validated_registry(registry: &str) -> Result<String> {
    match registry.trim_end_matches('/') {
        "https://registry.npmjs.org" | "https://registry.npmmirror.com" => {
            Ok(registry.trim_end_matches('/').to_string())
        }
        _ => Err(LauncherError::DshRegistry(
            "请选择受支持的 npm 下载源".to_string(),
        )),
    }
}

#[tauri::command]
pub async fn set_node_mirror_command(mirror: String) -> Result<()> {
    let mut state = match AppState::load()? {
        StateStatus::FirstRun => AppState::new(),
        StateStatus::Loaded(state) => *state,
    };
    state.node_mirror = Some(validated_node_mirror(&mirror)?);
    state.save()
}

#[tauri::command]
pub async fn set_registry_command(registry: String) -> Result<()> {
    let mut state = match AppState::load()? {
        StateStatus::FirstRun => AppState::new(),
        StateStatus::Loaded(state) => *state,
    };
    let registry = validated_registry(&registry)?;
    state.dsh.registry = registry.clone();
    if let Some(plan) = state.bootstrap_plan.as_mut() {
        plan.registry = registry;
    }
    state.save()
}

#[tauri::command]
pub async fn set_theme_command(window: tauri::WebviewWindow, theme: String) -> Result<()> {
    let mut state = match AppState::load()? {
        StateStatus::FirstRun => AppState::new(),
        StateStatus::Loaded(state) => *state,
    };
    state.theme = validated_theme(&theme)?;
    state.save()?;
    apply_window_theme(&window, state.theme);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_only_supported_sources() {
        assert!(validated_node_mirror(BUILTIN_MIRRORS[0].base_url).is_ok());
        assert!(validated_node_mirror("https://example.test").is_err());
        assert_eq!(
            validated_registry("https://registry.npmjs.org/").expect("registry"),
            "https://registry.npmjs.org"
        );
        assert!(validated_registry("https://example.test").is_err());
    }

    #[test]
    fn validates_only_supported_themes() {
        assert_eq!(validated_theme("light").expect("light"), ThemeMode::Light);
        assert_eq!(validated_theme("dark").expect("dark"), ThemeMode::Dark);
        assert!(matches!(
            validated_theme("system"),
            Err(LauncherError::Theme(_))
        ));
    }

    #[test]
    fn maps_theme_mode_to_native_window_theme() {
        assert_eq!(native_theme(ThemeMode::Light), tauri::Theme::Light);
        assert_eq!(native_theme(ThemeMode::Dark), tauri::Theme::Dark);
    }
}
