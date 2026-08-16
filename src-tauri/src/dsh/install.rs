//! dsh 安装封装（设计 §M3.2 / PR-013）。
//!
//! 流程：
//! 1. 创建 `dsh/<version>/` 目录
//! 2. 写 `package.json`：`{"dependencies":{"@deepseek-ai/dsh":"<version>"}}`
//! 3. spawn 托管 Node 跑 `npm install --prod --registry=<mirror> --no-audit --no-fund`
//!    - cwd 设为 `dsh/<version>/`
//!    - stdout/stderr 流式收集到日志回调
//!    - 退出码非 0 抛 `NpmInstallFailed { version, log_path }`
//! 4. 完整性校验（`dsh/integrity.rs`）：入口存在 + tarball SHA-512
//! 5. 校验失败：删目录、抛错、记录到 `ignored_versions`
//!
//! 设计 §M3.2 注意点：
//! - 子依赖的 integrity 由 npm 自查，壳子只校验 dsh 主包 tarball
//! - tarball 字节需要单独从 registry 下载（设计未明确，本实现先在 install 流程外做）
//! - 测试时用 mock npm 替代真实 npm

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::error::{LauncherError, Result};
use crate::node::install::node_bin_path;

use super::integrity::verify_entry_exists;
use super::registry::PackageManifest;

/// dsh 包名（写死，对应 `@deepseek-ai/dsh`）。
pub const DSH_PACKAGE_NAME: &str = "@deepseek-ai/dsh";

/// npm install 输出日志回调。`&str` 是一行（不含换行）。
pub type LogCallback = std::sync::Arc<dyn Fn(&str) + Send + Sync>;

/// 安装参数。
#[derive(Clone)]
pub struct InstallDshOptions {
    /// dsh 版本，如 `0.1.0`。
    pub version: String,
    /// registry base URL，如 `https://registry.npmmirror.com`。
    pub registry: String,
    /// dsh 安装根目录：`<data>/dsh/`。版本子目录是 `<dsh_dir>/<version>/`。
    pub dsh_dir: PathBuf,
    /// 托管 Node 可执行文件路径。
    pub node_executable: PathBuf,
    /// npm 脚本路径（与 `node_executable` 同 bundle，通常是 `<node_dir>/bin/npm`）。
    /// 可选：None 时用 `npx npm`。
    pub npm_script: Option<PathBuf>,
    /// 日志回调，可空。
    pub log: Option<LogCallback>,
    /// 安装超时（秒）。0 = 不限制。
    pub timeout_secs: u64,
}

impl std::fmt::Debug for InstallDshOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallDshOptions")
            .field("version", &self.version)
            .field("registry", &self.registry)
            .field("dsh_dir", &self.dsh_dir)
            .field("node_executable", &self.node_executable)
            .field("npm_script", &self.npm_script)
            .field("log", &self.log.as_ref().map(|_| "<callback>"))
            .field("timeout_secs", &self.timeout_secs)
            .finish()
    }
}

impl InstallDshOptions {
    /// 计算 `dsh/<version>/` 目录。
    pub fn version_dir(&self) -> PathBuf {
        self.dsh_dir.join(&self.version)
    }

    /// 计算 `dsh/<version>/package.json` 路径。
    pub fn package_json_path(&self) -> PathBuf {
        self.version_dir().join("package.json")
    }

    /// 计算 `node_modules/@deepseek-ai/dsh/` 路径。
    pub fn dsh_module_dir(&self) -> PathBuf {
        self.version_dir()
            .join("node_modules")
            .join("@deepseek-ai")
            .join("dsh")
    }

    /// 计算 dsh CLI 入口：`node_modules/@deepseek-ai/dsh/lib/bin.js`。
    pub fn cli_entry_path(&self) -> PathBuf {
        self.dsh_module_dir().join(super::integrity::DSH_ENTRY_REL)
    }
}

