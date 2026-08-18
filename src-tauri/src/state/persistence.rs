use std::path::{Path, PathBuf};

use crate::error::{LauncherError, Result, EXPECTED_SCHEMA_VERSION};

use super::{AppState, StateStatus};

pub fn load() -> Result<StateStatus> {
    load_from(crate::paths::state_file()?)
}

pub fn load_from(path: impl AsRef<Path>) -> Result<StateStatus> {
    let path = path.as_ref();
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let mut state: AppState =
                serde_json::from_str(&content).map_err(|error| LauncherError::StateCorrupt {
                    path: path.to_path_buf(),
                    cause: error.to_string(),
                })?;
            match state.schema_version {
                EXPECTED_SCHEMA_VERSION => {}
                1 | 2 => {
                    if state.schema_version == 1 {
                        state.bootstrap_plan = None;
                    }
                    state.schema_version = EXPECTED_SCHEMA_VERSION;
                    save_to(&state, path)?;
                }
                version => return Err(LauncherError::UnsupportedSchemaVersion(version)),
            }
            Ok(StateStatus::Loaded(Box::new(state)))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(StateStatus::FirstRun),
        Err(error) => Err(LauncherError::Io(error)),
    }
}

pub fn save(state: &AppState) -> Result<()> {
    save_to(state, crate::paths::state_file()?)
}

pub fn save_to(state: &AppState, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary_path = tmp_sibling(path);
    std::fs::write(&temporary_path, serde_json::to_vec_pretty(state)?)?;
    std::fs::rename(temporary_path, path)?;
    Ok(())
}

fn tmp_sibling(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| "state.json".into());
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ThemeMode;
    #[test]
    fn missing_state_is_first_run_without_writing() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("state.json");
        assert!(matches!(
            load_from(&path).expect("load"),
            StateStatus::FirstRun
        ));
        assert!(!path.exists());
    }
    #[test]
    fn save_round_trips_and_removes_temporary_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("state.json");
        let state = AppState::new();
        save_to(&state, &path).expect("save");
        assert!(matches!(
            load_from(&path).expect("load"),
            StateStatus::Loaded(_)
        ));
        assert!(!tmp_sibling(&path).exists());
    }
    #[test]
    fn corrupt_state_is_reported() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("state.json");
        std::fs::write(&path, "{").expect("write");
        assert!(matches!(
            load_from(path),
            Err(LauncherError::StateCorrupt { .. })
        ));
    }

    #[test]
    fn missing_theme_defaults_to_light() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("state.json");
        let mut state = serde_json::to_value(AppState::new()).expect("state json");
        state.as_object_mut().expect("state object").remove("theme");
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&state).expect("state bytes"),
        )
        .expect("write state");

        let StateStatus::Loaded(state) = load_from(path).expect("load state") else {
            panic!("state should load");
        };
        assert_eq!(state.theme, ThemeMode::Light);
    }
}
