//! 磁盘空间检查（设计 M4.4 / PR-020）。
//!
//! 下载 Node archive（~25MB 解压后 ~80MB）前检查目标盘可用空间 ≥ 200MB。
//! 用 `fs2::available_space` 跨平台读取 statvfs / GetDiskFreeSpaceEx。

use std::path::Path;

use crate::error::{LauncherError, Result};

/// 下载前要求的最低可用空间。
pub const MIN_FREE_BYTES: u64 = 200 * 1024 * 1024;

/// 读取 `dir` 所在盘的可用字节数。
pub fn available_space(dir: &Path) -> Result<u64> {
    // fs2::available_space 接受路径（内部走 statvfs / GetDiskFreeSpaceEx）。
    fs2::available_space(dir).map_err(LauncherError::Io)
}

/// 下载前检查：`dir` 所在盘可用空间是否 ≥ `MIN_FREE_BYTES`。
/// 不足时返回 `NodeDownload` 错误（user_message 会映射为磁盘空间提示）。
pub fn ensure_disk_space(dir: &Path) -> Result<()> {
    let free = available_space(dir)?;
    if free < MIN_FREE_BYTES {
        return Err(LauncherError::NodeDownload(format!(
            "insufficient disk space: {} bytes free at {}, need at least {} bytes",
            free,
            dir.display(),
            MIN_FREE_BYTES
        )));
    }
    Ok(())
}

/// 判定给定可用字节数是否满足下载要求。纯函数，便于单测。
pub fn has_enough_space(free_bytes: u64) -> bool {
    free_bytes >= MIN_FREE_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_is_200mb() {
        assert_eq!(MIN_FREE_BYTES, 200 * 1024 * 1024);
    }

    #[test]
    fn has_enough_space_boundaries() {
        assert!(has_enough_space(MIN_FREE_BYTES));
        assert!(has_enough_space(MIN_FREE_BYTES + 1));
        assert!(!has_enough_space(MIN_FREE_BYTES - 1));
        assert!(!has_enough_space(0));
    }

    #[test]
    fn available_space_reads_real_disk() {
        // tmp 目录在开发机上真实存在且空间充足
        let free = available_space(std::env::temp_dir().as_path()).expect("read free space");
        assert!(free > 0);
        assert!(has_enough_space(free));
    }

    #[test]
    fn ensure_disk_space_passes_on_dev_machine() {
        ensure_disk_space(std::env::temp_dir().as_path()).expect("dev machine has space");
    }

    #[test]
    fn ensure_disk_space_errors_on_missing_dir() {
        let err = ensure_disk_space(Path::new("/nonexistent-definitely-missing-dir"))
            .expect_err("missing dir errors");
        assert!(matches!(err, LauncherError::Io(_)));
    }
}
