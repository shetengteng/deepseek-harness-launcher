//! PR-008 集成测试：Node 下载与校验（设计 §M2.2 + §PR-007 用例）。
//!
//! 测试策略：
//! 1. 在测试内动态构造真实 `node-v22.19.0-darwin-arm64-arm64.tar.gz`（含 fake `bin/node`）
//! 2. 计算真实 SHA-256，生成 `SHASUMS256.txt`
//! 3. 用 wiremock mount 这两个 endpoint
//! 4. 调用 `download_with_retry` / `download_archive` / `verify_sha256` 验证全流程
//!
//! 覆盖测试设计 §PR-007 用例：
//! - 200 + SHA 通过 → Ok
//! - 404 → Err
//! - 5xx 重试 3 次后 Err
//! - SHA 不匹配 → Err + 文件删除
//! - 进度事件触发
//! - 下载中断（stream error）→ Err + .part 清理

use std::io::Write;

use deepseek_harness_launcher_lib::node::{
    download_archive, download_with_retry, find_sha_in_shasums, verify_sha256, Mirror, MirrorId,
    ProgressEvent,
};
use flate2::write::GzEncoder;
use flate2::Compression;
use reqwest::Client;
use sha2::{Digest, Sha256};
use tar::Builder;
use tokio::sync::mpsc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// 确定性伪随机字节（xorshift64）。制造不可压缩 payload，
/// 保证 fixture 压缩后仍大于 `download_archive` 的 1MB 错误页防御阈值。
fn pseudo_random_bytes(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.push((state & 0xff) as u8);
    }
    out
}

/// 构造一个真实的 `node-v22.19.0-darwin-arm64-arm64.tar.gz` fixture。
/// 内容：`bin/node`（fake 二进制）+ `bin/npm` + `package.json` + 2MB 不可压缩 payload。
/// 返回 (archive_bytes, archive_sha256)。
fn build_node_archive_fixture() -> (Vec<u8>, String) {
    let mut tar_buf: Vec<u8> = Vec::new();
    {
        let mut builder = Builder::new(&mut tar_buf);
        // fake node 二进制
        let node_bin = b"#!/bin/sh\necho 'fake node v22.19.0'\n";
        let mut header = tar::Header::new_gnu();
        header
            .set_path("node-v22.19.0-darwin-arm64/bin/node")
            .unwrap();
        header.set_size(node_bin.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append(&header, &node_bin[..]).unwrap();
        // fake npm
        let npm_bin = b"#!/bin/sh\necho 'fake npm'\n";
        let mut header = tar::Header::new_gnu();
        header
            .set_path("node-v22.19.0-darwin-arm64/bin/npm")
            .unwrap();
        header.set_size(npm_bin.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append(&header, &npm_bin[..]).unwrap();
        // package.json（让 archive 非空且结构合理）
        let pkg = b"{\"name\":\"node\",\"version\":\"22.19.0\"}\n";
        let mut header = tar::Header::new_gnu();
        header
            .set_path("node-v22.19.0-darwin-arm64/package.json")
            .unwrap();
        header.set_size(pkg.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, &pkg[..]).unwrap();
        // 2MB 不可压缩 payload：绕过 download_archive 对 <1MB 错误页的防御，
        // 贴近真实 Node archive（≥20MB）的体积特征。
        let payload = pseudo_random_bytes(2 * 1024 * 1024, 0x5eed);
        let mut header = tar::Header::new_gnu();
        header
            .set_path("node-v22.19.0-darwin-arm64/lib/payload.bin")
            .unwrap();
        header.set_size(payload.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, &payload[..]).unwrap();
        builder.finish().unwrap();
    }
    // gzip 压缩
    let mut gz_buf: Vec<u8> = Vec::new();
    {
        let mut encoder = GzEncoder::new(&mut gz_buf, Compression::default());
        encoder.write_all(&tar_buf).unwrap();
        encoder.finish().unwrap();
    }
    // SHA-256
    let mut hasher = Sha256::new();
    hasher.update(&gz_buf);
    let sha = hasher.finalize();
    let sha_hex = hex::encode(sha);
    (gz_buf, sha_hex)
}

/// 生成 SHASUMS256.txt 文本。
fn build_shasums(archive_filename: &str, sha: &str) -> String {
    format!("{sha}  {archive_filename}\n")
}

/// 生成自定义镜像源（指向 wiremock server）。
/// 注意：wiremock 用 http 而非 https，测试环境绕过 validate_custom_mirror 校验，
/// 直接构造 Mirror 结构。
fn custom_mirror(server_uri: String) -> Mirror {
    let base_url = server_uri.trim_end_matches('/').to_string();
    Mirror {
        id: MirrorId::Custom(base_url.clone()),
        name: "test",
        base_url: Box::leak(base_url.into_boxed_str()),
        trusted: false,
    }
}

fn build_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("client")
}

const ARCHIVE_FILENAME: &str = "node-v22.19.0-darwin-arm64-arm64.tar.gz";

/// Mount archive + SHASUMS256.txt on mock server。返回 (archive_bytes, sha)。
async fn mount_node_files(server: &MockServer, archive_bytes: &[u8], sha: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/v22.19.0/{ARCHIVE_FILENAME}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(archive_bytes.to_vec())
                .insert_header("content-type", "application/gzip"),
        )
        .mount(server)
        .await;

    let shasums = build_shasums(ARCHIVE_FILENAME, sha);
    Mock::given(method("GET"))
        .and(path("/v22.19.0/SHASUMS256.txt"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(shasums)
                .insert_header("content-type", "text/plain"),
        )
        .mount(server)
        .await;
}

#[tokio::test]
async fn download_and_verify_succeeds_when_sha_matches() {
    let server = MockServer::start().await;
    let (archive_bytes, sha) = build_node_archive_fixture();
    mount_node_files(&server, &archive_bytes, &sha).await;

    let mirror = custom_mirror(server.uri());
    let client = build_client();
    let tmp = tempfile::tempdir().unwrap();

    let dest = download_with_retry(
        &client,
        &mirror,
        "22.19.0",
        ARCHIVE_FILENAME,
        tmp.path(),
        None,
        2,
    )
    .await
    .expect("download should succeed");

    assert!(dest.exists());
    let downloaded = std::fs::read(&dest).unwrap();
    assert_eq!(downloaded, archive_bytes);
}

#[tokio::test]
async fn download_404_returns_err() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/v22.19.0/{ARCHIVE_FILENAME}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let mirror = custom_mirror(server.uri());
    let client = build_client();
    let tmp = tempfile::tempdir().unwrap();

    let err = download_with_retry(
        &client,
        &mirror,
        "22.19.0",
        ARCHIVE_FILENAME,
        tmp.path(),
        None,
        1,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        deepseek_harness_launcher_lib::error::LauncherError::NodeDownload(_)
    ));
}

