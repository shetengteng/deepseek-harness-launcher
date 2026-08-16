//! PR-014 集成测试：版本切换 + 回滚 + 清理的完整流程。
//!
//! 模拟真实场景：
//! - 构造多个 mock 版本目录
//! - 模拟升级 → 启动失败 → 回滚
//! - 验证 state + 文件系统指针的一致性
//! - 测试 prune 清理逻辑

use std::path::PathBuf;

use deepseek_harness_launcher_lib::dsh::{
    clear_pending, ignore_version, is_version_installed, list_installed_versions,
    promote_to_current, prune_old_versions, read_current_pointer, read_known_good_pointer,
    rollback_to_known_good, set_pending, switch, write_current_pointer, write_known_good_pointer,
};
use deepseek_harness_launcher_lib::state::AppState;
use tempfile::TempDir;

/// 构造 mock 版本目录：`dsh/<version>/` + `lib/bin.js`
fn create_mock_version(dsh_dir: &std::path::Path, version: &str) -> PathBuf {
    let version_dir = dsh_dir.join(version);
    std::fs::create_dir_all(version_dir.join("lib")).unwrap();
    std::fs::write(
        version_dir.join("lib").join("bin.js"),
        format!("// mock dsh {version}\nconsole.log('hello from {version}');\n"),
    )
    .unwrap();
    std::fs::write(
        version_dir.join("package.json"),
        format!(r#"{{"name":"@deepseek-ai/dsh","version":"{version}"}}"#),
    )
    .unwrap();
    version_dir
}

fn setup_dsh_dir() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let dsh_dir = tmp.path().join("dsh");
    std::fs::create_dir_all(&dsh_dir).unwrap();
    (tmp, dsh_dir)
}

#[test]
fn full_upgrade_lifecycle_success() {
    // 场景：从 0.1.0 升级到 0.2.0 成功
    let (_tmp, dsh_dir) = setup_dsh_dir();
    create_mock_version(&dsh_dir, "0.1.0");
    create_mock_version(&dsh_dir, "0.2.0");

    let mut state = AppState::new();

    // 1. 首次安装 0.1.0
    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
    assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
    assert!(state.dsh.known_good.is_none());
    assert_eq!(
        read_current_pointer(&dsh_dir).unwrap().as_deref(),
        Some("0.1.0")
    );

    // 2. 下载 0.2.0，设为 pending
    set_pending(&mut state, "0.2.0");

    // 3. 启动 0.2.0 成功，promote
    promote_to_current(&mut state, &dsh_dir, "0.2.0").unwrap();
    assert_eq!(state.dsh.current.as_deref(), Some("0.2.0"));
    assert_eq!(state.dsh.known_good.as_deref(), Some("0.1.0"));
    assert!(state.dsh.pending.is_none());
    assert_eq!(
        read_current_pointer(&dsh_dir).unwrap().as_deref(),
        Some("0.2.0")
    );
    assert_eq!(
        read_known_good_pointer(&dsh_dir).unwrap().as_deref(),
        Some("0.1.0")
    );

    // 4. 验证 installed 列表
    let versions: Vec<_> = state
        .dsh
        .installed
        .iter()
        .map(|i| i.version.clone())
        .collect();
    assert!(versions.contains(&"0.1.0".to_string()));
    assert!(versions.contains(&"0.2.0".to_string()));
}

#[test]
fn upgrade_failure_triggers_rollback() {
    // 场景：0.2.0 启动失败，回滚到 0.1.0
    let (_tmp, dsh_dir) = setup_dsh_dir();
    create_mock_version(&dsh_dir, "0.1.0");
    create_mock_version(&dsh_dir, "0.2.0");

    let mut state = AppState::new();
    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
    set_pending(&mut state, "0.2.0");
    promote_to_current(&mut state, &dsh_dir, "0.2.0").unwrap();

    // 模拟 0.2.0 启动失败
    set_pending(&mut state, "0.2.0");
    let rolled_back = rollback_to_known_good(&mut state, &dsh_dir).unwrap();

    assert_eq!(rolled_back, "0.1.0");
    assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
    assert!(state.dsh.known_good.is_none()); // 回滚后清空
    assert!(state.dsh.ignored_versions.contains(&"0.2.0".to_string()));
    assert!(state.dsh.pending.is_none());
    assert_eq!(
        read_current_pointer(&dsh_dir).unwrap().as_deref(),
        Some("0.1.0")
    );
}

