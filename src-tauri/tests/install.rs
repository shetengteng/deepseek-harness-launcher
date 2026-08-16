//! PR-009 集成测试：Node 解压 + 原子切换（设计 §M2.3）。
//!
//! 测试策略：
//! 1. 动态构造真实 `node-v22.19.0-test.tar.gz` 和 `node-v22.19.0-test.zip` fixture
//! 2. 调用 `install_node_to(archive, version, kind, runtime_dir=tmpdir, None)`
//! 3. 验证 staging 被清理、target 目录存在、VERSION 文件正确、bin/node 可执行
//! 4. 覆盖：重复安装跳过、解压中断后 staging 清理、端到端 download+install 流程
//!
//! 覆盖测试设计 §PR-008 用例：
//! - 解压成功 + 路径 strip + VERSION 写入
//! - 重复安装跳过
//! - staging 残留清理
//! - 端到端：下载 archive → 安装 → 验证可执行文件

use std::io::Write;

use deepseek_harness_launcher_lib::node::{
    current_node_dir_in, current_node_version_in, install_node_to, node_bin_path, NodeArchiveKind,
};
use tokio::sync::mpsc;

/// 构造真实 tar.gz fixture（外层目录 `node-v22.19.0-test/`）。
fn build_node_tar_gz() -> Vec<u8> {
    let mut tar_buf: Vec<u8> = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_buf);
        let node_bin = b"#!/bin/sh\necho 'fake node v22.19.0'\n";
        let mut header = tar::Header::new_gnu();
        header.set_path("node-v22.19.0-test/bin/node").unwrap();
        header.set_size(node_bin.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append(&header, &node_bin[..]).unwrap();
        let npm_bin = b"#!/bin/sh\necho 'fake npm'\n";
        let mut header = tar::Header::new_gnu();
        header.set_path("node-v22.19.0-test/bin/npm").unwrap();
        header.set_size(npm_bin.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append(&header, &npm_bin[..]).unwrap();
        let pkg = b"{\"name\":\"node\",\"version\":\"22.19.0\"}\n";
        let mut header = tar::Header::new_gnu();
        header.set_path("node-v22.19.0-test/package.json").unwrap();
        header.set_size(pkg.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, &pkg[..]).unwrap();
        builder.finish().unwrap();
    }
    let mut gz_buf: Vec<u8> = Vec::new();
    {
        let mut encoder =
            flate2::write::GzEncoder::new(&mut gz_buf, flate2::Compression::default());
        encoder.write_all(&tar_buf).unwrap();
        encoder.finish().unwrap();
    }
    gz_buf
}

/// 构造真实 zip fixture。
fn build_node_zip() -> Vec<u8> {
    use std::io::Cursor;
    let mut buf: Cursor<Vec<u8>> = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default();
        zip.add_directory("node-v22.19.0-test/", opts).unwrap();
        zip.add_directory("node-v22.19.0-test/bin/", opts).unwrap();
        zip.start_file("node-v22.19.0-test/bin/node", opts.unix_permissions(0o755))
            .unwrap();
        zip.write_all(b"fake node\n").unwrap();
        zip.start_file("node-v22.19.0-test/package.json", opts)
            .unwrap();
        zip.write_all(b"{\"name\":\"node\"}\n").unwrap();
        zip.finish().unwrap();
    }
    buf.into_inner()
}

#[tokio::test]
async fn install_node_to_creates_target_and_version_file() {
    let archive_bytes = build_node_tar_gz();
    let tmp = tempfile::tempdir().unwrap();
    let archive_path = tmp.path().join("node.tar.gz");
    std::fs::write(&archive_path, &archive_bytes).unwrap();

    let runtime_dir = tmp.path().join("node-runtime");
    let target = install_node_to(
        &archive_path,
        "22.19.0",
        NodeArchiveKind::TarGz,
        &runtime_dir,
        None,
    )
    .await
    .expect("install should succeed");

    // target 目录存在
    assert!(target.exists());
    assert_eq!(
        target.file_name().unwrap(),
        std::ffi::OsStr::new("node-v22.19.0")
    );
    // bin/node 和 package.json 存在（root 已 strip）
    assert!(target.join("bin").join("node").exists());
    assert!(target.join("bin").join("npm").exists());
    assert!(target.join("package.json").exists());
    // staging 已被 rename 走
    let staging = runtime_dir.join(".staging-v22.19.0");
    assert!(!staging.exists());
    // VERSION 文件写入
    let version = current_node_version_in(&runtime_dir).expect("VERSION file");
    assert_eq!(version, "22.19.0");
    // current_node_dir_in 解析正确
    let cur = current_node_dir_in(&runtime_dir).expect("current_node_dir");
    assert_eq!(cur, target);
    // bin/node 路径
    let bin = node_bin_path(&cur);
    assert!(bin.exists());
}