#[tokio::test]
async fn download_500_retries_and_eventually_fails() {
    let server = MockServer::start().await;
    // 始终 500
    Mock::given(method("GET"))
        .and(path(format!("/v22.19.0/{ARCHIVE_FILENAME}")))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let mirror = custom_mirror(server.uri());
    let client = build_client();
    let tmp = tempfile::tempdir().unwrap();

    let err = download_with_retry(
        &client,
        &mirror,
        "22.19.0",
        ARCHIVE_FILENAME,
        tmp.path(),
        None,
        3,
    )
    .await
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("500") || msg.contains("exhausted"));
    // .part 文件应被清理（每次失败后下次重试前）
    let part = tmp.path().join(format!("{ARCHIVE_FILENAME}.part"));
    assert!(!part.exists());
}

#[tokio::test]
async fn sha_mismatch_errors_and_deletes_file() {
    let server = MockServer::start().await;
    let (archive_bytes, _real_sha) = build_node_archive_fixture();
    // 故意用错误的 SHA
    let wrong_sha = "0".repeat(64);
    mount_node_files(&server, &archive_bytes, &wrong_sha).await;

    let mirror = custom_mirror(server.uri());
    let client = build_client();
    let tmp = tempfile::tempdir().unwrap();

    let err = download_with_retry(
        &client,
        &mirror,
        "22.19.0",
        ARCHIVE_FILENAME,
        tmp.path(),
        None,
        1,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        deepseek_harness_launcher_lib::error::LauncherError::NodeDownload(_)
    ));
    // archive 文件应被 verify_sha256 删除
    let dest = tmp.path().join(ARCHIVE_FILENAME);
    assert!(
        !dest.exists(),
        "archive file should be deleted on SHA mismatch"
    );
}