#[test]
fn multiple_upgrade_failures_accumulate_ignored() {
    // 场景：连续 3 个版本都失败，ignored_versions 累积
    let (_tmp, dsh_dir) = setup_dsh_dir();
    for v in &["0.1.0", "0.2.0", "0.3.0", "0.4.0"] {
        create_mock_version(&dsh_dir, v);
    }

    let mut state = AppState::new();
    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();

    for &fail_version in &["0.2.0", "0.3.0", "0.4.0"] {
        set_pending(&mut state, fail_version);
        promote_to_current(&mut state, &dsh_dir, fail_version).unwrap();
        rollback_to_known_good(&mut state, &dsh_dir).unwrap();
    }

    assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
    assert!(state.dsh.ignored_versions.contains(&"0.2.0".to_string()));
    assert!(state.dsh.ignored_versions.contains(&"0.3.0".to_string()));
    assert!(state.dsh.ignored_versions.contains(&"0.4.0".to_string()));
    assert_eq!(state.dsh.ignored_versions.len(), 3);
}

#[test]
fn rollback_without_known_good_errors() {
    let (_tmp, dsh_dir) = setup_dsh_dir();
    create_mock_version(&dsh_dir, "0.1.0");

    let mut state = AppState::new();
    // 没有 known_good，无法回滚
    let err = rollback_to_known_good(&mut state, &dsh_dir).unwrap_err();
    assert!(err.to_string().contains("no known_good"));
}

#[test]
fn rollback_with_missing_known_good_dir_errors() {
    let (_tmp, dsh_dir) = setup_dsh_dir();
    create_mock_version(&dsh_dir, "0.2.0");
    // 注意：0.1.0 目录不存在

    let mut state = AppState::new();
    state.dsh.current = Some("0.2.0".to_string());
    state.dsh.known_good = Some("0.1.0".to_string()); // 但 0.1.0 目录不存在

    let err = rollback_to_known_good(&mut state, &dsh_dir).unwrap_err();
    assert!(err.to_string().contains("known_good dir not found"));
}

#[test]
fn switch_changes_current_pointer_only() {
    // switch 只改文件系统指针，不改 state
    let (_tmp, dsh_dir) = setup_dsh_dir();
    create_mock_version(&dsh_dir, "0.1.0");
    create_mock_version(&dsh_dir, "0.2.0");

    write_current_pointer(&dsh_dir, "0.1.0").unwrap();
    switch(&dsh_dir, "0.2.0").unwrap();
    assert_eq!(
        read_current_pointer(&dsh_dir).unwrap().as_deref(),
        Some("0.2.0")
    );
}

#[test]
fn switch_fails_for_uninstalled_version() {
    let (_tmp, dsh_dir) = setup_dsh_dir();
    let err = switch(&dsh_dir, "9.9.9-not-installed").unwrap_err();
    assert!(err.to_string().contains("version dir not found"));
}

#[test]
fn prune_removes_old_versions() {
    let (_tmp, dsh_dir) = setup_dsh_dir();
    for v in &["0.1.0", "0.2.0", "0.3.0", "0.4.0", "0.5.0"] {
        create_mock_version(&dsh_dir, v);
    }

    let mut state = AppState::new();
    promote_to_current(&mut state, &dsh_dir, "0.5.0").unwrap();
    promote_to_current(&mut state, &dsh_dir, "0.4.0").unwrap();
    // current=0.4.0, known_good=0.5.0

    let pruned = prune_old_versions(&state, &dsh_dir, 0).unwrap();
    assert!(pruned.contains(&"0.1.0".to_string()));
    assert!(pruned.contains(&"0.2.0".to_string()));
    assert!(pruned.contains(&"0.3.0".to_string()));
    assert!(!pruned.contains(&"0.4.0".to_string()));
    assert!(!pruned.contains(&"0.5.0".to_string()));

    assert!(!is_version_installed(&dsh_dir, "0.1.0"));
    assert!(!is_version_installed(&dsh_dir, "0.2.0"));
    assert!(!is_version_installed(&dsh_dir, "0.3.0"));
    assert!(is_version_installed(&dsh_dir, "0.4.0"));
    assert!(is_version_installed(&dsh_dir, "0.5.0"));
}

