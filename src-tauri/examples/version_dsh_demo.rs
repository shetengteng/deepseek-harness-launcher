//! PR-014 本地演示：版本切换 + 回滚 + 清理的完整流程可视化。
//!
//! 运行方式：
//! ```bash
//! cd src-tauri
//! cargo run --example version_dsh_demo
//! ```
//!
//! 不依赖真实 npm install，用 mock 版本目录模拟。

use std::path::PathBuf;

use deepseek_harness_launcher_lib::dsh::{
    clear_pending, ignore_version, is_version_installed, list_installed_versions,
    promote_to_current, prune_old_versions, read_current_pointer, read_known_good_pointer,
    rollback_to_known_good, set_pending, switch,
};
use deepseek_harness_launcher_lib::state::AppState;

fn main() {
    println!("=== PR-014 dsh/version 本地演示 ===\n");

    let tmp = tempfile::tempdir().expect("tempdir");
    let dsh_dir = tmp.path().join("dsh");
    std::fs::create_dir_all(&dsh_dir).unwrap();
    println!("[setup] 临时 dsh 目录: {}\n", dsh_dir.display());

    let mut state = AppState::new();

    // === 步骤 1：首次安装 0.1.0 ===
    println!(">>> 步骤 1: 首次安装 0.1.0 并 promote");
    create_mock_version(&dsh_dir, "0.1.0");
    promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
    print_state(&state, &dsh_dir);
    println!();

    // === 步骤 2：升级到 0.2.0（成功）===
    println!(">>> 步骤 2: 升级到 0.2.0（成功）");
    create_mock_version(&dsh_dir, "0.2.0");
    set_pending(&mut state, "0.2.0");
    println!("    设 pending = 0.2.0");
    promote_to_current(&mut state, &dsh_dir, "0.2.0").unwrap();
    println!("    promote 成功");
    print_state(&state, &dsh_dir);
    println!();

    // === 步骤 3：升级到 0.3.0（失败回滚）===
    println!(">>> 步骤 3: 升级到 0.3.0（模拟启动失败 → 回滚）");
    create_mock_version(&dsh_dir, "0.3.0");
    set_pending(&mut state, "0.3.0");
    promote_to_current(&mut state, &dsh_dir, "0.3.0").unwrap();
    println!("    promote 成功（但启动失败模拟）");
    println!("    调用 rollback_to_known_good...");
    let rolled = rollback_to_known_good(&mut state, &dsh_dir).unwrap();
    println!("    回滚到: {rolled}");
    print_state(&state, &dsh_dir);
    println!();

    // === 步骤 4：再次尝试 0.4.0（成功）===
    println!(">>> 步骤 4: 再次尝试 0.4.0（成功）");
    create_mock_version(&dsh_dir, "0.4.0");
    set_pending(&mut state, "0.4.0");
    promote_to_current(&mut state, &dsh_dir, "0.4.0").unwrap();
    print_state(&state, &dsh_dir);
    println!();

    // === 步骤 5：用户主动忽略 0.3.0 ===
    println!(">>> 步骤 5: 用户主动忽略 0.3.0");
    ignore_version(&mut state, "0.3.0");
    println!("    ignored_versions = {:?}", state.dsh.ignored_versions);
    println!();

    // === 步骤 6：清理旧版本 ===
    println!(">>> 步骤 6: 清理旧版本（keep_extra = 0）");
    println!(
        "    清理前版本列表: {:?}",
        list_installed_versions(&dsh_dir).unwrap()
    );
    let pruned = prune_old_versions(&state, &dsh_dir, 0).unwrap();
    println!("    被删除的版本: {pruned:?}");
    println!(
        "    清理后版本列表: {:?}",
        list_installed_versions(&dsh_dir).unwrap()
    );
    println!();

    // === 步骤 7：完整状态总结 ===
    println!(">>> 步骤 7: 最终状态总结");
    print_state(&state, &dsh_dir);
    println!();
    println!("    文件系统版本目录:");
    for v in list_installed_versions(&dsh_dir).unwrap() {
        let marker = if Some(v.as_str()) == state.dsh.current.as_deref() {
            " ← current"
        } else if Some(v.as_str()) == state.dsh.known_good.as_deref() {
            " ← known_good"
        } else {
            ""
        };
        println!("      {v}{marker}");
    }
    println!();

    // === 步骤 8：手动 switch 演示 ===
    println!(">>> 步骤 8: 手动 switch 演示（不改 state，只改指针）");
    println!(
        "    switch 前 current 指针: {:?}",
        read_current_pointer(&dsh_dir).unwrap()
    );
    if is_version_installed(&dsh_dir, "0.2.0") {
        switch(&dsh_dir, "0.2.0").unwrap();
        println!("    switch 到 0.2.0");
        println!(
            "    switch 后 current 指针: {:?}",
            read_current_pointer(&dsh_dir).unwrap()
        );
        // 切回去
        switch(&dsh_dir, state.dsh.current.as_ref().unwrap()).unwrap();
        println!("    切回 current");
    }
    println!();

    // === 步骤 9：clear_pending 演示 ===
    println!(">>> 步骤 9: clear_pending 演示");
    set_pending(&mut state, "0.5.0-future");
    println!("    设 pending = 0.5.0-future");
    clear_pending(&mut state);
    println!("    clear_pending 后: pending = {:?}", state.dsh.pending);
    println!();

    println!("=== 演示完成 ===");
    println!();
    println!("验证点：");
    println!("  1. 步骤 1 首次 promote：current=0.1.0, known_good=None");
    println!("  2. 步骤 2 升级 0.2.0 成功：current=0.2.0, known_good=0.1.0");
    println!("  3. 步骤 3 升级 0.3.0 失败回滚：current=0.1.0, ignored=[0.3.0]");
    println!("  4. 步骤 4 升级 0.4.0 成功：current=0.4.0, known_good=0.1.0");
    println!("  5. 步骤 6 清理：删除 0.2.0、0.3.0（不在保留集）");
    println!("  6. 步骤 8 switch：原子切换指针");
}

fn print_state(state: &AppState, dsh_dir: &std::path::Path) {
    println!("    state.dsh.current     = {:?}", state.dsh.current);
    println!("    state.dsh.known_good  = {:?}", state.dsh.known_good);
    println!("    state.dsh.pending     = {:?}", state.dsh.pending);
    println!(
        "    state.dsh.ignored     = {:?}",
        state.dsh.ignored_versions
    );
    println!(
        "    state.dsh.installed   = {} 个版本",
        state.dsh.installed.len()
    );
    println!(
        "    FS  current 指针      = {:?}",
        read_current_pointer(dsh_dir).unwrap()
    );
    println!(
        "    FS  known-good 指针   = {:?}",
        read_known_good_pointer(dsh_dir).unwrap()
    );
}

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
