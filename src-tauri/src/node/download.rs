//! Node 下载与校验（设计 §M2.2）。
//!
//! 流程：
//! 1. `download_node(version, mirror, client, dest_dir, progress_tx)` → 写临时文件
//! 2. `verify_sha256(archive_path, expected_sha)` → 校验
//! 3. `fetch_shasums(client, mirror, version)` → 拉 `SHASUMS256.txt` 并解析同名条目
//!
//! 失败重试：单镜像最多 2 次（设计 §M2.2），全镜像失败抛 `NodeDownloadExhausted`。
//! 进度事件：每 64KB 触发一次 `ProgressEvent { stage, bytes, total }`（设计 §M2.2）。

use std::path::{Path, PathBuf};

use reqwest::Client;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::error::{LauncherError, Result};
use crate::node::mirror::Mirror;

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

/// SHASUMS256.txt 中的一行：`<hash>  <filename>`。
pub fn parse_shasums_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    let mut parts = line.split_whitespace();
    let hash = parts.next()?.to_string();
    let filename = parts.next()?.to_string();
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some((hash, filename))
}

/// 从 SHASUMS256.txt 文本中找到对应 archive 的 SHA-256。
///
/// `archive_filename` 形如 `node-v22.19.0-darwin-arm64-arm64.tar.gz`。
pub fn find_sha_in_shasums(shasums: &str, archive_filename: &str) -> Result<String> {
    for line in shasums.lines() {
        if let Some((hash, name)) = parse_shasums_line(line) {
            if name == archive_filename {
                return Ok(hash);
            }
        }
    }
    Err(LauncherError::NodeDownload(format!(
        "SHA-256 for '{archive_filename}' not found in SHASUMS256.txt"
    )))
}

/// 从镜像源拉取 SHASUMS256.txt 全文。
pub async fn fetch_shasums(client: &Client, mirror: &Mirror, version: &str) -> Result<String> {
    let url = mirror.shasums_url(version);
    tracing::debug!(url = %url, "fetching SHASUMS256.txt");
    let resp =
        client.get(&url).send().await.map_err(|e| {
            LauncherError::NodeDownload(format!("fetch SHASUMS256.txt failed: {e}"))
        })?;
    if !resp.status().is_success() {
        return Err(LauncherError::NodeDownload(format!(
            "fetch SHASUMS256.txt returned {} for {}",
            resp.status(),
            url
        )));
    }
    let body = resp.text().await.map_err(|e| {
        LauncherError::NodeDownload(format!("read SHASUMS256.txt body failed: {e}"))
    })?;
    Ok(body)
}