#[test]
fn prune_keeps_extra_n_versions() {
    let (_tmp, dsh_dir) = setup_dsh_dir();
    for v in &["0.1.0", "0.2.0", "0.3.0", "0.4.0", "0.5.0"] {
        create_mock_version(&dsh_dir, v);
    }

    let mut state = AppState::new();
    promote_to_current(&mut state, &dsh_dir, "0.5.0").unwrap();
    promote_to_current(&mut state, &dsh_dir, "0.4.0").unwrap();

    // keep_extra = 1：保留 0.4.0（current）+ 0.5.0（known_good）+ 0.3.0（最新 1 个）
    let pruned = prune_old_versions(&state, &dsh_dir, 1).unwrap();
    assert!(pruned.contains(&"0.1.0".to_string()));
    assert!(pruned.contains(&"0.2.0".to_string()));
    assert!(!pruned.contains(&"0.3.0".to_string()));
    assert!(is_version_installed(&dsh_dir, "0.3.0"));
}

#[test]
fn prune_keeps_pending() {
    let (_tmp, dsh_dir) = setup_dsh_dir();
    create_mock_version(&dsh_dir, "0.1.0");
    create_mock_version(&dsh_dir, "0.2.0");
    create_mock_version(&dsh_dir, "0.3.0-pending");

    let mut state = AppState::new();
    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
    state.dsh.pending = Some("0.3.0-pending".to_string());

    let pruned = prune_old_versions(&state, &dsh_dir, 0).unwrap();
    assert!(!pruned.contains(&"0.3.0-pending".to_string()));
    assert!(is_version_installed(&dsh_dir, "0.3.0-pending"));
}

#[test]
fn prune_does_not_remove_pointer_files() {
    let (_tmp, dsh_dir) = setup_dsh_dir();
    create_mock_version(&dsh_dir, "0.1.0");
    write_current_pointer(&dsh_dir, "0.1.0").unwrap();
    write_known_good_pointer(&dsh_dir, "0.1.0").unwrap();

    let state = AppState::new();
    let pruned = prune_old_versions(&state, &dsh_dir, 0).unwrap();
    assert!(!pruned.iter().any(|v| v == "current" || v == "known-good"));
    assert!(read_current_pointer(&dsh_dir).unwrap().is_some());
    assert!(read_known_good_pointer(&dsh_dir).unwrap().is_some());
}

#[test]
fn list_installed_versions_returns_all() {
    let (_tmp, dsh_dir) = setup_dsh_dir();
    for v in &["0.3.0", "0.1.0", "0.2.0"] {
        create_mock_version(&dsh_dir, v);
    }
    write_current_pointer(&dsh_dir, "0.2.0").unwrap();

    let versions = list_installed_versions(&dsh_dir).unwrap();
    assert_eq!(versions, vec!["0.1.0", "0.2.0", "0.3.0"]);
}

#[test]
fn ignore_version_idempotent() {
    let mut state = AppState::new();
    ignore_version(&mut state, "0.5.0");
    ignore_version(&mut state, "0.5.0");
    ignore_version(&mut state, "0.5.0");
    assert_eq!(state.dsh.ignored_versions.len(), 1);
}

#[test]
fn clear_pending_clears_field() {
    let mut state = AppState::new();
    state.dsh.pending = Some("0.3.0".to_string());
    clear_pending(&mut state);
    assert!(state.dsh.pending.is_none());
}

