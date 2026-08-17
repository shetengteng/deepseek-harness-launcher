use std::path::Path;

use reqwest::Client;
use sha2::{Digest, Sha256};

use crate::error::{LauncherError, Result};
use crate::node::mirror::Mirror;

use super::DownloadCancellation;

/// SHASUMS256.txt 中的一行：`<hash>  <filename>`。
pub fn parse_shasums_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.trim().split_whitespace();
    let hash = parts.next()?.to_string();
    let filename = parts.next()?.to_string();
    (hash.len() == 64 && hash.chars().all(|character| character.is_ascii_hexdigit()))
        .then_some((hash, filename))
}

/// 从 SHASUMS256.txt 文本中找到对应 archive 的 SHA-256。
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
    fetch_shasums_inner(client, mirror, version, None).await
}

pub(crate) async fn fetch_shasums_cancellable(
    client: &Client,
    mirror: &Mirror,
    version: &str,
    cancellation: &DownloadCancellation,
) -> Result<String> {
    fetch_shasums_inner(client, mirror, version, Some(cancellation)).await
}

async fn fetch_shasums_inner(
    client: &Client,
    mirror: &Mirror,
    version: &str,
    cancellation: Option<&DownloadCancellation>,
) -> Result<String> {
    check_cancelled(cancellation)?;
    let url = mirror.shasums_url(version);
    tracing::debug!(%url, "fetching SHASUMS256.txt");
    let request = client.get(&url).send();
    let response = match cancellation {
        Some(cancellation) => {
            let mut receiver = cancellation.receiver();
            tokio::select! {
                _ = receiver.changed() => return Err(cancellation.error()),
                response = request => response,
            }
        }
        None => request.await,
    }
    .map_err(|error| {
        LauncherError::NodeDownload(format!("fetch SHASUMS256.txt failed: {error}"))
    })?;
    if !response.status().is_success() {
        return Err(LauncherError::NodeDownload(format!(
            "fetch SHASUMS256.txt returned {} for {url}",
            response.status()
        )));
    }

    let body = response.text();
    match cancellation {
        Some(cancellation) => {
            let mut receiver = cancellation.receiver();
            tokio::select! {
                _ = receiver.changed() => Err(cancellation.error()),
                body = body => body.map_err(|error| LauncherError::NodeDownload(format!("read SHASUMS256.txt body failed: {error}"))),
            }
        }
        None => body.await.map_err(|error| {
            LauncherError::NodeDownload(format!("read SHASUMS256.txt body failed: {error}"))
        }),
    }
}

/// 校验文件 SHA-256 与期望值一致。
pub async fn verify_sha256(path: &Path, expected_sha: &str) -> Result<()> {
    verify_sha256_inner(path, expected_sha, None).await
}

pub(crate) async fn verify_sha256_cancellable(
    path: &Path,
    expected_sha: &str,
    cancellation: &DownloadCancellation,
) -> Result<()> {
    verify_sha256_inner(path, expected_sha, Some(cancellation)).await
}

async fn verify_sha256_inner(
    path: &Path,
    expected_sha: &str,
    cancellation: Option<&DownloadCancellation>,
) -> Result<()> {
    use tokio::io::AsyncReadExt;

    check_cancelled(cancellation)?;
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(LauncherError::Io)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        check_cancelled(cancellation)?;
        let count = file.read(&mut buffer).await.map_err(LauncherError::Io)?;
        check_cancelled(cancellation)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    let actual_sha = hex::encode(hasher.finalize());
    if actual_sha != expected_sha {
        let _ = tokio::fs::remove_file(path).await;
        return Err(LauncherError::NodeDownload(format!(
            "SHA-256 mismatch for {}: expected {expected_sha}, got {actual_sha}",
            path.display()
        )));
    }
    Ok(())
}

fn check_cancelled(cancellation: Option<&DownloadCancellation>) -> Result<()> {
    cancellation.map_or(Ok(()), DownloadCancellation::check)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::download::NodeDownloadOperations;

    #[test]
    fn parses_valid_shasums_lines() {
        let hash = "a".repeat(64);
        let line = format!("  {hash}  node-v22.19.0-darwin-arm64-arm64.tar.gz");
        assert_eq!(
            parse_shasums_line(&line),
            Some((hash, "node-v22.19.0-darwin-arm64-arm64.tar.gz".to_string()))
        );
    }

    #[test]
    fn rejects_invalid_shasums_hashes() {
        assert!(parse_shasums_line("short file.tar.gz").is_none());
        assert!(parse_shasums_line(&format!("{} file.tar.gz", "x".repeat(64))).is_none());
    }

    #[test]
    fn finds_sha_for_matching_archive() {
        let expected = "c".repeat(64);
        let shasums = format!(
            "{}  linux.tar.gz\n{expected}  darwin.tar.gz\n",
            "d".repeat(64)
        );
        assert_eq!(
            find_sha_in_shasums(&shasums, "darwin.tar.gz").unwrap(),
            expected
        );
        assert!(find_sha_in_shasums(&shasums, "missing.tar.gz").is_err());
    }

    #[tokio::test]
    async fn verifies_sha_and_removes_mismatches() {
        let file = tempfile::NamedTempFile::new().unwrap();
        tokio::fs::write(file.path(), b"hello world").await.unwrap();
        verify_sha256(
            file.path(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
        )
        .await
        .unwrap();

        tokio::fs::write(file.path(), b"hello world").await.unwrap();
        assert!(verify_sha256(file.path(), &"0".repeat(64)).await.is_err());
        assert!(!file.path().exists());
    }

    #[tokio::test]
    async fn cancellation_stops_checksum_verification() {
        let file = tempfile::NamedTempFile::new().unwrap();
        tokio::fs::write(file.path(), b"archive").await.unwrap();
        let operations = NodeDownloadOperations::default();
        let active = operations.register("operation-1").unwrap();
        let cancellation = active.cancellation();
        assert!(operations.cancel("operation-1"));

        let error = verify_sha256_cancellable(file.path(), &"0".repeat(64), &cancellation)
            .await
            .unwrap_err();
        assert!(matches!(error, LauncherError::NodeInstallCancelled { .. }));
        assert!(file.path().exists());
    }
}