/// 流式下载 Node archive 到 `dest_path`，每 64KB 通过 `progress_tx` 推一次进度。
///
/// 返回下载的总字节数。
pub async fn download_archive(
    client: &Client,
    url: &str,
    dest_path: &Path,
    progress_tx: Option<&mpsc::Sender<ProgressEvent>>,
) -> Result<u64> {
    tracing::debug!(url = %url, dest = %dest_path.display(), "downloading archive");
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| LauncherError::NodeDownload(format!("GET {url} failed: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        // 测试设计 §PR-007 用例：404 / 5xx
        return Err(LauncherError::NodeDownload(format!(
            "GET {url} returned {status}"
        )));
    }
    let total = resp.content_length();

    // 防御性检查：某些镜像源（如 npmmirror/S3）对不存在的 key 返回 200 + XML 错误页，
    // 而非 404。若 Content-Length 明显过小（< 1MB），几乎不可能是真实的 Node archive
    // （最小的 node-v22 tar.gz 也有 ~20MB），直接拒绝避免后续浪费 SHA 校验时间。
    if let Some(cl) = total {
        const MIN_ARCHIVE_BYTES: u64 = 1_000_000; // 1MB
        if cl < MIN_ARCHIVE_BYTES {
            return Err(LauncherError::NodeDownload(format!(
                "GET {url} returned suspiciously small body (Content-Length: {cl} bytes); \
                 likely an error page (XML/HTML) rather than a real archive"
            )));
        }
    }

    // 写到 `.part` 临时文件，校验通过后再 rename，避免中途崩溃留下半成品。
    let part_path = dest_path.with_extension("tar.gz.part");
    let mut file = tokio::fs::File::create(&part_path)
        .await
        .map_err(LauncherError::Io)?;

    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut bytes_written: u64 = 0;
    let mut last_progress_at: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            // 测试设计 §PR-007 用例：下载中断
            // 清理 .part 文件，避免污染
            let part_path = part_path.clone();
            tokio::spawn(async move {
                let _ = tokio::fs::remove_file(&part_path).await;
            });
            LauncherError::NodeDownload(format!("download stream error: {e}"))
        })?;
        file.write_all(&chunk).await.map_err(LauncherError::Io)?;
        bytes_written += chunk.len() as u64;

        // 每 PROGRESS_CHUNK_BYTES 推一次进度
        if let Some(tx) = progress_tx {
            if bytes_written - last_progress_at >= PROGRESS_CHUNK_BYTES {
                last_progress_at = bytes_written;
                let _ = tx.try_send(ProgressEvent {
                    stage: "download".to_string(),
                    bytes: bytes_written,
                    total,
                });
            }
        }
    }

    file.flush().await.map_err(LauncherError::Io)?;
    drop(file);

    // 流式下载完成后再次校验实际字节数，防止服务器返回不带 Content-Length 的小错误页
    if bytes_written < 1_000_000 {
        // 读取前 256 字节用于错误诊断（在删除之前读取）
        let diag = tokio::fs::read(&part_path).await.unwrap_or_default();
        let mut prefix = diag.clone();
        prefix.truncate(256);
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(LauncherError::NodeDownload(format!(
            "downloaded body too small ({bytes_written} bytes); likely an error page. First 256 bytes: {}",
            String::from_utf8_lossy(&prefix)
        )));
    }

    // rename .part → .tar.gz
    tokio::fs::rename(&part_path, dest_path)
        .await
        .map_err(LauncherError::Io)?;

    // 推一次最终进度
    if let Some(tx) = progress_tx {
        let _ = tx.try_send(ProgressEvent {
            stage: "download".to_string(),
            bytes: bytes_written,
            total,
        });
    }

    tracing::debug!(bytes = bytes_written, "download complete");
    Ok(bytes_written)
}