#[test]
fn full_scenario_upgrade_rollback_reupgrade() {
    // 完整场景：
    // 1. 装 0.1.0 → current
    // 2. 装 0.2.0 → 失败回滚到 0.1.0，0.2.0 进 ignored
    // 3. 装 0.3.0 → 成功，0.1.0 变 known_good
    // 4. 清理：0.2.0 目录还在（ignored 但未删），prune 时应删除
    let (_tmp, dsh_dir) = setup_dsh_dir();
    create_mock_version(&dsh_dir, "0.1.0");
    create_mock_version(&dsh_dir, "0.2.0");
    create_mock_version(&dsh_dir, "0.3.0");

    let mut state = AppState::new();

    // 1.
    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();

    // 2.
    set_pending(&mut state, "0.2.0");
    promote_to_current(&mut state, &dsh_dir, "0.2.0").unwrap();
    rollback_to_known_good(&mut state, &dsh_dir).unwrap();
    assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
    assert!(state.dsh.ignored_versions.contains(&"0.2.0".to_string()));

    // 3.
    set_pending(&mut state, "0.3.0");
    promote_to_current(&mut state, &dsh_dir, "0.3.0").unwrap();
    assert_eq!(state.dsh.current.as_deref(), Some("0.3.0"));
    assert_eq!(state.dsh.known_good.as_deref(), Some("0.1.0"));

    // 4. prune（keep_extra=0）：应删除 0.2.0（不在 current/known_good/pending 中）
    let pruned = prune_old_versions(&state, &dsh_dir, 0).unwrap();
    assert!(pruned.contains(&"0.2.0".to_string()));
    assert!(!is_version_installed(&dsh_dir, "0.2.0"));
    assert!(is_version_installed(&dsh_dir, "0.1.0"));
    assert!(is_version_installed(&dsh_dir, "0.3.0"));
}

#[test]
fn state_persistence_roundtrip() {
    // state 改动后能正确序列化/反序列化
    use deepseek_harness_launcher_lib::state::AppState;

    let (_tmp, dsh_dir) = setup_dsh_dir();
    create_mock_version(&dsh_dir, "0.1.0");
    create_mock_version(&dsh_dir, "0.2.0");

    let mut state = AppState::new();
    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
    promote_to_current(&mut state, &dsh_dir, "0.2.0").unwrap();
    state.dsh.ignored_versions.push("0.0.5-broken".to_string());

    let json = serde_json::to_string(&state).unwrap();
    let restored: AppState = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.dsh.current, state.dsh.current);
    assert_eq!(restored.dsh.known_good, state.dsh.known_good);
    assert_eq!(restored.dsh.ignored_versions, state.dsh.ignored_versions);
    assert_eq!(restored.dsh.installed.len(), state.dsh.installed.len());
}

#[test]
fn promote_same_version_twice_no_known_good_self_reference() {
    let (_tmp, dsh_dir) = setup_dsh_dir();
    create_mock_version(&dsh_dir, "0.1.0");

    let mut state = AppState::new();
    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();

    assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
    assert!(state.dsh.known_good.is_none());
    assert_ne!(
        read_known_good_pointer(&dsh_dir).unwrap().as_deref(),
        Some("0.1.0")
    );
}

#[test]
fn state_save_and_reload() {
    use deepseek_harness_launcher_lib::state::AppState;

    let tmp = TempDir::new().unwrap();
    let state_path = tmp.path().join("state.json");
    let dsh_dir = tmp.path().join("dsh");
    std::fs::create_dir_all(&dsh_dir).unwrap();
    create_mock_version(&dsh_dir, "0.1.0");

    let mut state = AppState::new();
    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
    state.save_to(&state_path).unwrap();

    // 重新加载
    use deepseek_harness_launcher_lib::state::StateStatus;
    match AppState::load_from(&state_path).unwrap() {
        StateStatus::Loaded(loaded) => {
            assert_eq!(loaded.dsh.current.as_deref(), Some("0.1.0"));
        }
        StateStatus::FirstRun => panic!("should have loaded"),
    }
}

#[test]
fn atomic_pointer_switch_preserves_consistency() {
    // 多次快速切换指针，验证一致性
    let (_tmp, dsh_dir) = setup_dsh_dir();
    for i in 0..5 {
        create_mock_version(&dsh_dir, &format!("0.{i}.0"));
    }

    // 快速切换 5 次
    for i in 0..5 {
        let v = format!("0.{i}.0");
        switch(&dsh_dir, &v).unwrap();
        assert_eq!(
            read_current_pointer(&dsh_dir).unwrap().as_deref(),
            Some(v.as_str())
        );
    }
}
