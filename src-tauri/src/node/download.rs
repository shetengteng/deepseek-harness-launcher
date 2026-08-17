//! Node 下载与校验（设计 §M2.2）。

mod cancellation;
mod checksum;
mod transfer;

use std::path::{Path, PathBuf};

use reqwest::Client;
use tokio::sync::mpsc;

use crate::error::{LauncherError, Result};
use crate::node::mirror::Mirror;

pub(crate) use cancellation::DownloadCancellation;
pub use cancellation::NodeDownloadOperations;
pub use checksum::{fetch_shasums, find_sha_in_shasums, parse_shasums_line, verify_sha256};
pub use transfer::download_archive;

/// 进度事件。通过 `mpsc::Sender` 推到调用方，Tauri 命令层可转 `emit`。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ProgressEvent {
    /// 阶段标签：`download` / `verify` / `extract`。
    pub stage: String,
    /// 已处理字节数。
    pub bytes: u64,
    /// 总字节数（`Content-Length`），未知为 `None`。
    pub total: Option<u64>,
}

/// 进度触发阈值。每 64KB 推一次，避免 channel 过载。
pub const PROGRESS_CHUNK_BYTES: u64 = 64 * 1024;

/// 下载单个镜像源 + 校验。失败重试 `max_retries` 次。
///
/// 测试设计 §PR-007 用例：5xx 重试 3 次后 Err。
pub async fn download_with_retry(
    client: &Client,
    mirror: &Mirror,
    version: &str,
    archive_filename: &str,
    dest_dir: &Path,
    progress_tx: Option<&mpsc::Sender<ProgressEvent>>,
    max_retries: u32,
    operations: &NodeDownloadOperations,
    operation_id: &str,
) -> Result<PathBuf> {
    let archive_url = format!(
        "{}/v{version}/{archive_filename}",
        mirror.base_url.trim_end_matches('/')
    );
    let dest_path = dest_dir.join(archive_filename);
    let mut last_err: Option<LauncherError> = None;

    for attempt in 1..=max_retries {
        tracing::info!(
            attempt,
            max_retries,
            mirror = %mirror.id,
            url = %archive_url,
            "downloading node archive"
        );
        let mut active_download = operations.register(operation_id)?;
        let download_result = download_archive(
            client,
            &archive_url,
            &dest_path,
            progress_tx,
            Some(active_download.cancellation()),
        )
        .await;
        drop(active_download);

        match download_result {
            Ok(_) => match fetch_shasums(client, mirror, version).await {
                Ok(shasums) => match find_sha_in_shasums(&shasums, archive_filename) {
                    Ok(expected_sha) => match verify_sha256(&dest_path, &expected_sha).await {
                        Ok(()) => return Ok(dest_path),
                        Err(error) => {
                            tracing::warn!(attempt, %error, "SHA verify failed");
                            let _ = tokio::fs::remove_file(&dest_path).await;
                            last_err = Some(error);
                        }
                    },
                    Err(error) => {
                        tracing::warn!(attempt, %error, archive_filename, "SHA entry not found");
                        let _ = tokio::fs::remove_file(&dest_path).await;
                        last_err = Some(error);
                    }
                },
                Err(error) => {
                    tracing::warn!(attempt, %error, "fetch SHASUMS failed");
                    let _ = tokio::fs::remove_file(&dest_path).await;
                    last_err = Some(error);
                }
            },
            Err(error @ LauncherError::NodeInstallCancelled { .. }) => return Err(error),
            Err(error) => {
                if matches!(error, LauncherError::Io(_)) {
                    tracing::error!(attempt, %error, "local io error, aborting retries");
                    let _ = tokio::fs::remove_file(&dest_path).await;
                    return Err(error);
                }
                tracing::warn!(attempt, %error, "download failed");
                last_err = Some(error);
            }
        }
    }

    let _ = tokio::fs::remove_file(&dest_path).await;
    Err(last_err
        .map(|error| {
            LauncherError::NodeDownload(format!(
                "all {max_retries} attempts failed for {archive_filename} via {}: {error}",
                mirror.base_url.trim_end_matches('/')
            ))
        })
        .unwrap_or_else(|| {
            LauncherError::NodeDownload(format!(
                "download exhausted retries for {archive_filename}"
            ))
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_event_serializes_to_snake_case() {
        let event = ProgressEvent {
            stage: "download".to_string(),
            bytes: 1024,
            total: Some(4096),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["stage"], "download");
        assert_eq!(json["bytes"], 1024);
        assert_eq!(json["total"], 4096);
    }
}
