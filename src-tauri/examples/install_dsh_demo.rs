//! PR-013 本地演示：起一个 mock npm-cli.js，可视化跑完整 install_dsh 流程。
//!
//! 运行方式：
//! ```bash
//! cd src-tauri
//! cargo run --example install_dsh_demo
//! ```
//!
//! 不会真的联网。用 mock 脚本模拟 npm install 行为。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use deepseek_harness_launcher_lib::dsh::{install_dsh, verify_install, InstallDshOptions, LogCallback};

#[tokio::main]
async fn main() {
    println!("=== PR-013 dsh/install 本地演示 ===\n");

    // 1. 准备临时目录
    let tmp = tempfile::tempdir().expect("tempdir");
    let dsh_dir = tmp.path().join("dsh");
    let mock_dir = tmp.path().join("mock");
    std::fs::create_dir_all(&mock_dir).unwrap();
    println!("[setup] 临时目录: {}", tmp.path().display());
    println!("[setup] dsh 安装目录: {}", dsh_dir.display());
    println!();

    // 2. 写 mock npm-cli.js
    let npm_script = write_mock_npm_cli(&mock_dir);
    println!("[setup] mock npm-cli.js: {}", npm_script.display());
    println!();

    // 3. 构造日志回调，收集 npm 输出
    let log_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let log_lines_clone = log_lines.clone();
    let log: LogCallback = Arc::new(move |line: &str| {
        println!("    [npm] {line}");
        log_lines_clone.lock().unwrap().push(line.to_string());
    });

    let opts = InstallDshOptions {
        version: "0.1.0".to_string(),
        registry: "https://registry.npmmirror.com".to_string(),
        dsh_dir: dsh_dir.clone(),
        node_executable: PathBuf::from("node"),
        npm_script: Some(npm_script),
        log: Some(log),
        timeout_secs: 30,
    };

    // 4. 跑完整 install_dsh
    println!(">>> 步骤 1: 调用 install_dsh(\"0.1.0\")");
    let t0 = Instant::now();
    install_dsh(&opts).await.expect("install");
    let dt = t0.elapsed();
    println!();
    println!("    耗时 = {:.2?}", dt);
    println!();

    // 5. 验证文件结构
    println!(">>> 步骤 2: 验证文件结构");
    let pkg_path = opts.package_json_path();
    let entry_path = opts.cli_entry_path();
    let dsh_module_dir = opts.dsh_module_dir();
    println!(
        "    package.json: {} ({})",
        pkg_path.display(),
        exists_label(&pkg_path)
    );
    println!(
        "    dsh module:   {} ({})",
        dsh_module_dir.display(),
        exists_label(&dsh_module_dir)
    );
    println!(
        "    cli entry:    {} ({})",
        entry_path.display(),
        exists_label(&entry_path)
    );
    println!();

    // 6. 读取 package.json 内容
    println!(">>> 步骤 3: 检查 package.json 内容");
    let pkg_content = std::fs::read_to_string(&pkg_path).unwrap();
    println!("{}", pretty_print_json(&pkg_content));
    println!();

    // 7. 读取 cli_entry 内容
    println!(">>> 步骤 4: 检查 cli_entry 内容");
    let entry_content = std::fs::read_to_string(&entry_path).unwrap();
    println!("{entry_content}");
    println!();

    // 8. 调用 verify_install 再次校验
    println!(">>> 步骤 5: 调用 verify_install()（应通过）");
    verify_install(&opts).expect("verify should pass");
    println!("    ✅ 校验通过");
    println!();

    // 9. 演示失败场景：npm install 失败时清理
    println!(">>> 步骤 6: 演示 npm install 失败场景（清理版本目录）");
    let fail_mock = mock_dir.join("fail-npm.js");
    std::fs::write(&fail_mock, "console.error('boom'); process.exit(1);\n").unwrap();
    let mut fail_opts = opts.clone();
    fail_opts.npm_script = Some(fail_mock.clone());
    fail_opts.version = "9.9.9-fail".to_string();
    let fail_version_dir = fail_opts.version_dir();
    let err = install_dsh(&fail_opts).await.unwrap_err();
    println!("    错误: {err}");
    println!(
        "    失败版本目录被清理: {} ({})",
        fail_version_dir.display(),
        !fail_version_dir.exists()
    );
    println!();

    // 10. 演示超时场景
    println!(">>> 步骤 7: 演示超时场景（1s 超时）");
    let hang_mock = mock_dir.join("hang-npm.js");
    std::fs::write(&hang_mock, "setInterval(() => {}, 1000);\n").unwrap();
    let mut hang_opts = opts.clone();
    hang_opts.npm_script = Some(hang_mock);
    hang_opts.version = "9.9.9-hang".to_string();
    hang_opts.timeout_secs = 1;
    let t1 = Instant::now();
    let err = install_dsh(&hang_opts).await.unwrap_err();
    let dt = t1.elapsed();
    println!("    错误: {err}");
    println!("    耗时 = {:.2?}（应在 1s 附近）", dt);
    assert!(err.to_string().contains("timed out"));
    println!();

    println!("=== 演示完成 ===");
    println!();
    println!("验证点：");
    println!("  1. 步骤 1 完整流程成功：写 package.json → spawn mock npm → 校验入口");
    println!("  2. 步骤 2 文件结构正确：node_modules/@deepseek-ai/dsh/lib/bin.js 存在");
    println!("  3. 步骤 6 失败场景清理版本目录");
    println!(
        "  4. 步骤 7 超时场景在 {}s 附近杀进程",
        hang_opts.timeout_secs
    );
}

fn exists_label(p: &std::path::Path) -> &'static str {
    if p.exists() {
        "✓"
    } else {
        "✗"
    }
}

fn pretty_print_json(s: &str) -> String {
    // 已经是 pretty JSON，原样返回
    s.to_string()
}

/// 写一个 mock npm-cli.js：读 cwd/package.json，模拟创建 node_modules 结构。
fn write_mock_npm_cli(dir: &std::path::Path) -> PathBuf {
    let script = r#"
const fs = require('fs');
const path = require('path');
console.log('mock-npm: starting install');
try {
    const pkg = JSON.parse(fs.readFileSync(path.resolve('package.json'), 'utf8'));
    console.log('mock-npm: installing ' + Object.keys(pkg.dependencies || {}).join(', '));
    for (const dep of Object.keys(pkg.dependencies || {})) {
        const target = path.join('node_modules', dep);
        fs.mkdirSync(target, { recursive: true });
        fs.writeFileSync(
            path.join(target, 'package.json'),
            JSON.stringify({ name: dep, version: pkg.dependencies[dep], main: 'lib/bin.js' })
        );
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