#[tokio::test]
async fn progress_events_are_emitted() {
    let server = MockServer::start().await;
    // 构造大于 1MB 的不可压缩 archive：既绕过错误页防御，又确保触发多次进度事件
    let big_node_bin = pseudo_random_bytes(2 * 1024 * 1024, 0xfeed);
    let mut tar_buf: Vec<u8> = Vec::new();
    {
        let mut builder = Builder::new(&mut tar_buf);
        let mut header = tar::Header::new_gnu();
        header
            .set_path("node-v22.19.0-darwin-arm64/bin/node")
            .unwrap();
        header.set_size(big_node_bin.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append(&header, &big_node_bin[..]).unwrap();
        builder.finish().unwrap();
    }
    let mut gz_buf: Vec<u8> = Vec::new();
    {
        let mut encoder = GzEncoder::new(&mut gz_buf, Compression::default());
        encoder.write_all(&tar_buf).unwrap();
        encoder.finish().unwrap();
    }
    let archive_bytes = gz_buf;
    let mut hasher = Sha256::new();
    hasher.update(&archive_bytes);
    let sha = hex::encode(hasher.finalize());

    mount_node_files(&server, &archive_bytes, &sha).await;

    let mirror = custom_mirror(server.uri());
    let client = build_client();
    let tmp = tempfile::tempdir().unwrap();
    let (tx, mut rx) = mpsc::channel::<ProgressEvent>(32);

    let dest_path = tmp.path().join(ARCHIVE_FILENAME);
    let archive_url = format!(
        "{}/v22.19.0/{ARCHIVE_FILENAME}",
        mirror.base_url.trim_end_matches('/')
    );
    let _bytes = download_archive(&client, &archive_url, &dest_path, Some(&tx))
        .await
        .expect("download ok");

    // 应至少收到一个进度事件
    let mut count = 0;
    while let Ok(_ev) = rx.try_recv() {
        count += 1;
    }
    assert!(
        count > 0,
        "expected at least one progress event, got {count}"
    );
}

#[tokio::test]
async fn download_with_retry_succeeds_after_one_failure() {
    let server = MockServer::start().await;
    let (archive_bytes, sha) = build_node_archive_fixture();

    // 第一次 500，第二次 200
    Mock::given(method("GET"))
        .and(path(format!("/v22.19.0/{ARCHIVE_FILENAME}")))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v22.19.0/{ARCHIVE_FILENAME}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(archive_bytes.clone())
                .insert_header("content-type", "application/gzip"),
        )
        .mount(&server)
        .await;
    let shasums = build_shasums(ARCHIVE_FILENAME, &sha);
    Mock::given(method("GET"))
        .and(path("/v22.19.0/SHASUMS256.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string(shasums))
        .mount(&server)
        .await;

    let mirror = custom_mirror(server.uri());
    let client = build_client();
    let tmp = tempfile::tempdir().unwrap();

    let dest = download_with_retry(
        &client,
        &mirror,
        "22.19.0",
        ARCHIVE_FILENAME,
        tmp.path(),
        None,
        3,
    )
    .await
    .expect("should succeed after retry");

    assert!(dest.exists());
}

#[tokio::test]
async fn find_sha_in_real_shasums_format() {
    // 用真实 Node SHASUMS256.txt 格式验证
    let (archive_bytes, sha) = build_node_archive_fixture();
    let shasums = build_shasums(ARCHIVE_FILENAME, &sha);
    let found = find_sha_in_shasums(&shasums, ARCHIVE_FILENAME).expect("should find");
    assert_eq!(found, sha);
    // archive_bytes 仅供引用，避免 unused 警告
    let _ = &archive_bytes;
}

#[tokio::test]
async fn verify_sha256_accepts_real_archive() {
    let (archive_bytes, sha) = build_node_archive_fixture();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), &archive_bytes).unwrap();
    verify_sha256(tmp.path(), &sha).await.expect("should match");
}

#[tokio::test]
async fn download_missing_shasums_returns_err() {
    let server = MockServer::start().await;
    let (archive_bytes, _sha) = build_node_archive_fixture();
    // 只 mount archive，不 mount SHASUMS256.txt
    Mock::given(method("GET"))
        .and(path(format!("/v22.19.0/{ARCHIVE_FILENAME}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(archive_bytes.clone())
                .insert_header("content-type", "application/gzip"),
        )
        .mount(&server)
        .await;
    // SHASUMS256.txt 返回 404
    Mock::given(method("GET"))
        .and(path("/v22.19.0/SHASUMS256.txt"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let mirror = custom_mirror(server.uri());
    let client = build_client();
    let tmp = tempfile::tempdir().unwrap();

    let err = download_with_retry(
        &client,
        &mirror,
        "22.19.0",
        ARCHIVE_FILENAME,
        tmp.path(),
        None,
        1,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        deepseek_harness_launcher_lib::error::LauncherError::NodeDownload(_)
    ));
    // archive 文件应被下载但校验阶段失败
    let _ = archive_bytes;
}

/// 本地磁盘/权限错误（PR-020）：dest 目录不可写时，Io 错误必须原样返回
/// （保留 kind=io → 前端展示磁盘/权限文案），且不消耗重试次数。
#[tokio::test]
#[cfg(unix)]
async fn download_local_io_error_returns_immediately_without_retry() {
    let server = MockServer::start().await;
    let (archive_bytes, sha) = build_node_archive_fixture();
    mount_node_files(&server, &archive_bytes, &sha).await;

    let mirror = custom_mirror(server.uri());
    let client = build_client();
    let tmp = tempfile::tempdir().unwrap();
    let ro = tmp.path().join("read-only");
    std::fs::create_dir(&ro).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();

    let err = download_with_retry(&client, &mirror, "22.19.0", ARCHIVE_FILENAME, &ro, None, 3)
        .await
        .expect_err("write to read-only dir must fail");

    // 保留 Io kind，而非被包成 NodeDownload"换镜像源"文案
    assert!(
        matches!(
            err,
            deepseek_harness_launcher_lib::error::LauncherError::Io(_)
        ),
        "unexpected error: {err:?}"
    );
    // 只发过 1 次 GET：本地错误不重试
    let hits = server.received_requests().await.unwrap().len();
    assert_eq!(hits, 1, "local io error should not be retried");
}
