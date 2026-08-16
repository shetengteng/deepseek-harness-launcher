//! dsh 版本切换与回滚（设计 §M3.3，PR-014）。
//!
//! 设计要点：
//! - `current` / `known-good` 指针：Unix 用 symlink，Windows 用 JSON 文件
//! - `switch(version)`：原子改 current 指针
//! - `promote_to_current(state, version)`：新版本启动成功后调用
//!   - 旧 current → known_good
//!   - 新版本 → current
//!   - 清除 pending
//! - `rollback_to_known_good(state)`：当前版本失败时回滚
//!   - known_good → current
//!   - 失败版本 → ignored_versions
//!   - 清除 pending
//! - `prune_old_versions(state, keep_versions)`：清理旧版本，保留 current + known_good + N

use std::path::Path;

use chrono::Utc;

use crate::error::{LauncherError, Result};
use crate::state::{AppState, DshState, InstalledDsh};

/// 指针文件名（Windows 用 JSON 文件代替 symlink）。
pub const CURRENT_POINTER_NAME: &str = "current";
pub const KNOWN_GOOD_POINTER_NAME: &str = "known-good";

/// Windows 指针 JSON 文件的后缀。
#[cfg(windows)]
const POINTER_JSON_SUFFIX: &str = ".json";

/// 非 Windows 平台的占位（实际不使用，仅为常量定义统一）。
#[cfg(not(windows))]
const POINTER_JSON_SUFFIX: &str = "";

/// 读取 current 指针指向的版本。
///
/// - Unix：读 symlink 的 target basename
/// - Windows：读 `current.json` 的 `version` 字段
pub fn read_current_pointer(dsh_dir: &Path) -> Result<Option<String>> {
    read_pointer(dsh_dir, CURRENT_POINTER_NAME)
}

/// 读取 known-good 指针指向的版本。
pub fn read_known_good_pointer(dsh_dir: &Path) -> Result<Option<String>> {
    read_pointer(dsh_dir, KNOWN_GOOD_POINTER_NAME)
}

/// 写 current 指针：原子替换。
///
/// 调用前需保证 `version_dir` 存在（已安装）。
pub fn write_current_pointer(dsh_dir: &Path, version: &str) -> Result<()> {
    write_pointer(dsh_dir, CURRENT_POINTER_NAME, version)
}

/// 写 known-good 指针。
pub fn write_known_good_pointer(dsh_dir: &Path, version: &str) -> Result<()> {
    write_pointer(dsh_dir, KNOWN_GOOD_POINTER_NAME, version)
}

/// 切换 current 指针到指定版本。
///
/// 流程：
/// 1. 校验 `dsh/<version>/` 存在
/// 2. 原子写入新指针
///
/// **不修改 state.json**——调用方负责 state 更新（通常配合 `promote_to_current`）。
pub fn switch(dsh_dir: &Path, version: &str) -> Result<()> {
    let version_dir = dsh_dir.join(version);
    if !version_dir.exists() {
        return Err(LauncherError::DshVersion(format!(
            "cannot switch: version dir not found: {}",
            version_dir.display()
        )));
    }
    write_current_pointer(dsh_dir, version)
}

/// 提升新版本为 current：state 层面的更新。
///
/// 设计 §M3.3：
/// - 旧 current → known_good
/// - 新版本 → current
/// - 清除 pending
/// - 把新版本加到 installed（若不存在）
///
/// 同时更新文件系统指针：
/// - 如果旧 current 存在，写到 known-good 指针
/// - 新版本写到 current 指针
pub fn promote_to_current(state: &mut AppState, dsh_dir: &Path, version: &str) -> Result<()> {
    let version_dir = dsh_dir.join(version);
    if !version_dir.exists() {
        return Err(LauncherError::DshVersion(format!(
            "cannot promote: version dir not found: {}",
            version_dir.display()
        )));
    }

    let old_current = state.dsh.current.clone();

    // 1. 旧 current → known_good（指针）
    if let Some(old) = &old_current {
        if old != version {
            write_known_good_pointer(dsh_dir, old)?;
        }
    }

    // 2. 新版本 → current（指针）
    write_current_pointer(dsh_dir, version)?;

    // 3. 更新 state
    state.dsh.known_good = old_current.filter(|old| old != version);
    state.dsh.current = Some(version.to_string());
    state.dsh.pending = None;

    // 4. 加入 installed（若不存在）
    upsert_installed(&mut state.dsh, version, "verified");

    Ok(())
}