/// 校验文件 SHA-256 与期望值一致。
pub async fn verify_sha256(path: &Path, expected_sha: &str) -> Result<()> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(LauncherError::Io)?;
    let mut hasher = Sha256::new();
    use tokio::io::AsyncReadExt;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).await.map_err(LauncherError::Io)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = hasher.finalize();
    let actual_hex = hex::encode(actual);
    if actual_hex != expected_sha {
        // 测试设计 §PR-007 用例：SHA 不匹配，删除文件
        let _ = tokio::fs::remove_file(path).await;
        return Err(LauncherError::NodeDownload(format!(
            "SHA-256 mismatch for {}: expected {expected_sha}, got {actual_hex}",
            path.display()
        )));
    }
    Ok(())
}

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
        match download_archive(client, &archive_url, &dest_path, progress_tx).await {
            Ok(_) => {
                // 下载完成，校验 SHA
                match fetch_shasums(client, mirror, version).await {
                    Ok(shasums) => match find_sha_in_shasums(&shasums, archive_filename) {
                        Ok(expected_sha) => match verify_sha256(&dest_path, &expected_sha).await {
                            Ok(()) => return Ok(dest_path),
                            Err(e) => {
                                tracing::warn!(attempt, error = %e, "SHA verify failed");
                                let _ = tokio::fs::remove_file(&dest_path).await;
                                last_err = Some(e);
                            }
                        },
                        Err(e) => {
                            // SHASUMS 里找不到对应文件名 → 几乎肯定是 archive_filename 拼接错误
                            tracing::warn!(attempt, error = %e, archive_filename, "SHA entry not found");
                            let _ = tokio::fs::remove_file(&dest_path).await;
                            last_err = Some(e);
                        }
                    },
                    Err(e) => {
                        tracing::warn!(attempt, error = %e, "fetch SHASUMS failed");
                        let _ = tokio::fs::remove_file(&dest_path).await;
                        last_err = Some(e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!(attempt, error = %e, "download failed");
                last_err = Some(e);
            }
        }
    }
    // 最后一次清理残留（download_archive 内部已清理 .part，但可能残留最终文件）
    let _ = tokio::fs::remove_file(&dest_path).await;
    Err(last_err.unwrap_or_else(|| {
        LauncherError::NodeDownload(format!("download exhausted retries for {archive_filename}"))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_shasums_line_valid() {
        // 64 字符 hex
        let hash = "a".repeat(64);
        let line = format!("{hash}  node-v22.19.0-darwin-arm64-arm64.tar.gz");
        let (h, name) = parse_shasums_line(&line).unwrap();
        assert_eq!(h, hash);
        assert_eq!(name, "node-v22.19.0-darwin-arm64-arm64.tar.gz");
    }

    #[test]
    fn parse_shasums_line_handles_leading_whitespace() {
        // Node 官方 SHASUMS256.txt 用两个空格分隔，有时行首也有空格
        let hash = "b".repeat(64);
        let line = format!("  {hash}  node-v22.19.0-darwin-arm64-arm64.tar.gz");
        let (h, name) = parse_shasums_line(&line).unwrap();
        assert_eq!(h, hash);
        assert_eq!(name, "node-v22.19.0-darwin-arm64-arm64.tar.gz");
    }

    #[test]
    fn parse_shasums_line_rejects_short_hash() {
        assert!(parse_shasums_line("short  file.tar.gz").is_none());
    }

    #[test]
    fn parse_shasums_line_rejects_non_hex() {
        // 长度对但含非十六进制字符
        let non_hex = "x".repeat(64);
        let line = format!("{non_hex}  file.tar.gz");
        assert!(parse_shasums_line(&line).is_none());
    }

    #[test]
    fn find_sha_in_shasums_finds_matching_filename() {
        let h1 = "d".repeat(64);
        let h2 = "c".repeat(64);
        let shasums = format!(
            "{h1}  node-v22.19.0-linux-x64.tar.gz\n{h2}  node-v22.19.0-darwin-arm64-arm64.tar.gz\n"
        );
        let sha = find_sha_in_shasums(&shasums, "node-v22.19.0-darwin-arm64-arm64.tar.gz").unwrap();
        assert_eq!(sha, h2);
    }

    #[test]
    fn find_sha_in_shasums_errors_when_not_found() {
        let shasums = "deadbeef...  other.tar.gz";
        let err =
            find_sha_in_shasums(shasums, "node-v22.19.0-darwin-arm64-arm64.tar.gz").unwrap_err();
        assert!(matches!(err, LauncherError::NodeDownload(_)));
    }

    #[tokio::test]
    async fn verify_sha256_matches_expected() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let data = b"hello world";
        tokio::fs::write(tmp.path(), data).await.unwrap();
        // sha256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        verify_sha256(
            tmp.path(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn verify_sha256_rejects_mismatch_and_deletes_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        tokio::fs::write(tmp.path(), b"hello world").await.unwrap();
        let err = verify_sha256(
            tmp.path(),
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .await
        .unwrap_err();
        assert!(matches!(err, LauncherError::NodeDownload(_)));
        // 文件应被删除
        assert!(!tmp.path().exists());
    }

    #[test]
    fn progress_event_serializes_to_snake_case() {
        let ev = ProgressEvent {
            stage: "download".to_string(),
            bytes: 1024,
            total: Some(4096),
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["stage"], "download");
        assert_eq!(json["bytes"], 1024);
        assert_eq!(json["total"], 4096);
    }
}
