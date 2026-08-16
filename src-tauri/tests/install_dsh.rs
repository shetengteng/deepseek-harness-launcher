//! PR-013 集成测试：用 mock npm-cli.js 模拟 npm install 行为，
//! 验证完整 install_dsh 流程（写 package.json + spawn + 校验入口）。
//!
//! 设计：
//! - 用真实 node 执行 mock npm-cli.js（验证 spawn 路径）
//! - mock 脚本读取 `package.json` 的 dependencies，模拟创建 `node_modules/<pkg>/lib/bin.js`
//! - 验证 install_dsh 成功路径 + 各种失败路径

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use deepseek_harness_launcher_lib::dsh::{
    install_dsh, run_npm_install, write_package_json, InstallDshOptions, LogCallback,
};
use tempfile::TempDir;

/// 写一个 mock npm-cli.js：读 cwd/package.json，模拟创建 node_modules/<pkg>/lib/bin.js。
///
/// 用法：`node mock-npm-cli.js install --prod --registry=... ...`
fn write_mock_npm_cli(dir: &std::path::Path) -> PathBuf {
    let script = r#"
// mock npm-cli.js：模拟 npm install 行为
// 读取 cwd/package.json，根据 dependencies 创建 node_modules 结构
const fs = require('fs');
const path = require('path');

// 输出 info 日志（被 install.rs 的 log callback 收集）
console.log('mock-npm: starting install');

try {
    const pkgPath = path.resolve('package.json');
    const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
    console.log('mock-npm: installing ' + Object.keys(pkg.dependencies || {}).join(', '));

    for (const dep of Object.keys(pkg.dependencies || {})) {
        // 解析 scoped 包名：@deepseek-ai/dsh → @deepseek-ai/dsh
        const target = path.join('node_modules', dep);
        fs.mkdirSync(target, { recursive: true });
        // 写 package.json
        fs.writeFileSync(
            path.join(target, 'package.json'),
            JSON.stringify({ name: dep, version: pkg.dependencies[dep], main: 'lib/bin.js' })
        );
        // 写入口 lib/bin.js
        fs.mkdirSync(path.join(target, 'lib'), { recursive: true });
        fs.writeFileSync(
            path.join(target, 'lib', 'bin.js'),
            '// mock dsh entry\nconsole.log("hello from mock dsh");\n'
        );
        console.log('mock-npm: installed ' + dep);
    }

    console.log('mock-npm: install complete');
    process.exit(0);
} catch (err) {
    console.error('mock-npm: FATAL ' + err.message);
    process.exit(1);
}
"#;
    let p = dir.join("mock-npm-cli.js");
    std::fs::write(&p, script).unwrap();
    p
}

/// 构造 InstallDshOptions，用系统 node + mock npm-cli.js。
fn make_opts(dsh_dir: &std::path::Path, version: &str, npm_script: PathBuf) -> InstallDshOptions {
    InstallDshOptions {
        version: version.to_string(),
        registry: "https://registry.npmmirror.com".to_string(),
        dsh_dir: dsh_dir.to_path_buf(),
        node_executable: PathBuf::from("node"),
        npm_script: Some(npm_script),
        log: None,
        timeout_secs: 30,
    }
}

#[tokio::test]
async fn install_dsh_full_pipeline_success() {
    let tmp = TempDir::new().unwrap();
    let dsh_dir = tmp.path().join("dsh");
    let mock_dir = tmp.path().join("mock");
    std::fs::create_dir_all(&mock_dir).unwrap();
    let npm_script = write_mock_npm_cli(&mock_dir);

    let opts = make_opts(&dsh_dir, "0.1.0", npm_script);

    install_dsh(&opts).await.expect("install should succeed");

    // 验证：package.json 写入
    let pkg_path = opts.package_json_path();
    assert!(pkg_path.exists(), "package.json should be created");
    let pkg: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pkg_path).unwrap()).unwrap();
    assert_eq!(pkg["dependencies"]["@deepseek-ai/dsh"], "0.1.0");

    // 验证：cli entry 存在
    let entry = opts.cli_entry_path();
    assert!(
        entry.exists(),
        "cli entry should exist: {}",
        entry.display()
    );
    let entry_content = std::fs::read_to_string(&entry).unwrap();
    assert!(entry_content.contains("mock dsh entry"));
}

