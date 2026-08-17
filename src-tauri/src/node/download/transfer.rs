use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use reqwest::Client;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::error::{LauncherError, Result};

use super::{DownloadCancellation, ProgressEvent, PROGRESS_CHUNK_BYTES};

const MIN_ARCHIVE_BYTES: u64 = 1_000_000;

/// 流式下载 Node archive 到 `dest_path`，每 64KB 通过 `progress_tx` 推一次进度。
pub async fn download_archive(
    client: &Client,
    url: &str,
    dest_path: &Path,
    progress_tx: Option<&mpsc::Sender<ProgressEvent>>,
    cancellation: Option<DownloadCancellation>,
) -> Result<u64> {
    tracing::debug!(%url, dest = %dest_path.display(), "downloading archive");
    let part_path = archive_part_path(dest_path);
    let request = client.get(url).send();
    let response = match cancellation.as_ref() {
        Some(cancel) => {
            let mut receiver = cancel.receiver();
            tokio::select! {
                _ = receiver.changed() => {
                    let error = cancel.error();
                    cleanup_archive_artifacts(&part_path, dest_path).await;
                    return Err(error);
                }
                response = request => response,
            }
        }
        None => request.await,
    }
    .map_err(|error| LauncherError::NodeDownload(format!("GET {url} failed: {error}")))?;
    if !response.status().is_success() {
        return Err(LauncherError::NodeDownload(format!(
            "GET {url} returned {}",
            response.status()
        )));
    }

    let total = response.content_length();
    if total.is_some_and(|length| length < MIN_ARCHIVE_BYTES) {
        return Err(LauncherError::NodeDownload(format!(
            "GET {url} returned suspiciously small body (Content-Length: {} bytes); likely an error page (XML/HTML) rather than a real archive",
            total.unwrap()
        )));
    }

    let mut file = tokio::fs::File::create(&part_path)
        .await
        .map_err(LauncherError::Io)?;
    let mut stream = response.bytes_stream();
    let mut bytes_written = 0;
    let mut last_progress_at = 0;

    loop {
        let chunk = match cancellation.as_ref() {
            Some(cancel) => {
                let mut receiver = cancel.receiver();
                tokio::select! {
                    _ = receiver.changed() => {
                        let error = cancel.error();
                        drop(file);
                        cleanup_archive_artifacts(&part_path, dest_path).await;
                        return Err(error);
                    }
                    chunk = stream.next() => chunk,
                }
            }
            None => stream.next().await,
        };
        let Some(chunk) = chunk else {
            break;
        };
        let chunk = chunk.map_err(|error| {
            let part_path = part_path.clone();
            tokio::spawn(async move {
                let _ = tokio::fs::remove_file(part_path).await;
            });
            LauncherError::NodeDownload(format!("download stream error: {error}"))
        })?;
        file.write_all(&chunk).await.map_err(LauncherError::Io)?;
        bytes_written += chunk.len() as u64;
        if bytes_written - last_progress_at >= PROGRESS_CHUNK_BYTES {
            last_progress_at = bytes_written;
            send_progress(progress_tx, bytes_written, total);
        }
    }
    file.flush().await.map_err(LauncherError::Io)?;
    drop(file);

    if bytes_written < MIN_ARCHIVE_BYTES {
        let mut prefix = tokio::fs::read(&part_path).await.unwrap_or_default();
        prefix.truncate(256);
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(LauncherError::NodeDownload(format!(
            "downloaded body too small ({bytes_written} bytes); likely an error page. First 256 bytes: {}",
            String::from_utf8_lossy(&prefix)
        )));
    }

    tokio::fs::rename(&part_path, dest_path)
        .await
        .map_err(LauncherError::Io)?;
    send_progress(progress_tx, bytes_written, total);
    tracing::debug!(bytes = bytes_written, "download complete");
    Ok(bytes_written)
}

fn archive_part_path(dest_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.part", dest_path.display()))
}

async fn cleanup_archive_artifacts(part_path: &Path, dest_path: &Path) {
    let _ = tokio::fs::remove_file(part_path).await;
    let _ = tokio::fs::remove_file(dest_path).await;
}

fn send_progress(
    progress_tx: Option<&mpsc::Sender<ProgressEvent>>,
    bytes: u64,
    total: Option<u64>,
) {
    if let Some(sender) = progress_tx {
        let _ = sender.try_send(ProgressEvent {
            stage: "download".to_string(),
            bytes,
            total,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::net::TcpListener;

    use super::*;
    use crate::node::download::NodeDownloadOperations;

    #[tokio::test]
    async fn cancellation_aborts_pending_request_and_removes_artifacts() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let _connection = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(5)).await;
        });
        let directory = tempfile::tempdir().unwrap();
        let archive_path = directory.path().join("node.tar.gz");
        let operations = Arc::new(NodeDownloadOperations::default());
        let cancellation_operations = Arc::clone(&operations);
        let cancel = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(25)).await;
            assert!(cancellation_operations.cancel("operation-1"));
        });
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let active = operations.register("operation-1").unwrap();

        let error = download_archive(
            &client,
            &format!("http://{address}/node.tar.gz"),
            &archive_path,
            None,
            Some(active.cancellation()),
        )
        .await
        .unwrap_err();

        cancel.await.unwrap();
        server.abort();
        assert!(matches!(error, LauncherError::NodeInstallCancelled { .. }));
        assert!(!archive_path.exists());
        assert!(!archive_part_path(&archive_path).exists());
    }
}