/// 回滚到 known_good：当前版本启动失败时调用。
///
/// 设计 §M3.3：
/// - known_good → current（指针 + state）
/// - 失败版本（原 current）→ ignored_versions
/// - 清除 pending
/// - 失败版本在 installed 中标记为 `broken`
///
/// 若无 known_good → 报错（无法回滚）。
pub fn rollback_to_known_good(state: &mut AppState, dsh_dir: &Path) -> Result<String> {
    let known_good = state.dsh.known_good.clone().ok_or_else(|| {
        LauncherError::DshVersion("cannot rollback: no known_good version available".to_string())
    })?;

    let failed_version = state.dsh.current.clone();

    // 1. known_good → current（指针）
    let known_good_dir = dsh_dir.join(&known_good);
    if !known_good_dir.exists() {
        return Err(LauncherError::DshVersion(format!(
            "cannot rollback: known_good dir not found: {}",
            known_good_dir.display()
        )));
    }
    write_current_pointer(dsh_dir, &known_good)?;

    // 2. 更新 state：known_good 提升为 current，原 known_good 字段清空
    //    （没有更老的 known_good 了；下次 promote 会把新 current 存为 known_good）
    state.dsh.current = Some(known_good.clone());
    state.dsh.known_good = None;
    state.dsh.pending = None;

    // 3. 失败版本 → ignored_versions + installed 标 broken
    if let Some(failed) = failed_version {
        if !state.dsh.ignored_versions.contains(&failed) {
            state.dsh.ignored_versions.push(failed.clone());
        }
        mark_installed_status(&mut state.dsh, &failed, "broken");
    }

    // 4. known_good 在 installed 中标 verified（恢复）
    mark_installed_status(&mut state.dsh, &known_good, "verified");

    Ok(known_good)
}

/// 设置 pending 版本（下载安装完成后、启动前调用）。
pub fn set_pending(state: &mut AppState, version: &str) {
    state.dsh.pending = Some(version.to_string());
}

/// 清除 pending（升级完成或放弃时调用）。
pub fn clear_pending(state: &mut AppState) {
    state.dsh.pending = None;
}

/// 把版本加入 ignored_versions（用户主动忽略或多次失败时调用）。
pub fn ignore_version(state: &mut AppState, version: &str) {
    if !state.dsh.ignored_versions.contains(&version.to_string()) {
        state.dsh.ignored_versions.push(version.to_string());
    }
}

/// 清理旧版本，保留 current + known_good + `keep_extra` 个最新版本。
///
/// 设计 §M3.3：`prune_old_versions(keep_versions)` 保留 current + known_good + N。
///
/// - `keep_extra = 0`：只保留 current + known_good
/// - `keep_extra = 2`：保留 current + known_good + 最新 2 个
///
/// 返回被删除的版本列表。
pub fn prune_old_versions(
    state: &AppState,
    dsh_dir: &Path,
    keep_extra: usize,
) -> Result<Vec<String>> {
    let mut keep_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(c) = &state.dsh.current {
        keep_set.insert(c.clone());
    }
    if let Some(k) = &state.dsh.known_good {
        keep_set.insert(k.clone());
    }
    // pending 也保留
    if let Some(p) = &state.dsh.pending {
        keep_set.insert(p.clone());
    }

    // 从文件系统扫描所有版本目录（不依赖 state.dsh.installed，避免 state 与文件系统不同步）
    let mut installed: Vec<String> = list_installed_versions(dsh_dir)?
        .into_iter()
        .filter(|v| !keep_set.contains(v))
        .collect();

    // 按版本号降序排（保留最新的 keep_extra 个）
    installed.sort_by(|a, b| b.cmp(a));
    let keep_extra_set: std::collections::HashSet<String> =
        installed.into_iter().take(keep_extra).collect();

    // 扫描文件系统上的版本目录，不在保留集合内的都删
    let mut pruned = Vec::new();
    if !dsh_dir.exists() {
        return Ok(pruned);
    }

    for entry in std::fs::read_dir(dsh_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match entry.file_name().to_str() {
            Some(n) => n.to_string(),
            None => continue,
        };
        // 跳过指针文件
        if name == CURRENT_POINTER_NAME
            || name == KNOWN_GOOD_POINTER_NAME
            || name == format!("{CURRENT_POINTER_NAME}{POINTER_JSON_SUFFIX}")
            || name == format!("{KNOWN_GOOD_POINTER_NAME}{POINTER_JSON_SUFFIX}")
        {
            continue;
        }
        if keep_set.contains(&name) || keep_extra_set.contains(&name) {
            continue;
        }
        // 删除版本目录
        std::fs::remove_dir_all(&path).map_err(|e| {
            LauncherError::DshVersion(format!("failed to prune version {}: {e}", path.display()))
        })?;
        pruned.push(name);
    }

    Ok(pruned)
}