#[tokio::test]
async fn install_dsh_writes_correct_package_json() {
    let tmp = TempDir::new().unwrap();
    let mock_dir = tmp.path().join("mock");
    std::fs::create_dir_all(&mock_dir).unwrap();
    let npm_script = write_mock_npm_cli(&mock_dir);

    let opts = make_opts(&tmp.path().join("dsh"), "0.2.0", npm_script);
    write_package_json(&opts).unwrap();

    let pkg = std::fs::read_to_string(opts.package_json_path()).unwrap();
    assert!(pkg.contains(r#""@deepseek-ai/dsh": "0.2.0""#));
    assert!(pkg.contains(r#""private": true"#));
}

#[tokio::test]
async fn install_dsh_with_log_callback_collects_output() {
    let tmp = TempDir::new().unwrap();
    let mock_dir = tmp.path().join("mock");
    std::fs::create_dir_all(&mock_dir).unwrap();
    let npm_script = write_mock_npm_cli(&mock_dir);

    let log_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let log_lines_clone = log_lines.clone();
    let log: LogCallback = Arc::new(move |line: &str| {
        log_lines_clone.lock().unwrap().push(line.to_string());
    });

    let mut opts = make_opts(&tmp.path().join("dsh"), "0.1.0", npm_script);
    opts.log = Some(log);

    install_dsh(&opts).await.expect("install");

    let collected = log_lines.lock().unwrap().clone();
    assert!(collected
        .iter()
        .any(|l| l.contains("mock-npm: starting install")));
    assert!(collected.iter().any(|l| l.contains("install complete")));
}

#[tokio::test]
async fn install_dsh_cleans_dir_on_npm_failure() {
    let tmp = TempDir::new().unwrap();
    // mock npm-cli.js 失败版：直接 exit(1)
    let mock = tmp.path().join("fail-npm.js");
    std::fs::write(&mock, "console.error('boom'); process.exit(1);\n").unwrap();

    let opts = make_opts(&tmp.path().join("dsh"), "0.1.0", mock);
    let err = install_dsh(&opts).await.unwrap_err();
    assert!(err.to_string().contains("install failed"));
    assert!(err.to_string().contains("npm install failed"));

    // 关键：版本目录被清理
    assert!(
        !opts.version_dir().exists(),
        "version dir should be cleaned"
    );
}

#[tokio::test]
async fn install_dsh_cleans_dir_on_verify_failure() {
    let tmp = TempDir::new().unwrap();
    // mock npm-cli.js 半成功：exit(0) 但不创建 node_modules
    let mock = tmp.path().join("noop-npm.js");
    std::fs::write(&mock, "console.log('noop'); process.exit(0);\n").unwrap();

    let opts = make_opts(&tmp.path().join("dsh"), "0.1.0", mock);
    let err = install_dsh(&opts).await.unwrap_err();
    assert!(err.to_string().contains("verify failed"));
    assert!(err.to_string().contains("not found"));
    assert!(!opts.version_dir().exists());
}

#[tokio::test]
async fn install_dsh_cleans_dir_on_partial_install() {
    let tmp = TempDir::new().unwrap();
    // mock npm-cli.js 只创建 package.json 不创建 node_modules
    let mock = tmp.path().join("partial-npm.js");
    std::fs::write(
        &mock,
        r#"
        const fs = require('fs');
        // 只读 package.json 但不创建 node_modules
        const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
        console.log('partial install of ' + Object.keys(pkg.dependencies));
        process.exit(0);
        "#,
    )
    .unwrap();

    let opts = make_opts(&tmp.path().join("dsh"), "0.1.0", mock);
    let err = install_dsh(&opts).await.unwrap_err();
    assert!(err.to_string().contains("verify failed"));
    assert!(!opts.version_dir().exists());
}

#[tokio::test]
async fn install_dsh_creates_node_modules_structure() {
    let tmp = TempDir::new().unwrap();
    let mock_dir = tmp.path().join("mock");
    std::fs::create_dir_all(&mock_dir).unwrap();
    let npm_script = write_mock_npm_cli(&mock_dir);

    let opts = make_opts(&tmp.path().join("dsh"), "1.0.0", npm_script);
    install_dsh(&opts).await.expect("install");

    let dsh_module_dir = opts.dsh_module_dir();
    assert!(dsh_module_dir.exists(), "dsh module dir should exist");
    assert!(dsh_module_dir.join("package.json").exists());
    assert!(dsh_module_dir.join("lib").join("bin.js").exists());
}

#[tokio::test]
async fn install_dsh_with_nonexistent_node_errors() {
    let tmp = TempDir::new().unwrap();
    let mock = tmp.path().join("noop.js");
    std::fs::write(&mock, "process.exit(0);").unwrap();

    let mut opts = make_opts(&tmp.path().join("dsh"), "0.1.0", mock);
    opts.node_executable = PathBuf::from("/nonexistent/node-binary");

    let err = install_dsh(&opts).await.unwrap_err();
    assert!(err.to_string().contains("spawn npm install failed"));
    assert!(!opts.version_dir().exists());
}

#[tokio::test]
async fn install_dsh_timeout_kills_process() {
    let tmp = TempDir::new().unwrap();
    // mock npm 永远不退出
    let mock = tmp.path().join("hang-npm.js");
    std::fs::write(&mock, "console.log('start'); setInterval(() => {}, 1000);").unwrap();

    let mut opts = make_opts(&tmp.path().join("dsh"), "0.1.0", mock);
    opts.timeout_secs = 1;

    let err = install_dsh(&opts).await.unwrap_err();
    assert!(err.to_string().contains("timed out"));
    assert!(!opts.version_dir().exists());
}

#[tokio::test]
async fn run_npm_install_errors_when_package_json_missing() {
    let tmp = TempDir::new().unwrap();
    let mock = tmp.path().join("noop.js");
    std::fs::write(&mock, "process.exit(0);").unwrap();

    let opts = make_opts(&tmp.path().join("dsh"), "0.1.0", mock);
    let err = run_npm_install(&opts).await.unwrap_err();
    assert!(err.to_string().contains("package.json not found"));
}

#[tokio::test]
async fn install_dsh_multiple_versions_coexist() {
    // 安装两个版本，互不影响
    let tmp = TempDir::new().unwrap();
    let mock_dir = tmp.path().join("mock");
    std::fs::create_dir_all(&mock_dir).unwrap();
    let npm_script = write_mock_npm_cli(&mock_dir);

    let dsh_dir = tmp.path().join("dsh");
    let opts_v1 = make_opts(&dsh_dir, "0.1.0", npm_script.clone());
    let opts_v2 = make_opts(&dsh_dir, "0.2.0", npm_script);

    install_dsh(&opts_v1).await.expect("v1 install");
    install_dsh(&opts_v2).await.expect("v2 install");

    assert!(opts_v1.cli_entry_path().exists());
    assert!(opts_v2.cli_entry_path().exists());
    assert_ne!(opts_v1.version_dir(), opts_v2.version_dir());
}

#[tokio::test]
async fn install_dsh_overrides_existing_version() {
    // 同一版本重复安装：覆盖写入
    let tmp = TempDir::new().unwrap();
    let mock_dir = tmp.path().join("mock");
    std::fs::create_dir_all(&mock_dir).unwrap();
    let npm_script = write_mock_npm_cli(&mock_dir);

    let opts = make_opts(&tmp.path().join("dsh"), "0.1.0", npm_script);

    install_dsh(&opts).await.expect("first install");
    let first_mtime = std::fs::metadata(opts.cli_entry_path())
        .unwrap()
        .modified()
        .unwrap();

    // 等 10ms 确保时间戳不同
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    install_dsh(&opts).await.expect("second install");
    let second_mtime = std::fs::metadata(opts.cli_entry_path())
        .unwrap()
        .modified()
        .unwrap();
    assert!(second_mtime >= first_mtime);
}