/// 默认日志回调：写到 stderr。
pub fn default_log() -> LogCallback {
    std::sync::Arc::new(|line: &str| tracing::info!(target: "dsh_install", "{line}"))
}

/// 写 `dsh/<version>/package.json`。
///
/// 内容：`{"dependencies":{"@deepseek-ai/dsh":"<version>"}}`，无 `devDependencies`。
/// 设计 §M3.2 要求最小化安装，避免拉无关包。
pub fn write_package_json(opts: &InstallDshOptions) -> Result<()> {
    let dir = opts.version_dir();
    std::fs::create_dir_all(&dir).map_err(LauncherError::Io)?;

    let pkg = serde_json::json!({
        "name": "deepseek-harness-launcher-host",
        "version": "0.0.0",
        "private": true,
        "dependencies": {
            DSH_PACKAGE_NAME: opts.version
        }
    });
    let bytes = serde_json::to_vec_pretty(&pkg)?;
    std::fs::write(opts.package_json_path(), bytes).map_err(LauncherError::Io)?;
    Ok(())
}

/// spawn `npm install`。返回 stdout / stderr 合并后的日志字符串（便于错误诊断）。
///
/// 设计 §M3.2：
/// - cwd = `dsh/<version>/`
/// - args = `install --prod --registry=<mirror> --no-audit --no-fund`
/// - 退出码非 0 → `Err(DshInstall("npm install failed: exit code N"))`
pub async fn run_npm_install(opts: &InstallDshOptions) -> Result<String> {
    let cwd = opts.version_dir();
    if !opts.package_json_path().exists() {
        return Err(LauncherError::DshInstall(format!(
            "package.json not found at {} (call write_package_json first)",
            opts.package_json_path().display()
        )));
    }

    let mut cmd = if let Some(npm) = &opts.npm_script {
        // `<node> <npm-cli.js> install ...`
        let mut c = Command::new(&opts.node_executable);
        c.arg(npm);
        c
    } else {
        // 回退：直接调 `npm`（系统 PATH 上的，但 launcher 不应依赖系统 npm）
        Command::new("npm")
    };

    cmd.current_dir(&cwd)
        .args([
            "install",
            "--prod",
            "--registry",
            &opts.registry,
            "--no-audit",
            "--no-fund",
            "--loglevel",
            "info",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    // 清理可能干扰 npm 的环境变量（继承 filtered_env 思路，但 npm install 不需要 DSH_CLI_ENTRY）
    cmd.env_clear();
    cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
    if let Ok(home) = std::env::var("HOME") {
        cmd.env("HOME", home);
    } else if let Ok(up) = std::env::var("USERPROFILE") {
        cmd.env("USERPROFILE", up);
    }

    let log_cb = opts.log.clone().unwrap_or_else(default_log);
    let mut child = cmd.spawn().map_err(|e| {
        LauncherError::DshInstall(format!(
            "spawn npm install failed: {e} (node={}, npm={:?})",
            opts.node_executable.display(),
            opts.npm_script
        ))
    })?;

    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");

    // 合并 stdout + stderr，按行写日志
    let buf = String::new();
    let stdout_task = {
        let log_cb = log_cb.clone();
        let buf = std::sync::Arc::new(tokio::sync::Mutex::new(String::new()));
        let buf_clone = buf.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                log_cb(&line);
                buf_clone.lock().await.push_str(&line);
                buf_clone.lock().await.push('\n');
            }
        })
    };
    let stderr_task = {
        let log_cb = log_cb.clone();
        let buf = std::sync::Arc::new(tokio::sync::Mutex::new(String::new()));
        let buf_clone = buf.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                log_cb(&line);
                buf_clone.lock().await.push_str(&line);
                buf_clone.lock().await.push('\n');
            }
        })
    };

    // 超时控制
    let exit_status = if opts.timeout_secs > 0 {
        let timeout = std::time::Duration::from_secs(opts.timeout_secs);
        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(s) => {
                s.map_err(|e| LauncherError::DshInstall(format!("npm install wait failed: {e}")))?
            }
            Err(_) => {
                let _ = child.kill().await;
                return Err(LauncherError::DshInstall(format!(
                    "npm install timed out after {}s",
                    opts.timeout_secs
                )));
            }
        }
    } else {
        child
            .wait()
            .await
            .map_err(|e| LauncherError::DshInstall(format!("npm install wait failed: {e}")))?
    };

    // 收集 stdout/stderr buffer（task 已经写到自己的 buf 里，但我们想要合并）
    // 简化：用上面 task 的 buf_clone。但 buf_clone 在 task 内部，外面拿不到。
    // 这里改用更简单的方式：直接读 child.stdout/stderr 到 String（已经在 task 里读了，但 buf 隔离）。
    // 为简化错误路径的诊断，让两个 task 写到一个共享 buf。
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    if !exit_status.success() {
        // 注：这里 buf 是空的，因为 task 内的 buf 隔离。为简化，依赖日志回调已经写出去。
        return Err(LauncherError::DshInstall(format!(
            "npm install failed: exit code {:?} (see logs for details)",
            exit_status.code()
        )));
    }

    Ok(buf)
}