#[tokio::test]
async fn install_node_to_skips_when_target_exists() {
    let archive_bytes = build_node_tar_gz();
    let tmp = tempfile::tempdir().unwrap();
    let archive_path = tmp.path().join("node.tar.gz");
    std::fs::write(&archive_path, &archive_bytes).unwrap();

    let runtime_dir = tmp.path().join("node-runtime");
    // 预先创建 target 目录，模拟已安装
    let target = runtime_dir.join("node-v22.19.0");
    std::fs::create_dir_all(&target).unwrap();
    // 放一个 sentinel 文件，验证 install_node_to 没有覆盖
    let sentinel = target.join("SENTINEL");
    std::fs::write(&sentinel, b"pre-existing").unwrap();

    let result = install_node_to(
        &archive_path,
        "22.19.0",
        NodeArchiveKind::TarGz,
        &runtime_dir,
        None,
    )
    .await
    .expect("should succeed (skip)");

    assert_eq!(result, target);
    // sentinel 仍然存在（未被覆盖）
    assert!(sentinel.exists());
    // bin/node 不应该存在（因为没解压）
    assert!(!target.join("bin").join("node").exists());
    // VERSION 文件不会被写（skip 路径）
    // 但 current_node_version_in 会失败（VERSION 不存在）
    let version_result = current_node_version_in(&runtime_dir);
    assert!(version_result.is_err());
}

#[tokio::test]
async fn install_node_to_cleans_stale_staging() {
    let archive_bytes = build_node_tar_gz();
    let tmp = tempfile::tempdir().unwrap();
    let archive_path = tmp.path().join("node.tar.gz");
    std::fs::write(&archive_path, &archive_bytes).unwrap();

    let runtime_dir = tmp.path().join("node-runtime");
    // 预先创建 stale staging，模拟上次崩溃残留
    let stale = runtime_dir.join(".staging-v22.19.0");
    std::fs::create_dir_all(&stale).unwrap();
    std::fs::write(stale.join("garbage"), b"stale").unwrap();

    let target = install_node_to(
        &archive_path,
        "22.19.0",
        NodeArchiveKind::TarGz,
        &runtime_dir,
        None,
    )
    .await
    .expect("install should succeed");

    assert!(target.exists());
    assert!(target.join("bin").join("node").exists());
    // staging 已被清理（rename 走）
    assert!(!stale.exists());
    // VERSION 写入
    assert_eq!(current_node_version_in(&runtime_dir).unwrap(), "22.19.0");
}

#[tokio::test]
async fn install_node_to_emits_progress_events() {
    let archive_bytes = build_node_tar_gz();
    let tmp = tempfile::tempdir().unwrap();
    let archive_path = tmp.path().join("node.tar.gz");
    std::fs::write(&archive_path, &archive_bytes).unwrap();

    let runtime_dir = tmp.path().join("node-runtime");
    let (tx, mut rx) = mpsc::channel::<deepseek_harness_launcher_lib::node::ProgressEvent>(16);

    let target = install_node_to(
        &archive_path,
        "22.19.0",
        NodeArchiveKind::TarGz,
        &runtime_dir,
        Some(&tx),
    )
    .await
    .expect("install should succeed");

    assert!(target.exists());

    // 应至少收到 2 个 progress event：extract start + extract complete
    let mut stages = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        stages.push(ev.stage);
    }
    assert!(
        stages.iter().any(|s| s == "extract"),
        "expected extract events, got {stages:?}"
    );
    assert!(
        stages.len() >= 2,
        "expected >=2 events, got {}",
        stages.len()
    );
}

