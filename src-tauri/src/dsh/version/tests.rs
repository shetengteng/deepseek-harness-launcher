use super::*;
use std::path::{Path, PathBuf};

use crate::state::AppState;
use tempfile::TempDir;

fn setup_dsh_dir() -> (TempDir, PathBuf) {
    let temp = TempDir::new().unwrap();
    let dsh_dir = temp.path().join("dsh");
    std::fs::create_dir_all(&dsh_dir).unwrap();
    (temp, dsh_dir)
}

fn create_version(dsh_dir: &Path, version: &str) {
    std::fs::create_dir_all(dsh_dir.join(version)).unwrap();
}

#[test]
fn pointers_are_absent_until_written_and_replace_their_previous_value() {
    let (_temp, dsh_dir) = setup_dsh_dir();
    assert_eq!(read_current_pointer(&dsh_dir).unwrap(), None);
    assert_eq!(read_known_good_pointer(&dsh_dir).unwrap(), None);

    write_current_pointer(&dsh_dir, "0.1.0").unwrap();
    write_current_pointer(&dsh_dir, "0.2.0").unwrap();
    write_known_good_pointer(&dsh_dir, "0.1.0").unwrap();
    assert_eq!(
        read_current_pointer(&dsh_dir).unwrap().as_deref(),
        Some("0.2.0")
    );
    assert_eq!(
        read_known_good_pointer(&dsh_dir).unwrap().as_deref(),
        Some("0.1.0")
    );
}

#[test]
fn switch_requires_an_installed_version_and_updates_the_pointer() {
    let (_temp, dsh_dir) = setup_dsh_dir();
    assert!(switch(&dsh_dir, "9.9.9")
        .unwrap_err()
        .to_string()
        .contains("version dir not found"));

    create_version(&dsh_dir, "0.1.0");
    switch(&dsh_dir, "0.1.0").unwrap();
    assert_eq!(
        read_current_pointer(&dsh_dir).unwrap().as_deref(),
        Some("0.1.0")
    );
}

#[test]
fn promote_updates_state_and_preserves_the_previous_current_as_known_good() {
    let (_temp, dsh_dir) = setup_dsh_dir();
    create_version(&dsh_dir, "0.1.0");
    create_version(&dsh_dir, "0.2.0");
    let mut state = AppState::new();

    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
    state.dsh.pending = Some("0.2.0".to_string());
    promote_to_current(&mut state, &dsh_dir, "0.2.0").unwrap();

    assert_eq!(state.dsh.current.as_deref(), Some("0.2.0"));
    assert_eq!(state.dsh.known_good.as_deref(), Some("0.1.0"));
    assert!(state.dsh.pending.is_none());
    assert_eq!(state.dsh.installed.len(), 2);
    assert_eq!(
        read_known_good_pointer(&dsh_dir).unwrap().as_deref(),
        Some("0.1.0")
    );
}

#[test]
fn promote_same_version_does_not_make_it_its_own_known_good() {
    let (_temp, dsh_dir) = setup_dsh_dir();
    create_version(&dsh_dir, "0.1.0");
    let mut state = AppState::new();

    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
    assert!(state.dsh.known_good.is_none());
    assert_eq!(state.dsh.installed.len(), 1);
}

#[test]
fn promote_rejects_a_missing_version_directory() {
    let (_temp, dsh_dir) = setup_dsh_dir();
    let error = promote_to_current(&mut AppState::new(), &dsh_dir, "9.9.9").unwrap_err();
    assert!(error.to_string().contains("version dir not found"));
}

#[test]
fn rollback_restores_known_good_marks_failure_and_clears_pending() {
    let (_temp, dsh_dir) = setup_dsh_dir();
    create_version(&dsh_dir, "0.1.0");
    create_version(&dsh_dir, "0.2.0");
    let mut state = AppState::new();
    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
    promote_to_current(&mut state, &dsh_dir, "0.2.0").unwrap();
    set_pending(&mut state, "0.2.0");

    assert_eq!(
        rollback_to_known_good(&mut state, &dsh_dir).unwrap(),
        "0.1.0"
    );
    assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
    assert!(state.dsh.known_good.is_none());
    assert!(state.dsh.pending.is_none());
    assert_eq!(
        state
            .dsh
            .installed
            .iter()
            .find(|item| item.version == "0.2.0")
            .map(|item| item.status.as_str()),
        Some("broken")
    );
}

#[test]
fn rollback_requires_an_existing_known_good_directory() {
    let (_temp, dsh_dir) = setup_dsh_dir();
    let mut state = AppState::new();
    assert!(rollback_to_known_good(&mut state, &dsh_dir)
        .unwrap_err()
        .to_string()
        .contains("no known_good"));

    state.dsh.known_good = Some("0.1.0".to_string());
    assert!(rollback_to_known_good(&mut state, &dsh_dir)
        .unwrap_err()
        .to_string()
        .contains("known_good dir not found"));
}

#[test]
fn pending_helpers_update_only_the_pending_field() {
    let mut state = AppState::new();
    set_pending(&mut state, "0.3.0");
    assert_eq!(state.dsh.pending.as_deref(), Some("0.3.0"));
    clear_pending(&mut state);
    assert!(state.dsh.pending.is_none());
}

#[test]
fn prune_keeps_current_known_good_pending_and_requested_recent_versions() {
    let (_temp, dsh_dir) = setup_dsh_dir();
    for version in ["0.1.0", "0.2.0", "0.3.0", "0.4.0", "0.5.0"] {
        create_version(&dsh_dir, version);
    }
    let mut state = AppState::new();
    state.dsh.current = Some("0.5.0".to_string());
    state.dsh.known_good = Some("0.4.0".to_string());
    state.dsh.pending = Some("0.1.0".to_string());

    let pruned = prune_old_versions(&state, &dsh_dir, 1).unwrap();
    assert_eq!(pruned, vec!["0.2.0"]);
    for version in ["0.1.0", "0.3.0", "0.4.0", "0.5.0"] {
        assert!(dsh_dir.join(version).exists());
    }
}

#[test]
fn version_inventory_is_sorted_ignores_pointers_and_handles_missing_directories() {
    let (_temp, dsh_dir) = setup_dsh_dir();
    for version in ["0.3.0", "0.1.0", "0.2.0"] {
        create_version(&dsh_dir, version);
    }
    write_current_pointer(&dsh_dir, "0.1.0").unwrap();
    assert_eq!(
        list_installed_versions(&dsh_dir).unwrap(),
        vec!["0.1.0", "0.2.0", "0.3.0"]
    );
    assert!(is_version_installed(&dsh_dir, "0.1.0"));
    assert!(!is_version_installed(&dsh_dir, "9.9.9"));
    assert!(list_installed_versions(&dsh_dir.join("missing"))
        .unwrap()
        .is_empty());
    assert!(
        prune_old_versions(&AppState::new(), &dsh_dir.join("missing"), 0)
            .unwrap()
            .is_empty()
    );
}
