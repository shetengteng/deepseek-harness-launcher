use std::future::Future;
use std::path::Path;

use crate::error::{LauncherError, Result};
use crate::state::{AppState, StateStatus};

pub(super) async fn start_with_one_known_good_fallback<F, Fut>(mut start: F) -> Result<String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<String>>,
{
    attempt_with_one_known_good_fallback(&mut start, rollback_startup_current).await
}

async fn attempt_with_one_known_good_fallback<F, Fut, R>(
    mut start: F,
    mut rollback: R,
) -> Result<String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<String>>,
    R: FnMut() -> Result<Option<String>>,
{
    let current_error = match start().await {
        Ok(origin) => return Ok(origin),
        Err(error) => error,
    };
    if !should_try_known_good_fallback(&current_error) {
        return Err(current_error);
    }
    let Some(version) = rollback()? else {
        return Err(current_error);
    };
    tracing::warn!(%version, %current_error, "current dsh failed; retrying known-good version once");
    start().await.map_err(|known_good_error| {
        LauncherError::Host(format!(
            "current dsh failed: {current_error}; known-good {version} also failed: {known_good_error}"
        ))
    })
}

fn should_try_known_good_fallback(error: &LauncherError) -> bool {
    matches!(
        error,
        LauncherError::DshNotInstalled { .. } | LauncherError::Host(_)
    )
}

fn rollback_startup_current() -> Result<Option<String>> {
    let mut state = match AppState::load()? {
        StateStatus::FirstRun => return Ok(None),
        StateStatus::Loaded(state) => *state,
    };
    let version = rollback_startup_state(&mut state, &crate::paths::dsh_dir()?)?;
    if version.is_some() {
        state.save()?;
    }
    Ok(version)
}

fn rollback_startup_state(state: &mut AppState, dsh_dir: &Path) -> Result<Option<String>> {
    if state.dsh.known_good.is_none() || state.dsh.known_good == state.dsh.current {
        return Ok(None);
    }
    rollback_dsh_update(state, dsh_dir).map(Some)
}

fn rollback_dsh_update(state: &mut AppState, dsh_dir: &Path) -> Result<String> {
    crate::dsh::rollback_to_known_good(state, dsh_dir)
}

pub(super) fn current_dsh_version() -> Result<String> {
    match AppState::load()? {
        StateStatus::Loaded(state) => {
            state
                .dsh
                .current
                .clone()
                .ok_or_else(|| LauncherError::DshNotInstalled {
                    reason: "state.dsh.current is None".to_string(),
                })
        }
        StateStatus::FirstRun => Err(LauncherError::DshNotInstalled {
            reason: "state.json not found".to_string(),
        }),
    }
}

pub(super) fn rollback_failed_dsh_update() -> Result<String> {
    let mut state = match AppState::load()? {
        StateStatus::Loaded(state) => *state,
        StateStatus::FirstRun => {
            return Err(LauncherError::DshNotInstalled {
                reason: "state.json not found".to_string(),
            })
        }
    };
    let version = rollback_dsh_update(&mut state, &crate::paths::dsh_dir()?)?;
    state.save()?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::super::test_support::write_dsh_entry;

    #[test]
    fn failed_update_restores_known_good_version() {
        let temp = tempfile::tempdir().unwrap();
        let dsh_dir = temp.path().join("dsh");
        std::fs::create_dir_all(dsh_dir.join("0.1.0")).unwrap();
        std::fs::create_dir_all(dsh_dir.join("0.2.0")).unwrap();
        let mut state = AppState::new();

        crate::dsh::promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
        crate::dsh::promote_to_current(&mut state, &dsh_dir, "0.2.0").unwrap();

        assert_eq!(rollback_dsh_update(&mut state, &dsh_dir).unwrap(), "0.1.0");
        assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
        assert!(state.dsh.known_good.is_none());
    }

    #[test]
    fn startup_rollback_marks_current_broken_and_consumes_known_good() {
        let temp = tempfile::tempdir().unwrap();
        let dsh = temp.path().join("dsh");
        write_dsh_entry(&dsh, "0.1.0");
        std::fs::create_dir_all(dsh.join("0.2.0")).unwrap();
        let mut state = AppState::new();
        crate::dsh::promote_to_current(&mut state, &dsh, "0.1.0").unwrap();
        crate::dsh::promote_to_current(&mut state, &dsh, "0.2.0").unwrap();

        assert_eq!(
            rollback_startup_state(&mut state, &dsh).unwrap().as_deref(),
            Some("0.1.0")
        );

        assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
        assert!(state.dsh.known_good.is_none());
        assert_eq!(
            crate::dsh::read_current_pointer(&dsh).unwrap().as_deref(),
            Some("0.1.0")
        );
    }

    #[tokio::test]
    async fn startup_retries_known_good_exactly_once() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_start = Arc::clone(&attempts);

        let origin = attempt_with_one_known_good_fallback(
            move || {
                let attempt = attempts_for_start.fetch_add(1, Ordering::AcqRel);
                async move {
                    if attempt == 0 {
                        Err(LauncherError::Host("readiness timed out".to_string()))
                    } else {
                        Ok("http://127.0.0.1:48123".to_string())
                    }
                }
            },
            || Ok(Some("0.1.0".to_string())),
        )
        .await
        .unwrap();

        assert_eq!(origin, "http://127.0.0.1:48123");
        assert_eq!(attempts.load(Ordering::Acquire), 2);
    }

    #[tokio::test]
    async fn startup_does_not_retry_when_node_is_missing() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_start = Arc::clone(&attempts);

        let error = attempt_with_one_known_good_fallback(
            move || {
                attempts_for_start.fetch_add(1, Ordering::AcqRel);
                async {
                    Err(LauncherError::NodeNotInstalled {
                        reason: "missing".to_string(),
                    })
                }
            },
            || Ok(Some("0.1.0".to_string())),
        )
        .await
        .unwrap_err();

        assert!(matches!(error, LauncherError::NodeNotInstalled { .. }));
        assert_eq!(attempts.load(Ordering::Acquire), 1);
    }
}