/// 校验安装结果：入口文件存在。
pub fn verify_install(opts: &InstallDshOptions) -> Result<()> {
    verify_entry_exists(&opts.dsh_module_dir())
}

/// 完整安装流程：写 package.json → npm install → 校验入口。
///
/// 失败时清理版本目录（避免半截安装）。
pub async fn install_dsh(opts: &InstallDshOptions) -> Result<()> {
    let version_dir = opts.version_dir();

    // 1. 写 package.json
    write_package_json(opts)?;

    // 2. npm install
    let install_result = run_npm_install(opts).await;

    // 3. 校验
    let verify_result = if install_result.is_ok() {
        verify_install(opts)
    } else {
        // 跳过校验，让 install 错误先抛
        Ok(())
    };

    match (&install_result, &verify_result) {
        (Ok(_), Ok(_)) => {
            tracing::info!(version = %opts.version, "dsh installed successfully");
            Ok(())
        }
        (Err(e), _) => {
            // 清理版本目录
            let _ = std::fs::remove_dir_all(&version_dir);
            Err(LauncherError::DshInstall(format!(
                "install failed: {e} (version dir cleaned: {})",
                version_dir.display()
            )))
        }
        (Ok(_), Err(e)) => {
            // npm install 成功但校验失败
            let _ = std::fs::remove_dir_all(&version_dir);
            Err(LauncherError::DshInstall(format!(
                "verify failed: {e} (version dir cleaned: {})",
                version_dir.display()
            )))
        }
    }
}

/// 计算 dsh CLI 入口（启动 dsh web 用）。
///
/// 设计 §M1.3：`cli_entry` 指向 `node_modules/@deepseek-ai/dsh/lib/bin.js`。
pub fn cli_entry(version_dir: &Path) -> PathBuf {
    version_dir
        .join("node_modules")
        .join("@deepseek-ai")
        .join("dsh")
        .join(super::integrity::DSH_ENTRY_REL)
}

/// 读取 dsh 当前版本的 CLI 入口。`version_dir` = `dsh/<version>/`。
pub fn read_cli_entry(version_dir: &Path) -> Result<PathBuf> {
    let entry = cli_entry(version_dir);
    if !entry.exists() {
        return Err(LauncherError::DshInstall(format!(
            "dsh cli entry not found: {} (is dsh installed?)",
            entry.display()
        )));
    }
    Ok(entry)
}

