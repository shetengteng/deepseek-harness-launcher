//! Node 安装流程本地演示：下载 + 校验 + 解压 + 签名。
//!
//! 运行方式：
//! ```bash
//! cd src-tauri
//! cargo run --example install_node_demo
//! ```
//!
//! 会真实联网下载 Node archive，测试完整流程。

use std::time::Instant;

use deepseek_harness_launcher_lib::node::{
    download_with_retry, install_node_to, node_archive_filename, DownloadRequest, NodeArchiveKind,
    NodeDownloadOperations, BUILTIN_MIRRORS,
};

#[tokio::main]
async fn main() {
    println!("=== Node 安装流程本地演示 ===\n");

    let version = "22.19.0";
    // 前端 detectPlatformArch() 返回 { platform: "darwin", arch: "arm64" }
    let platform = "darwin";
    let arch = if std::env::consts::ARCH == "aarch64" {
        "arm64"
    } else {
        "x64"
    };
    let archive_filename = node_archive_filename(version, platform, arch);
    println!("[setup] version={version}, platform={platform}, arch={arch}");
    println!("[setup] archive_filename={archive_filename}");
    println!();

    // 1. 准备临时目录
    let tmp = tempfile::tempdir().expect("tempdir");
    let runtime_dir = tmp.path().join("node-runtime");
    let download_dir = runtime_dir.join(".downloads");
    std::fs::create_dir_all(&download_dir).unwrap();
    println!("[setup] runtime_dir={}", runtime_dir.display());
    println!("[setup] download_dir={}", download_dir.display());
    println!();

    // 2. 构造 reqwest client
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .expect("client");

    // 3. 用 npmmirror 源
    let mirror = &BUILTIN_MIRRORS[0];
    println!("[mirror] id={}, base_url={}", mirror.id, mirror.base_url);
    println!(
        "[mirror] archive_url={}",
        mirror.archive_url(version, platform, arch)
    );
    println!("[mirror] shasums_url={}", mirror.shasums_url(version));
    println!();

    // 4. 下载 + 校验
    let operations = NodeDownloadOperations::default();
    let operation_id = "install-node-demo";
    println!(">>> 步骤 1: download_with_retry");
    let t0 = Instant::now();
    let archive_path = download_with_retry(
        DownloadRequest {
            client: &client,
            mirror,
            version,
            archive_filename: &archive_filename,
            dest_dir: &download_dir,
            progress_tx: None,
            max_retries: 2,
        },
        &operations,
        operation_id,
    )
    .await
    .expect("download failed");
    let dt = t0.elapsed();
    println!("[ok] 下载完成: {} ({:.2?})", archive_path.display(), dt);
    println!();

    // 5. 解压 + 签名
    println!(">>> 步骤 2: install_node_to (extract + re-sign)");
    let t1 = Instant::now();
    let target_dir = install_node_to(
        &archive_path,
        version,
        NodeArchiveKind::for_platform(platform),
        &runtime_dir,
        None,
    )
    .await
    .expect("install_node_to failed");
    let dt = t1.elapsed();
    println!("[ok] 解压完成: {} ({:.2?})", target_dir.display(), dt);
    println!();

    // 6. 验证 node 二进制签名
    println!(">>> 步骤 3: 验证 node 二进制签名");
    let node_bin = target_dir.join("bin").join("node");
    println!("[check] node_bin={}", node_bin.display());
    if node_bin.exists() {
        println!("[check] node binary exists");
        // 检查签名
        let output = std::process::Command::new("codesign")
            .arg("-dvvv")
            .arg(&node_bin)
            .output()
            .expect("codesign -dvvv failed");
        let stderr = String::from_utf8_lossy(&output.stderr);
        if let Some(line) = stderr.lines().find(|l| l.contains("flags")) {
            println!("[check] signature: {}", line);
        }
        if stderr.contains("linker-signed") {
            println!("[WARN] node binary is linker-signed! spawn may fail.");
        } else {
            println!("[ok] node binary is NOT linker-signed");
        }

        // 尝试 spawn node
        println!();
        println!(">>> 步骤 4: 尝试 spawn node");
        match std::process::Command::new(&node_bin)
            .arg("--version")
            .output()
        {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                println!("[ok] spawn node SUCCESS: --version={}", stdout.trim());
            }
            Err(e) => {
                println!("[FAIL] spawn node FAILED: {} (kind={:?})", e, e.kind());
            }
        }
    } else {
        println!("[FAIL] node binary NOT found at {}", node_bin.display());
    }
    println!();
    println!("=== 演示完成 ===");
}