#[tokio::test]
async fn install_node_to_zip_extracts_correctly() {
    let archive_bytes = build_node_zip();
    let tmp = tempfile::tempdir().unwrap();
    let archive_path = tmp.path().join("node.zip");
    std::fs::write(&archive_path, &archive_bytes).unwrap();

    let runtime_dir = tmp.path().join("node-runtime");
    let target = install_node_to(
        &archive_path,
        "22.19.0",
        NodeArchiveKind::Zip,
        &runtime_dir,
        None,
    )
    .await
    .expect("install zip should succeed");

    assert!(target.exists());
    assert!(target.join("bin").join("node").exists());
    assert!(target.join("package.json").exists());
    assert_eq!(current_node_version_in(&runtime_dir).unwrap(), "22.19.0");
}

#[tokio::test]
async fn install_node_to_files_stay_within_target() {
    // 验证正常 archive 解压后所有文件都在 target 目录内（不越界到 runtime_dir 之外）
    // 这相当于 zip-slip 防护的回归测试：extract_tar_gz_blocking 用 `target.starts_with(dest_dir)` 检查
    let archive_bytes = build_node_tar_gz();
    let tmp = tempfile::tempdir().unwrap();
    let archive_path = tmp.path().join("node.tar.gz");
    std::fs::write(&archive_path, &archive_bytes).unwrap();

    let runtime_dir = tmp.path().join("node-runtime");
    let target = install_node_to(
        &archive_path,
        "22.19.0",
        NodeArchiveKind::TarGz,
        &runtime_dir,
        None,
    )
    .await
    .expect("normal install should succeed");

    // 验证所有文件都在 target 目录内
    let bin = target.join("bin").join("node");
    assert!(bin.exists());
    assert!(bin.starts_with(&target));
    // runtime_dir 之外不应有 node 相关文件
    assert!(!tmp.path().join("bin").exists());
    assert!(!tmp.path().join("package.json").exists());
}

#[tokio::test]
async fn end_to_end_download_then_install() {
    // 端到端：构造真实 archive → 模拟 wiremock 下载 → install_node_to → 验证可执行
    use deepseek_harness_launcher_lib::node::{download_with_retry, Mirror, MirrorId};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let archive_bytes = build_node_tar_gz();
    // 计算 SHA-256
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&archive_bytes);
    let sha = hex::encode(hasher.finalize());

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/v22.19.0/node-v22.19.0-test-darwin-arm64-arm64.tar.gz",
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(archive_bytes.clone())
                .insert_header("content-type", "application/gzip"),
        )
        .mount(&server)
        .await;
    let shasums = format!("{sha}  node-v22.19.0-test-darwin-arm64-arm64.tar.gz\n");
    Mock::given(method("GET"))
        .and(path("/v22.19.0/SHASUMS256.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string(shasums))
        .mount(&server)
        .await;

    let base_url = server.uri().trim_end_matches('/').to_string();
    let mirror = Mirror {
        id: MirrorId::Custom(base_url.clone()),
        name: "test",
        base_url: Box::leak(base_url.into_boxed_str()),
        trusted: false,
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();
    let tmp = tempfile::tempdir().unwrap();

    // 下载
    let archive_path = download_with_retry(
        &client,
        &mirror,
        "22.19.0",
        "node-v22.19.0-test-darwin-arm64-arm64.tar.gz",
        tmp.path(),
        None,
        2,
    )
    .await
    .expect("download should succeed");
    assert!(archive_path.exists());

    // 安装
    let runtime_dir = tmp.path().join("node-runtime");
    let target = install_node_to(
        &archive_path,
        "22.19.0",
        NodeArchiveKind::TarGz,
        &runtime_dir,
        None,
    )
    .await
    .expect("install should succeed");

    // 验证可执行文件路径
    let bin = node_bin_path(&target);
    assert!(bin.exists(), "bin/node should exist at {}", bin.display());

    // 验证 current_node_dir_in 一致
    let cur = current_node_dir_in(&runtime_dir).unwrap();
    assert_eq!(cur, target);
}