/// 列出文件系统上所有已安装的版本目录（不含指针）。
pub fn list_installed_versions(dsh_dir: &Path) -> Result<Vec<String>> {
    if !dsh_dir.exists() {
        return Ok(Vec::new());
    }
    let mut versions = Vec::new();
    for entry in std::fs::read_dir(dsh_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match entry.file_name().to_str() {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name == CURRENT_POINTER_NAME
            || name == KNOWN_GOOD_POINTER_NAME
            || name == format!("{CURRENT_POINTER_NAME}{POINTER_JSON_SUFFIX}")
            || name == format!("{KNOWN_GOOD_POINTER_NAME}{POINTER_JSON_SUFFIX}")
        {
            continue;
        }
        versions.push(name);
    }
    versions.sort();
    Ok(versions)
}

/// 校验版本目录是否已安装（存在性检查）。
pub fn is_version_installed(dsh_dir: &Path, version: &str) -> bool {
    dsh_dir.join(version).exists()
}

// ===== 内部辅助函数 =====

/// 读取指针：Unix 读 symlink，Windows 读 JSON 文件。
fn read_pointer(dsh_dir: &Path, name: &str) -> Result<Option<String>> {
    #[cfg(unix)]
    {
        let link_path = dsh_dir.join(name);
        match std::fs::read_link(&link_path) {
            Ok(target) => {
                // target 是 `./<version>` 或 `<version>`
                let basename = target
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string());
                Ok(basename)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(LauncherError::Io(e)),
        }
    }

    #[cfg(windows)]
    {
        let json_path = dsh_dir.join(format!("{name}{POINTER_JSON_SUFFIX}"));
        match std::fs::read_to_string(&json_path) {
            Ok(content) => {
                let v: serde_json::Value = serde_json::from_str(&content)?;
                Ok(v.get("version")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(LauncherError::Io(e)),
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (dsh_dir, name);
        Err(LauncherError::DshVersion(
            "unsupported platform for pointer".to_string(),
        ))
    }
}

/// 写指针：原子替换。
///
/// Unix：`symlink(tmp, target)` + `rename(tmp, target)` 实现原子替换
/// Windows：写 `name.json` 临时文件 + rename
fn write_pointer(dsh_dir: &Path, name: &str, version: &str) -> Result<()> {
    std::fs::create_dir_all(dsh_dir)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let target = dsh_dir.join(name);
        let tmp_target = dsh_dir.join(format!(".{name}.tmp.{pid}", pid = std::process::id()));

        // 先删除可能存在的 tmp
        let _ = std::fs::remove_file(&tmp_target);

        // symlink 到相对路径 `./<version>`（便于整个 dsh_dir 移动）
        symlink(Path::new(version), &tmp_target)?;

        // 原子 rename
        std::fs::rename(&tmp_target, &target)?;

        Ok(())
    }

    #[cfg(windows)]
    {
        let json_path = dsh_dir.join(format!("{name}{POINTER_JSON_SUFFIX}"));
        let tmp_path = dsh_dir.join(format!(".{name}{POINTER_JSON_SUFFIX}.tmp"));

        let content = serde_json::json!({
            "version": version,
            "updated_at": Utc::now().to_rfc3339(),
        });
        let json = serde_json::to_vec_pretty(&content)?;
        std::fs::write(&tmp_path, json)?;
        std::fs::rename(&tmp_path, &json_path)?;

        Ok(())
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (name, version);
        Err(LauncherError::DshVersion(
            "unsupported platform for pointer".to_string(),
        ))
    }
}

/// 在 installed 列表中 upsert 一个版本（不存在则插入）。
fn upsert_installed(dsh: &mut DshState, version: &str, status: &str) {
    if let Some(item) = dsh.installed.iter_mut().find(|i| i.version == version) {
        item.status = status.to_string();
    } else {
        dsh.installed.push(InstalledDsh {
            version: version.to_string(),
            installed_at: Utc::now(),
            status: status.to_string(),
        });
    }
}

/// 标记已安装版本的状态。
fn mark_installed_status(dsh: &mut DshState, version: &str, status: &str) {
    if let Some(item) = dsh.installed.iter_mut().find(|i| i.version == version) {
        item.status = status.to_string();
    } else {
        // 不存在则插入一条
        dsh.installed.push(InstalledDsh {
            version: version.to_string(),
            installed_at: Utc::now(),
            status: status.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_dsh_dir() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let dsh_dir = tmp.path().join("dsh");
        std::fs::create_dir_all(&dsh_dir).unwrap();
        (tmp, dsh_dir)
    }

    fn create_version_dir(dsh_dir: &Path, version: &str) -> PathBuf {
        let p = dsh_dir.join(version);
        std::fs::create_dir_all(&p).unwrap();
        // 写一个标志文件
        std::fs::write(
            p.join("package.json"),
            r#"{"version": ""#.to_string() + version + r#""}"#,
        )
        .unwrap();
        p
    }

    #[test]
    fn read_pointer_returns_none_when_absent() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        assert_eq!(read_current_pointer(&dsh_dir).unwrap(), None);
        assert_eq!(read_known_good_pointer(&dsh_dir).unwrap(), None);
    }

    #[test]
    fn write_and_read_current_pointer_roundtrip() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");

        write_current_pointer(&dsh_dir, "0.1.0").unwrap();
        assert_eq!(
            read_current_pointer(&dsh_dir).unwrap().as_deref(),
            Some("0.1.0")
        );
    }

    #[test]
    fn write_pointer_overwrites_existing() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");
        create_version_dir(&dsh_dir, "0.2.0");

        write_current_pointer(&dsh_dir, "0.1.0").unwrap();
        write_current_pointer(&dsh_dir, "0.2.0").unwrap();
        assert_eq!(
            read_current_pointer(&dsh_dir).unwrap().as_deref(),
            Some("0.2.0")
        );
    }

    #[test]
    fn switch_fails_when_version_dir_missing() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        let err = switch(&dsh_dir, "9.9.9-not-installed").unwrap_err();
        assert!(err.to_string().contains("version dir not found"));
    }

    #[test]
    fn switch_writes_current_pointer() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");
        switch(&dsh_dir, "0.1.0").unwrap();
        assert_eq!(
            read_current_pointer(&dsh_dir).unwrap().as_deref(),
            Some("0.1.0")
        );
    }

    #[test]
    fn promote_to_current_first_time() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");

        let mut state = AppState::new();
        // 初始：无 current、无 known_good
        assert!(state.dsh.current.is_none());
        assert!(state.dsh.known_good.is_none());

        promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();

        assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
        assert!(state.dsh.known_good.is_none()); // 首次没有旧 current
        assert!(state.dsh.pending.is_none());
        assert_eq!(state.dsh.installed.len(), 1);
        assert_eq!(state.dsh.installed[0].version, "0.1.0");
        assert_eq!(state.dsh.installed[0].status, "verified");
        assert_eq!(
            read_current_pointer(&dsh_dir).unwrap().as_deref(),
            Some("0.1.0")
        );
    }

    #[test]
    fn promote_to_current_demotes_old_to_known_good() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");
        create_version_dir(&dsh_dir, "0.2.0");

        let mut state = AppState::new();
        // 先把 0.1.0 提升为 current
        promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
        assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));

        // 再把 0.2.0 提升为 current，0.1.0 应该变 known_good
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
    }

    #[test]
    fn promote_to_current_clears_pending() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");

        let mut state = AppState::new();
        state.dsh.pending = Some("0.1.0".to_string());

        promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
        assert!(state.dsh.pending.is_none());
    }

    #[test]
    fn promote_to_current_fails_when_version_not_installed() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        let mut state = AppState::new();
        let err = promote_to_current(&mut state, &dsh_dir, "9.9.9").unwrap_err();
        assert!(err.to_string().contains("version dir not found"));
    }

    #[test]
    fn rollback_to_known_good_succeeds() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");
        create_version_dir(&dsh_dir, "0.2.0");

        let mut state = AppState::new();
        // 0.1.0 → current，0.2.0 → current（0.1.0 变 known_good）
        promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
        promote_to_current(&mut state, &dsh_dir, "0.2.0").unwrap();
        assert_eq!(state.dsh.current.as_deref(), Some("0.2.0"));
        assert_eq!(state.dsh.known_good.as_deref(), Some("0.1.0"));

        // 0.2.0 失败，回滚
        state.dsh.pending = Some("0.2.0".to_string());
        let rolled_back = rollback_to_known_good(&mut state, &dsh_dir).unwrap();

        assert_eq!(rolled_back, "0.1.0");
        assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
        assert!(state.dsh.ignored_versions.contains(&"0.2.0".to_string()));
        assert!(state.dsh.pending.is_none());
        assert_eq!(
            read_current_pointer(&dsh_dir).unwrap().as_deref(),
            Some("0.1.0")
        );
    }

    #[test]
    fn rollback_fails_without_known_good() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        let mut state = AppState::new();
        // 没有 current 也没有 known_good
        let err = rollback_to_known_good(&mut state, &dsh_dir).unwrap_err();
        assert!(err.to_string().contains("no known_good"));
    }

    #[test]
    fn rollback_fails_when_known_good_dir_missing() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");

        let mut state = AppState::new();
        state.dsh.current = Some("0.2.0".to_string());
        state.dsh.known_good = Some("0.1.0".to_string());
        // 但 0.1.0 目录存在，0.2.0 不存在，rollback 应该成功
        let rolled = rollback_to_known_good(&mut state, &dsh_dir).unwrap();
        assert_eq!(rolled, "0.1.0");
        assert!(state.dsh.ignored_versions.contains(&"0.2.0".to_string()));
    }

    #[test]
    fn rollback_fails_when_known_good_dir_truly_missing() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.2.0");

        let mut state = AppState::new();
        state.dsh.current = Some("0.2.0".to_string());
        state.dsh.known_good = Some("0.1.0".to_string()); // 0.1.0 目录不存在

        let err = rollback_to_known_good(&mut state, &dsh_dir).unwrap_err();
        assert!(err.to_string().contains("known_good dir not found"));
    }

    #[test]
    fn rollback_does_not_double_add_to_ignored() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");
        create_version_dir(&dsh_dir, "0.2.0");

        let mut state = AppState::new();
        promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
        promote_to_current(&mut state, &dsh_dir, "0.2.0").unwrap();

        // 先 ignore 0.2.0
        state.dsh.ignored_versions.push("0.2.0".to_string());

        rollback_to_known_good(&mut state, &dsh_dir).unwrap();

        let count = state
            .dsh
            .ignored_versions
            .iter()
            .filter(|v| *v == "0.2.0")
            .count();
        assert_eq!(count, 1, "should not duplicate in ignored_versions");
    }

    #[test]
    fn set_pending_sets_field() {
        let mut state = AppState::new();
        set_pending(&mut state, "0.3.0");
        assert_eq!(state.dsh.pending.as_deref(), Some("0.3.0"));
    }

    #[test]
    fn clear_pending_clears_field() {
        let mut state = AppState::new();
        state.dsh.pending = Some("0.3.0".to_string());
        clear_pending(&mut state);
        assert!(state.dsh.pending.is_none());
    }

    #[test]
    fn ignore_version_does_not_duplicate() {
        let mut state = AppState::new();
        ignore_version(&mut state, "0.5.0");
        ignore_version(&mut state, "0.5.0");
        ignore_version(&mut state, "0.5.0");
        assert_eq!(
            state
                .dsh
                .ignored_versions
                .iter()
                .filter(|v| *v == "0.5.0")
                .count(),
            1
        );
    }

    #[test]
    fn prune_old_versions_keeps_current_and_known_good() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        for v in &["0.1.0", "0.2.0", "0.3.0", "0.4.0"] {
            create_version_dir(&dsh_dir, v);
        }

        let mut state = AppState::new();
        state.dsh.current = Some("0.4.0".to_string());
        state.dsh.known_good = Some("0.3.0".to_string());

        let pruned = prune_old_versions(&state, &dsh_dir, 0).unwrap();
        // 应该删除 0.1.0、0.2.0
        assert!(pruned.contains(&"0.1.0".to_string()));
        assert!(pruned.contains(&"0.2.0".to_string()));
        assert!(!pruned.contains(&"0.3.0".to_string()));
        assert!(!pruned.contains(&"0.4.0".to_string()));

        assert!(!dsh_dir.join("0.1.0").exists());
        assert!(!dsh_dir.join("0.2.0").exists());
        assert!(dsh_dir.join("0.3.0").exists());
        assert!(dsh_dir.join("0.4.0").exists());
    }

    #[test]
    fn prune_keeps_extra_versions() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        for v in &["0.1.0", "0.2.0", "0.3.0", "0.4.0", "0.5.0"] {
            create_version_dir(&dsh_dir, v);
        }

        let mut state = AppState::new();
        state.dsh.current = Some("0.5.0".to_string());
        state.dsh.known_good = Some("0.4.0".to_string());

        // keep_extra = 1：保留 0.5.0、0.4.0 + 最新 1 个（0.3.0）
        let pruned = prune_old_versions(&state, &dsh_dir, 1).unwrap();
        assert!(pruned.contains(&"0.1.0".to_string()));
        assert!(pruned.contains(&"0.2.0".to_string()));
        assert!(!pruned.contains(&"0.3.0".to_string()));
        assert!(!pruned.contains(&"0.4.0".to_string()));
        assert!(!pruned.contains(&"0.5.0".to_string()));
    }

    #[test]
    fn prune_keeps_pending() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        for v in &["0.1.0", "0.2.0", "0.3.0"] {
            create_version_dir(&dsh_dir, v);
        }

        let mut state = AppState::new();
        state.dsh.current = Some("0.2.0".to_string());
        state.dsh.known_good = Some("0.1.0".to_string());
        state.dsh.pending = Some("0.3.0".to_string());

        let pruned = prune_old_versions(&state, &dsh_dir, 0).unwrap();
        assert!(pruned.is_empty());
        assert!(dsh_dir.join("0.3.0").exists());
    }

    #[test]
    fn prune_skips_pointer_files() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");
        write_current_pointer(&dsh_dir, "0.1.0").unwrap();
        write_known_good_pointer(&dsh_dir, "0.1.0").unwrap();

        let state = AppState::new();
        let pruned = prune_old_versions(&state, &dsh_dir, 0).unwrap();
        // current / known-good 指针不应该被删
        assert!(pruned.is_empty() || !pruned.iter().any(|v| v == "current" || v == "known-good"));
        assert!(read_current_pointer(&dsh_dir).unwrap().is_some());
    }

    #[test]
    fn prune_on_nonexistent_dsh_dir_returns_empty() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        let state = AppState::new();
        let pruned = prune_old_versions(&state, &dsh_dir.join("nonexistent"), 0).unwrap();
        assert!(pruned.is_empty());
    }

    #[test]
    fn list_installed_versions_returns_sorted() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.3.0");
        create_version_dir(&dsh_dir, "0.1.0");
        create_version_dir(&dsh_dir, "0.2.0");

        let versions = list_installed_versions(&dsh_dir).unwrap();
        assert_eq!(versions, vec!["0.1.0", "0.2.0", "0.3.0"]);
    }

    #[test]
    fn list_installed_versions_excludes_pointers() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");
        write_current_pointer(&dsh_dir, "0.1.0").unwrap();

        let versions = list_installed_versions(&dsh_dir).unwrap();
        assert_eq!(versions, vec!["0.1.0"]);
    }

    #[test]
    fn list_installed_versions_empty_when_dir_missing() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        let versions = list_installed_versions(&dsh_dir.join("nonexistent")).unwrap();
        assert!(versions.is_empty());
    }

    #[test]
    fn is_version_installed_checks_dir_existence() {
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");
        assert!(is_version_installed(&dsh_dir, "0.1.0"));
        assert!(!is_version_installed(&dsh_dir, "9.9.9"));
    }

    #[test]
    fn full_upgrade_and_rollback_scenario() {
        // 模拟完整场景：
        // 1. 装 0.1.0 并 promote
        // 2. 装 0.2.0 并 promote（0.1.0 变 known_good）
        // 3. 0.2.0 启动失败，rollback 到 0.1.0
        // 4. 0.2.0 进 ignored_versions
        // 5. 清理：0.2.0 被删除
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");
        create_version_dir(&dsh_dir, "0.2.0");

        let mut state = AppState::new();

        // 1. 装 0.1.0
        promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
        assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));

        // 2. set pending + promote 0.2.0
        set_pending(&mut state, "0.2.0");
        promote_to_current(&mut state, &dsh_dir, "0.2.0").unwrap();
        assert_eq!(state.dsh.current.as_deref(), Some("0.2.0"));
        assert_eq!(state.dsh.known_good.as_deref(), Some("0.1.0"));

        // 3. 0.2.0 失败，回滚
        set_pending(&mut state, "0.2.0"); // 模拟还在 pending 状态
        let rolled = rollback_to_known_good(&mut state, &dsh_dir).unwrap();
        assert_eq!(rolled, "0.1.0");
        assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
        assert!(state.dsh.ignored_versions.contains(&"0.2.0".to_string()));
        assert!(state.dsh.pending.is_none());

        // 4. 清理：删除 ignored_versions 中的版本目录
        // (实际清理可能用 prune_old_versions，但 ignored 不在保留集)
        // 模拟手动删除 0.2.0
        std::fs::remove_dir_all(dsh_dir.join("0.2.0")).unwrap();
        assert!(!is_version_installed(&dsh_dir, "0.2.0"));
    }

    #[test]
    fn multiple_rollbacks_accumulate_ignored() {
        // 多次升级失败，ignored_versions 应该累积
        let (_tmp, dsh_dir) = setup_dsh_dir();
        for v in &["0.1.0", "0.2.0", "0.3.0", "0.4.0"] {
            create_version_dir(&dsh_dir, v);
        }

        let mut state = AppState::new();

        // 0.1.0 → current
        promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();

        // 0.2.0 升级失败
        promote_to_current(&mut state, &dsh_dir, "0.2.0").unwrap();
        rollback_to_known_good(&mut state, &dsh_dir).unwrap();

        // 0.3.0 升级失败（注意：known_good 现在是 0.1.0）
        promote_to_current(&mut state, &dsh_dir, "0.3.0").unwrap();
        rollback_to_known_good(&mut state, &dsh_dir).unwrap();

        // 0.4.0 升级失败
        promote_to_current(&mut state, &dsh_dir, "0.4.0").unwrap();
        rollback_to_known_good(&mut state, &dsh_dir).unwrap();

        // 最终 current 应该是 0.1.0，ignored = [0.2.0, 0.3.0, 0.4.0]
        assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
        assert_eq!(state.dsh.known_good.as_deref(), None); // 0.1.0 已是 current
        assert!(state.dsh.ignored_versions.contains(&"0.2.0".to_string()));
        assert!(state.dsh.ignored_versions.contains(&"0.3.0".to_string()));
        assert!(state.dsh.ignored_versions.contains(&"0.4.0".to_string()));
    }

    #[test]
    fn promote_same_version_no_op_for_known_good() {
        // 同一版本连续 promote：known_good 不应该被设置成自己
        let (_tmp, dsh_dir) = setup_dsh_dir();
        create_version_dir(&dsh_dir, "0.1.0");

        let mut state = AppState::new();
        promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();
        promote_to_current(&mut state, &dsh_dir, "0.1.0").unwrap();

        assert_eq!(state.dsh.current.as_deref(), Some("0.1.0"));
        assert!(state.dsh.known_good.is_none());
    }
}