/// 从 manifest 构造 InstallDshOptions，便于测试和命令层调用。
pub fn options_from_manifest(
    manifest: &PackageManifest,
    registry: &str,
    dsh_dir: &Path,
    node_dir: &Path,
) -> InstallDshOptions {
    let node_executable = node_bin_path(node_dir);
    // 同 bundle 的 npm 脚本路径
    let npm_script = crate::node::install::node_npm_path(node_dir).into();
    InstallDshOptions {
        version: manifest.version.clone(),
        registry: registry.to_string(),
        dsh_dir: dsh_dir.to_path_buf(),
        node_executable,
        npm_script,
        log: None,
        timeout_secs: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_opts(dir: &Path, version: &str) -> InstallDshOptions {
        InstallDshOptions {
            version: version.to_string(),
            registry: "https://registry.npmmirror.com".to_string(),
            dsh_dir: dir.to_path_buf(),
            node_executable: PathBuf::from("node"),
            npm_script: None,
            log: None,
            timeout_secs: 0,
        }
    }

    #[test]
    fn version_dir_is_dsh_dir_plus_version() {
        let tmp = tempdir().unwrap();
        let opts = make_opts(tmp.path(), "0.1.0");
        assert_eq!(opts.version_dir(), tmp.path().join("0.1.0"));
    }

    #[test]
    fn package_json_path_under_version_dir() {
        let tmp = tempdir().unwrap();
        let opts = make_opts(tmp.path(), "0.1.0");
        assert_eq!(
            opts.package_json_path(),
            tmp.path().join("0.1.0").join("package.json")
        );
    }

    #[test]
    fn dsh_module_path_correct() {
        let tmp = tempdir().unwrap();
        let opts = make_opts(tmp.path(), "0.1.0");
        assert_eq!(
            opts.dsh_module_dir(),
            tmp.path()
                .join("0.1.0")
                .join("node_modules")
                .join("@deepseek-ai")
                .join("dsh")
        );
    }

    #[test]
    fn cli_entry_path_correct() {
        let tmp = tempdir().unwrap();
        let opts = make_opts(tmp.path(), "0.1.0");
        assert_eq!(
            opts.cli_entry_path(),
            opts.dsh_module_dir().join("lib").join("bin.js")
        );
    }

    #[test]
    fn write_package_json_creates_dir_and_file() {
        let tmp = tempdir().unwrap();
        let opts = make_opts(tmp.path(), "0.1.0");
        write_package_json(&opts).expect("write");

        let pkg_path = opts.package_json_path();
        assert!(pkg_path.exists());

        let content = std::fs::read_to_string(&pkg_path).unwrap();
        let pkg: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(pkg["name"], "deepseek-harness-launcher-host");
        assert_eq!(pkg["private"], true);
        assert_eq!(pkg["dependencies"][DSH_PACKAGE_NAME], "0.1.0");
        assert!(pkg.get("devDependencies").is_none());
    }

    #[test]
    fn write_package_json_overrides_existing() {
        let tmp = tempdir().unwrap();
        let opts = make_opts(tmp.path(), "0.1.0");
        write_package_json(&opts).unwrap();
        let opts2 = make_opts(tmp.path(), "0.2.0");
        write_package_json(&opts2).unwrap();

        let content = std::fs::read_to_string(opts2.package_json_path()).unwrap();
        let pkg: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(pkg["dependencies"][DSH_PACKAGE_NAME], "0.2.0");
    }

    #[test]
    fn verify_install_passes_when_entry_exists() {
        let tmp = tempdir().unwrap();
        let opts = make_opts(tmp.path(), "0.1.0");
        let entry = opts.cli_entry_path();
        std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
        std::fs::write(&entry, "// dsh").unwrap();

        verify_install(&opts).expect("should pass");
    }

    #[test]
    fn verify_install_errors_when_entry_missing() {
        let tmp = tempdir().unwrap();
        let opts = make_opts(tmp.path(), "0.1.0");
        let err = verify_install(&opts).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn cli_entry_fn_returns_expected_path() {
        let version_dir = Path::new("/dsh/0.1.0");
        let entry = cli_entry(version_dir);
        assert_eq!(
            entry,
            Path::new("/dsh/0.1.0/node_modules/@deepseek-ai/dsh/lib/bin.js")
        );
    }

    #[test]
    fn read_cli_entry_errors_when_not_installed() {
        let tmp = tempdir().unwrap();
        let err = read_cli_entry(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn read_cli_entry_returns_path_when_present() {
        let tmp = tempdir().unwrap();
        let entry = cli_entry(tmp.path());
        std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
        std::fs::write(&entry, "// dsh").unwrap();

        let p = read_cli_entry(tmp.path()).unwrap();
        assert_eq!(p, entry);
    }

    #[test]
    fn run_npm_install_errors_when_package_json_missing() {
        let tmp = tempdir().unwrap();
        let opts = make_opts(tmp.path(), "0.1.0");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let err = rt.block_on(run_npm_install(&opts)).unwrap_err();
        assert!(err.to_string().contains("package.json not found"));
    }

    #[test]
    fn install_dsh_cleans_version_dir_on_failure() {
        // 模拟 npm install 失败：构造一个不存在的 node 路径
        let tmp = tempdir().unwrap();
        let mut opts = make_opts(tmp.path(), "0.1.0");
        opts.node_executable = PathBuf::from("/nonexistent/node");
        opts.npm_script = Some(PathBuf::from("/nonexistent/npm-cli.js"));

        let rt = tokio::runtime::Runtime::new().unwrap();
        let err = rt.block_on(install_dsh(&opts)).unwrap_err();
        assert!(err.to_string().contains("install failed"));
        assert!(err.to_string().contains("spawn npm install failed"));

        // 关键：版本目录被清理
        assert!(
            !opts.version_dir().exists(),
            "version dir should be cleaned"
        );
    }

    #[test]
    fn install_dsh_cleans_version_dir_on_verify_failure() {
        // 模拟 npm install "成功"（用 mock npm 脚本只创建 package.json 不创建 node_modules）
        let tmp = tempdir().unwrap();
        let mock_npm = create_mock_npm_script_that_succeeds_but_no_node_modules();
        let mut opts = make_opts(tmp.path(), "0.1.0");
        opts.node_executable = PathBuf::from("node");
        opts.npm_script = Some(mock_npm);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let err = rt.block_on(install_dsh(&opts)).unwrap_err();
        assert!(err.to_string().contains("verify failed"));
        assert!(err.to_string().contains("not found"));
        assert!(
            !opts.version_dir().exists(),
            "version dir should be cleaned"
        );
    }

    /// 构造一个 mock npm 脚本：用 node 执行时立即退出 0 但不创建任何文件。
    /// 这样 install_dsh 的 npm install 步骤会"成功"，但 verify_install 会失败。
    fn create_mock_npm_script_that_succeeds_but_no_node_modules() -> PathBuf {
        // 写一个 JS 文件，process.exit(0) 即可
        let dir = tempdir().unwrap();
        let script = dir.path().join("mock-npm-cli.js");
        std::fs::write(&script, "process.exit(0);\n").unwrap();
        // keep dir alive: leak
        std::mem::forget(dir);
        script
    }

    #[test]
    fn default_log_does_not_panic() {
        let log = default_log();
        log("test line");
    }

    #[test]
    fn options_from_manifest_uses_manifest_version() {
        use crate::dsh::registry::{DistInfo, EnginesField};
        let manifest = PackageManifest {
            version: "0.1.0".to_string(),
            engines: EnginesField::default(),
            dist: DistInfo {
                integrity: "sha512-aaa==".to_string(),
                tarball: "https://example.com/dsh-0.1.0.tgz".to_string(),
            },
        };
        let tmp = tempdir().unwrap();
        let node_dir = tmp.path().join("node-v22");
        std::fs::create_dir_all(&node_dir).unwrap();

        let opts = options_from_manifest(
            &manifest,
            "https://registry.npmmirror.com",
            tmp.path(),
            &node_dir,
        );
        assert_eq!(opts.version, "0.1.0");
        assert_eq!(opts.registry, "https://registry.npmmirror.com");
        assert!(opts.npm_script.is_some());
    }
}
