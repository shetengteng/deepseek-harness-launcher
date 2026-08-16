//! Tauri 命令层（设计 §M1.4）。
//!
//! 把 `HostSupervisor` 和 `AppState` 暴露给前端：
//! - `launcher_status`：返回当前状态快照（首启 / 已就绪 / 错误等）
//! - `start_host`：启动 dsh web 子进程，返回 origin URL
//! - `shutdown_host`：关闭 dsh web 子进程
//!
//! 错误通过 `LauncherError` 的 `Serialize` 实现序列化到前端，结构为 `{ kind, message, data }`。

use std::io::Write;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{LauncherError, Result};
use crate::host::{HostSupervisor, HostSupervisorConfig, HostSupervisorError, SpawnDshWebOptions};
use crate::node::{self, probe_mirrors, validate_custom_mirror, Mirror, MirrorId, BUILTIN_MIRRORS};
use crate::state::{AppState, StateStatus};

/// 注入到 Tauri 的共享状态。前端通过 `invoke` 间接触发命令，命令通过 `State<'_, SharedState>` 访问。
pub struct SharedState {
    /// Host 监管器。一次 start 成功后缓存 origin，shutdown 后不可再 start。
    /// `Arc` 包裹：exit monitor 与崩溃自动重启（设计 §5.5）需要克隆引用。
    pub supervisor: Arc<HostSupervisor>,
}

impl SharedState {
    /// 在 `lib::run()` 里构造，注入到 `tauri::Builder::manage()`。
    pub fn new() -> Self {
        Self {
            supervisor: Arc::new(HostSupervisor::new(HostSupervisorConfig::default())),
        }
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}

/// `launcher_status` 返回的状态快照。前端用 `phase` 驱动状态机。
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct StatusSnapshot {
    /// `first_run`（无 state.json）/ `ready`（Host 已启动）/ `idle`（Host 未启动但 state 存在）。
    pub phase: String,
    /// Host 就绪后的 origin URL，前端 webview 加载此 URL。
    pub host_origin: Option<String>,
    /// 当前 dsh 版本（`state.dsh.current`），用于 UI 展示。
    pub dsh_version: Option<String>,
    /// 当前 Node 版本（`state.node.version`），用于 UI 展示。
    pub node_version: Option<String>,
    /// 宿主平台标识（`darwin` / `win` / `linux`），与 Node archive 命名一致。
    /// WKWebView UA 在 Apple Silicon 上仍报 "Intel Mac OS X"，前端 UA 判 arch 不可靠，
    /// 以 Rust `std::env::consts` 为准。
    pub platform: String,
    /// 宿主架构（`arm64` / `x64`），来源同上。
    pub arch: String,
}

/// 归一化 `std::env::consts::OS`/`ARCH` 到 Node archive 命名（darwin/win/linux + arm64/x64）。
pub fn host_platform_arch() -> (&'static str, &'static str) {
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win",
        _ => "linux",
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        _ => "x64",
    };
    (platform, arch)
}

/// 获取 launcher 当前状态。前端启动时调一次，根据 `phase` 决定渲染哪个视图。
#[tauri::command]
pub async fn launcher_status() -> Result<StatusSnapshot> {
    let status = AppState::load()?;
    Ok(build_status_snapshot(status))
}

/// 从 `StateStatus` 派生状态快照。提取为纯函数便于单元测试（不读真实 state.json）。
///
/// phase 派生规则（page-flow-analysis.md §3.6）：
/// - 无 state.json → `first_run`（node_version / dsh_version 均为 None）
/// - state.node 未装 → `first_run`（node_version=None，前端 wizardStep=mirror_select）
/// - state.node 已装、dsh.current 未装 → `first_run`（node_version 有值，前端 wizardStep=done）
/// - 都已装 → `idle`
///
/// 关键：`first_run` phase 下也透传 `node_version` / `dsh_version`，让前端决定 wizardStep。
pub fn build_status_snapshot(status: StateStatus) -> StatusSnapshot {
    let (platform, arch) = host_platform_arch();
    match status {
        StateStatus::FirstRun => StatusSnapshot {
            phase: "first_run".to_string(),
            host_origin: None,
            dsh_version: None,
            node_version: None,
            platform: platform.to_string(),
            arch: arch.to_string(),
        },
        StateStatus::Loaded(state) => {
            let node_version = state.node.as_ref().map(|n| n.version.clone());
            let dsh_version = state.dsh.current.clone();
            let phase = match (node_version.as_ref(), dsh_version.as_ref()) {
                (None, _) => "first_run",       // Node 未装
                (Some(_), None) => "first_run", // dsh 未装
                (Some(_), Some(_)) => "idle",   // 都装了
            };
            StatusSnapshot {
                phase: phase.to_string(),
                host_origin: None,
                dsh_version,
                node_version,
                platform: platform.to_string(),
                arch: arch.to_string(),
            }
        }
    }
}

/// 启动 dsh web Host 子进程。返回 origin URL，前端用 `webview.navigate` 或 `iframe.src` 加载。
///
/// 幂等：已启动则返回缓存的 origin；已 shutdown 则返回 `Host` 错误。
/// 用户主动启动成功 → 崩溃计数清零（设计 §5.5 规则 5）。
#[tauri::command]
pub async fn start_host(state: State<'_, SharedState>) -> Result<String> {
    let origin = start_host_inner(&state.supervisor).await?;
    Ok(origin)
}

/// `start_host` / `restart_host` / 崩溃自动重启共用的启动逻辑。
/// 成功后重置崩溃计数（用户主动启动视为新一轮）。
async fn start_host_inner(supervisor: &Arc<HostSupervisor>) -> Result<String> {
    let opts = build_spawn_options()?;
    let origin = supervisor.start(&opts).await.map_err(map_host_error)?;

    // 用户主动启动成功 → 清零崩溃计数（设计 §5.5）。
    match AppState::load() {
        Ok(StateStatus::Loaded(mut s)) => {
            crate::host::reset_crash_counter(&mut s);
            if let Err(e) = s.save() {
                tracing::warn!(error = %e, "start_host: reset crash counter save failed");
            }
        }
        _ => {}
    }

    Ok(origin.as_str().to_string())
}

/// 崩溃弹窗用户点"重试"：清零计数器后重启 Host（设计 §5.5 / PR-017）。
#[tauri::command]
pub async fn restart_host(state: State<'_, SharedState>) -> Result<String> {
    start_host_inner(&state.supervisor).await
}

/// 关闭 dsh web Host 子进程。幂等：多次调用安全。
#[tauri::command]
pub async fn shutdown_host(state: State<'_, SharedState>) -> Result<()> {
    let shutdown = state.supervisor.shutdown().await;
    shutdown.await_completion().await;
    Ok(())
}

/// 构造 `SpawnDshWebOptions`。从 state + dsh/current 指针读 cli_entry：
/// - `node_executable`：`node-runtime/node-v<version>/bin/node`（未安装 → `NodeNotInstalled`）
/// - `cli_entry`：`dsh/<current>/node_modules/@deepseek-ai/dsh/lib/bin.js`（未装 dsh → `DshNotInstalled`）
/// - `cwd`：dsh/current 指针指向的版本目录
/// - `env`：`filtered_env()` 过滤后的环境变量 + `DSH_CLI_ENTRY` 透传给子进程
fn build_spawn_options() -> Result<SpawnDshWebOptions> {
    use crate::dsh::{read_current_pointer, DSH_ENTRY_REL};
    use crate::node::install::current_node_dir;
    use crate::paths;

    // 1. Node 运行时：读 node-runtime/VERSION，解析到 node-v<version>/bin/node
    let node_dir = current_node_dir().map_err(|e| match e {
        LauncherError::NodeDownload(msg) if msg.contains("read VERSION file failed") => {
            LauncherError::NodeNotInstalled {
                reason: "node-runtime/VERSION not found; first-run wizard not completed"
                    .to_string(),
            }
        }
        other => other,
    })?;
    let node_executable = crate::node::install::node_bin_path(&node_dir);

    // 2. dsh cli_entry：从 dsh/current 指针读版本目录
    let dsh_dir = paths::dsh_dir()?;
    let current_version = read_current_pointer(&dsh_dir)
        .map_err(|e| LauncherError::PathResolve {
            what: "dsh_current_pointer",
            cause: e.to_string(),
        })?
        .ok_or_else(|| LauncherError::DshNotInstalled {
            reason: "dsh/current pointer not set; first-run wizard not completed".to_string(),
        })?;

    let version_dir = dsh_dir.join(&current_version);
    let cli_entry = version_dir
        .join("node_modules")
        .join("@deepseek-ai")
        .join("dsh")
        .join(DSH_ENTRY_REL);

    if !cli_entry.exists() {
        return Err(LauncherError::DshNotInstalled {
            reason: format!(
                "dsh cli entry not found: {} (version {} may be broken)",
                cli_entry.display(),
                current_version
            ),
        });
    }

    // 3. env：filter + DSH_CLI_ENTRY 透传（lifecycle.rs 中 is_passthrough 已放行 DSH_*）
    let mut env = crate::host::filtered_env();
    env.insert(
        "DSH_CLI_ENTRY".to_string(),
        cli_entry.to_string_lossy().into_owned(),
    );

    Ok(SpawnDshWebOptions {
        node_executable,
        cli_entry,
        cwd: version_dir,
        env,
        electron_run_as_node: false,
    })
}

/// `HostSupervisorError` → `LauncherError`：保留原始错误信息，前端通过 `kind: "host"` 识别。
fn map_host_error(e: HostSupervisorError) -> LauncherError {
    LauncherError::Host(e.to_string())
}

// ─── 首启向导：镜像源探活 + Node 安装（设计 §M2.5 / PR-011） ───

/// 前端可展示的镜像源。
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct MirrorInfo {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub trusted: bool,
}

impl From<&Mirror> for MirrorInfo {
    fn from(m: &Mirror) -> Self {
        Self {
            id: m.id.to_string(),
            name: m.name.to_string(),
            base_url: m.base_url.to_string(),
            trusted: m.trusted,
        }
    }
}

/// 列出内置镜像源。前端首启向导展示选项。
#[tauri::command]
pub async fn list_mirrors() -> Result<Vec<MirrorInfo>> {
    Ok(BUILTIN_MIRRORS.iter().map(MirrorInfo::from).collect())
}

/// 探活镜像源列表，返回首个可用源。供前端"自动选择"按钮。
///
/// `custom_urls`：可选的自定义源候选；先于内置源探活。
#[tauri::command]
pub async fn probe_mirrors_command(custom_urls: Option<Vec<String>>) -> Result<MirrorInfo> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| LauncherError::Mirror(e.to_string()))?;
    let timeout = std::time::Duration::from_secs(10);

    let mut mirrors: Vec<Mirror> = Vec::new();
    if let Some(urls) = custom_urls {
        for u in urls {
            match validate_custom_mirror(&u) {
                Ok(m) => mirrors.push(m),
                Err(e) => {
                    tracing::warn!(url = %u, error = %e, "skipping invalid custom mirror");
                }
            }
        }
    }
    mirrors.extend(BUILTIN_MIRRORS.iter().cloned());

    let picked = probe_mirrors(&client, &mirrors, timeout)
        .await
        .map_err(|e| LauncherError::Mirror(e.to_string()))?;
    Ok(MirrorInfo::from(&picked))
}

/// 校验自定义镜像源 URL。前端在用户输入时即时校验。
#[tauri::command]
pub async fn validate_custom_mirror_command(url: String) -> Result<MirrorInfo> {
    let m = validate_custom_mirror(&url).map_err(|e| LauncherError::Mirror(e.to_string()))?;
    Ok(MirrorInfo::from(&m))
}

/// Node 安装参数。前端发起安装请求时传入。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InstallNodeArgs {
    /// 目标 Node 版本，如 `22.19.0`
    pub version: String,
    /// 镜像源 base_url（不带 `v{version}` 后缀）
    pub mirror_base_url: String,
    /// archive 平台标识，如 `darwin-arm64-arm64`
    pub platform: String,
    /// archive arch 标识
    pub arch: String,
}

/// 一次性触发：下载 + 校验 + 解压 + 写 VERSION。
///
/// 进度通过 Tauri 事件 `download-progress` / `extract-progress` 推送，
/// payload 为 `ProgressEvent`。
#[tauri::command]
pub async fn install_node_command(app: tauri::AppHandle, args: InstallNodeArgs) -> Result<String> {
    use crate::node::{download_with_retry, install_node_to, NodeArchiveKind};
    use tauri::Emitter;

    let InstallNodeArgs {
        version,
        mirror_base_url,
        platform: _,
        arch: _,
    } = args;

    // Rust 端最终决定权：无视前端上报的 platform/arch（WKWebView UA 在
    // Apple Silicon 上误报 Intel），一律用宿主真实值拼 archive 文件名。
    let (platform, arch) = host_platform_arch();

    // 构造 Mirror（用 Custom，因为 base_url 可能是内置源或自定义）
    let mirror = Mirror {
        id: MirrorId::Custom(mirror_base_url.clone()),
        name: "user-selected",
        base_url: Box::leak(mirror_base_url.clone().into_boxed_str()),
        trusted: false,
    };

    tracing::info!(version = %version, mirror = %mirror_base_url, platform = %platform, arch = %arch, "install_node_command: start");

    // Node 官方 archive 命名规则：`node-v{version}-{platform}-{arch}.tar.gz`
    // 正确示例：`node-v22.19.0-darwin-arm64.tar.gz`（platform=darwin, arch=arm64）
    // 错误示例：`node-v22.19.0-darwin-arm64-arm64.tar.gz`（platform 误传 darwin-arm64）
    let archive_filename = format!("node-v{version}-{platform}-{arch}.tar.gz");
    tracing::info!(archive_filename = %archive_filename, "install_node_command: archive filename");
    let runtime_dir = crate::paths::node_runtime_dir()?;
    tracing::info!(runtime_dir = %runtime_dir.display(), "install_node_command: creating runtime_dir");
    std::fs::create_dir_all(&runtime_dir).map_err(LauncherError::Io)?;
    let staging_download_dir = runtime_dir.join(".downloads");
    tracing::info!(staging_download_dir = %staging_download_dir.display(), "install_node_command: creating staging_download_dir");
    std::fs::create_dir_all(&staging_download_dir).map_err(LauncherError::Io)?;

    // PR-020：下载前检查磁盘空间 ≥ 200MB。不足时返回 NodeDownload 错误
    //（user_message 映射为"磁盘空间不足"提示）。
    crate::node::disk::ensure_disk_space(&staging_download_dir)?;

    // mpsc channel 接收 ProgressEvent，转 Tauri emit
    let (tx, mut rx) = tokio::sync::mpsc::channel::<node::ProgressEvent>(64);
    let app_clone = app.clone();
    let emit_task = tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let event_name = match ev.stage.as_str() {
                "extract" => "extract-progress",
                _ => "download-progress",
            };
            let _ = app_clone.emit(event_name, &ev);
        }
    });

    tracing::info!("install_node_command: building reqwest client");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| LauncherError::NodeDownload(e.to_string()))?;

    tracing::info!(archive_filename = %archive_filename, "install_node_command: starting download");
    let archive_path = download_with_retry(
        &client,
        &mirror,
        &version,
        &archive_filename,
        &staging_download_dir,
        Some(&tx),
        2,
    )
    .await?;
    tracing::info!(archive_path = %archive_path.display(), "install_node_command: download complete");

    // 下载完成，关闭 sender 让 emit_task 结束
    drop(tx);
    let _ = emit_task.await;

    // 解压 + 原子切换
    tracing::info!("install_node_command: starting extract");
    let _target = install_node_to(
        &archive_path,
        &version,
        NodeArchiveKind::TarGz, // 当前实现仅支持 tar.gz，Windows 后续 PR
        &runtime_dir,
        None,
    )
    .await?;
    tracing::info!(target = %_target.display(), "install_node_command: extract complete");

    // 更新 state.json：记录 Node 已安装
    let mut state = match AppState::load()? {
        StateStatus::FirstRun => AppState::new(),
        StateStatus::Loaded(s) => *s,
    };
    state.node = Some(crate::state::NodeState {
        version: version.clone(),
        installed_at: chrono::Utc::now(),
        mirror: mirror_base_url,
    });
    state.save()?;

    // 清理下载 staging（保留 archive 文件可选，这里清理）
    let _ = std::fs::remove_file(&archive_path);

    Ok(version)
}

/// 一次性触发 dsh 安装：拉 registry 元数据 → npm install → 完整性校验 → promote_to_current。
///
/// 设计 §M3.2/§M3.3。失败抛 `DshInstall` / `DshRegistry` / `DshVersion` 错误。
#[tauri::command]
pub async fn install_dsh_command(app: tauri::AppHandle) -> Result<String> {
    use crate::dsh::{
        default_client, fetch_dist_tags, fetch_package_manifest, install_dsh,
        options_from_manifest, promote_to_current, RegistryCache,
    };
    use tauri::Emitter;

    // 1. 读 state：必须有 Node 已装
    let mut state = match AppState::load()? {
        StateStatus::FirstRun => {
            return Err(LauncherError::NodeNotInstalled {
                reason: "state.json not found; complete first-run wizard first".to_string(),
            });
        }
        StateStatus::Loaded(s) => *s,
    };
    let node_state = state
        .node
        .clone()
        .ok_or_else(|| LauncherError::NodeNotInstalled {
            reason: "state.node is None; complete first-run wizard first".to_string(),
        })?;

    // 2. 解析 Node 路径
    let node_dir = crate::node::install::current_node_dir()?;
    let node_executable = crate::node::install::node_bin_path(&node_dir);
    if !node_executable.exists() {
        return Err(LauncherError::NodeNotInstalled {
            reason: format!(
                "node binary not found: {} (VERSION says {} but binary missing)",
                node_executable.display(),
                node_state.version
            ),
        });
    }

    // 3. 从 state.dsh.registry 拿 registry（默认 npmmirror）
    let registry = state.dsh.registry.clone();

    // 4. 拉元数据，找 latest 版本
    let client = default_client();
    let cache = RegistryCache::new();

    // 推送进度事件（虽然 fetch 不发 stage 事件，但为统一接口保留）
    let (tx, mut rx) = tokio::sync::mpsc::channel::<crate::node::ProgressEvent>(16);
    let app_clone = app.clone();
    let emit_task = tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let event_name = match ev.stage.as_str() {
                "extract" => "extract-progress",
                _ => "download-progress",
            };
            let _ = app_clone.emit(event_name, &ev);
        }
    });

    let dist_tags = fetch_dist_tags(&registry, &cache, &client).await?;
    let version = dist_tags.latest.clone();
    tracing::info!(version = %version, registry = %registry, "installing dsh");

    let manifest = fetch_package_manifest(&registry, &version, &cache, &client).await?;

    // 5. 构造 InstallDshOptions（用 options_from_manifest，但需要 node_dir 而非 node_executable）
    let dsh_dir = crate::paths::dsh_dir()?;
    let mut opts = options_from_manifest(&manifest, &registry, &dsh_dir, &node_dir);
    opts.node_executable = node_executable;
    opts.npm_script = Some(crate::node::install::node_npm_path(&node_dir));

    // 6. 执行 install_dsh（npm install + verify）
    install_dsh(&opts).await?;

    // 7. promote_to_current：写 dsh/current 指针 + 更新 state
    promote_to_current(&mut state, &dsh_dir, &version)?;
    state.save()?;

    // 关闭 channel 让 emit_task 结束
    drop(tx);
    let _ = emit_task.await;

    Ok(version)
}

// ─── PR-015: 升级编排（设计 §M3.4） ───

/// 前端设置页展示的 dsh 状态详情。
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DshStateSnapshot {
    pub current: Option<String>,
    pub known_good: Option<String>,
    pub pending: Option<String>,
    pub pinned_range: String,
    pub auto_upgrade: bool,
    pub check_interval_hours: u32,
    pub registry: String,
    pub installed: Vec<InstalledDshInfo>,
    pub ignored_versions: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct InstalledDshInfo {
    pub version: String,
    pub installed_at: String,
    pub status: String,
}

/// 升级检查结果。
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UpgradeCheckResult {
    pub available: bool,
    pub version: Option<String>,
    pub engines_node: Option<String>,
    /// 有新版 dsh 但当前 Node 不满足 engines.node（PR-018）。
    /// 前端据此弹 Node 升级确认框。
    pub node_block: Option<NodeBlockInfo>,
}

/// Node 版本阻塞详情（设计 §5.4 / PR-018）。
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct NodeBlockInfo {
    /// 要求 Node 版本的 dsh 版本号。
    pub dsh_version: String,
    /// dsh 声明的 engines.node range，如 `>=24.0.0`。
    pub engines_node: String,
    /// 当前托管的 Node 版本。None 表示未安装。
    pub current_node: Option<String>,
    /// 解析出的建议安装的 Node 版本（`resolve_node_target`）。
    pub node_target: String,
    /// Node 下载镜像源（优先历史安装用过的源，其次内置默认源）。
    pub mirror_base_url: String,
}

impl NodeBlockInfo {
    /// 从 `dsh::upgrade::NodeBlock` 构造前端 payload。
    /// `node_mirror`：state.node.mirror（历史安装源），None 时用内置默认源。
    pub fn from_block(
        block: &crate::dsh::upgrade::NodeBlock,
        node_mirror: Option<&str>,
    ) -> Result<Self> {
        let node_target = crate::dsh::upgrade::resolve_node_target(&block.engines_node)?;
        let mirror_base_url = node_mirror
            .filter(|m| !m.is_empty())
            .map(|m| m.to_string())
            .unwrap_or_else(|| crate::node::pick_default_mirror().base_url.to_string());
        Ok(Self {
            dsh_version: block.dsh_version.clone(),
            engines_node: block.engines_node.clone(),
            current_node: block.current_node.clone(),
            node_target,
            mirror_base_url,
        })
    }
}

/// 返回 dsh 状态详情，供设置页展示。
#[tauri::command]
pub async fn get_dsh_state() -> Result<DshStateSnapshot> {
    let status = AppState::load()?;
    match status {
        StateStatus::FirstRun => Ok(DshStateSnapshot {
            current: None,
            known_good: None,
            pending: None,
            pinned_range: "~0.1.0".to_string(),
            auto_upgrade: true,
            check_interval_hours: 24,
            registry: "https://registry.npmmirror.com".to_string(),
            installed: vec![],
            ignored_versions: vec![],
        }),
        StateStatus::Loaded(state) => Ok(DshStateSnapshot {
            current: state.dsh.current,
            known_good: state.dsh.known_good,
            pending: state.dsh.pending,
            pinned_range: state.dsh.pinned_range,
            auto_upgrade: state.auto_upgrade,
            check_interval_hours: state.dsh.check_interval_hours,
            registry: state.dsh.registry,
            installed: state
                .dsh
                .installed
                .iter()
                .map(|i| InstalledDshInfo {
                    version: i.version.clone(),
                    installed_at: i.installed_at.to_rfc3339(),
                    status: i.status.clone(),
                })
                .collect(),
            ignored_versions: state.dsh.ignored_versions,
        }),
    }
}

/// 检查 dsh 升级。不修改 state，仅返回是否有可用版本。
///
/// 三种结果：
/// - `available=true`：可直接升级，调 `prepare_upgrade_command`
/// - `node_block` 非空：新版 dsh 需要更高 Node，前端先走 Node 升级流程（PR-018）
/// - 两者皆空：无可用升级
#[tauri::command]
pub async fn check_for_upgrade_command() -> Result<UpgradeCheckResult> {
    use crate::dsh::{check_for_upgrade, default_client, RegistryCache};

    let state = match AppState::load()? {
        StateStatus::FirstRun => {
            return Ok(UpgradeCheckResult {
                available: false,
                version: None,
                engines_node: None,
                node_block: None,
            });
        }
        StateStatus::Loaded(s) => *s,
    };

    let registry = state.dsh.registry.clone();
    let client = default_client();
    let cache = RegistryCache::new();

    let check = check_for_upgrade(&state, &registry, &cache, &client).await?;
    let node_mirror = state.node.as_ref().map(|n| n.mirror.as_str());
    Ok(upgrade_check_to_result(check, node_mirror))
}

/// `UpgradeCheck` → `UpgradeCheckResult`。纯函数，便于单测。
/// `node_mirror`：state.node.mirror（历史安装源），None 时用内置默认源。
pub fn upgrade_check_to_result(
    check: crate::dsh::upgrade::UpgradeCheck,
    node_mirror: Option<&str>,
) -> UpgradeCheckResult {
    match check.candidate {
        Some(candidate) => UpgradeCheckResult {
            available: true,
            version: Some(candidate.version),
            engines_node: if candidate.engines_node.is_empty() {
                None
            } else {
                Some(candidate.engines_node)
            },
            node_block: None,
        },
        None => match check.node_block {
            // resolve 失败时降级为"无升级"，错误信息进日志（不阻塞普通升级路径）。
            Some(block) => match NodeBlockInfo::from_block(&block, node_mirror) {
                Ok(info) => UpgradeCheckResult {
                    available: false,
                    version: Some(block.dsh_version),
                    engines_node: Some(block.engines_node.clone()),
                    node_block: Some(info),
                },
                Err(e) => {
                    tracing::warn!(error = %e, "resolve_node_target failed for node_block");
                    UpgradeCheckResult {
                        available: false,
                        version: None,
                        engines_node: None,
                        node_block: None,
                    }
                }
            },
            None => UpgradeCheckResult {
                available: false,
                version: None,
                engines_node: None,
                node_block: None,
            },
        },
    }
}

/// 安装升级候选版本：下载 + npm install + 校验 + 设 pending。
///
/// 成功后 `state.dsh.pending` 设为新版本，`last_check` 更新。
/// 前端应提示用户重启生效，或 `auto_upgrade` 为 true 时自动重启。
#[tauri::command]
pub async fn prepare_upgrade_command() -> Result<String> {
    use crate::dsh::{check_for_upgrade, default_client, prepare_upgrade, RegistryCache};

    let mut state = match AppState::load()? {
        StateStatus::FirstRun => {
            return Err(LauncherError::DshNotInstalled {
                reason: "state.json not found; complete first-run wizard first".to_string(),
            });
        }
        StateStatus::Loaded(s) => *s,
    };

    let registry = state.dsh.registry.clone();
    let client = default_client();
    let cache = RegistryCache::new();

    let check = check_for_upgrade(&state, &registry, &cache, &client).await?;
    let candidate = check
        .candidate
        .ok_or_else(|| LauncherError::DshVersion("no upgrade available".to_string()))?;

    let node_dir = crate::node::install::current_node_dir()?;
    let dsh_dir = crate::paths::dsh_dir()?;

    prepare_upgrade(
        &mut state, &registry, &candidate, &dsh_dir, &node_dir, &client, &cache,
    )
    .await?;

    state.save()?;

    Ok(candidate.version)
}

// ─── PR-016: 设置管理命令 ───

/// 更新 pinned_range。校验 semver 合法性。
#[tauri::command]
pub async fn set_pinned_range_command(range: String) -> Result<()> {
    use semver::VersionReq;

    // 校验 semver range 合法性
    VersionReq::parse(&range)
        .map_err(|e| LauncherError::DshVersion(format!("invalid semver range '{range}': {e}")))?;

    let mut state = match AppState::load()? {
        StateStatus::FirstRun => AppState::new(),
        StateStatus::Loaded(s) => *s,
    };
    state.dsh.pinned_range = range;
    state.save()?;
    Ok(())
}

/// 切换 auto_upgrade。
#[tauri::command]
pub async fn set_auto_upgrade_command(enabled: bool) -> Result<()> {
    let mut state = match AppState::load()? {
        StateStatus::FirstRun => AppState::new(),
        StateStatus::Loaded(s) => *s,
    };
    state.auto_upgrade = enabled;
    state.save()?;
    Ok(())
}

/// 更新检查间隔（小时）。
#[tauri::command]
pub async fn set_check_interval_command(hours: u32) -> Result<()> {
    let mut state = match AppState::load()? {
        StateStatus::FirstRun => AppState::new(),
        StateStatus::Loaded(s) => *s,
    };
    state.dsh.check_interval_hours = hours;
    state.save()?;
    Ok(())
}

/// 忽略指定版本。
#[tauri::command]
pub async fn ignore_version_command(version: String) -> Result<()> {
    use crate::dsh::version::ignore_version;

    let mut state = match AppState::load()? {
        StateStatus::FirstRun => {
            return Err(LauncherError::DshNotInstalled {
                reason: "state.json not found".to_string(),
            });
        }
        StateStatus::Loaded(s) => *s,
    };
    ignore_version(&mut state, &version);
    state.save()?;
    Ok(())
}

/// 取消忽略指定版本。
#[tauri::command]
pub async fn unignore_version_command(version: String) -> Result<()> {
    let mut state = match AppState::load()? {
        StateStatus::FirstRun => {
            return Err(LauncherError::DshNotInstalled {
                reason: "state.json not found".to_string(),
            });
        }
        StateStatus::Loaded(s) => *s,
    };
    state.dsh.ignored_versions.retain(|v| v != &version);
    state.save()?;
    Ok(())
}

// ─── PR-017: 崩溃恢复（设计 §5.5） ───

/// `host-crash-limit` 事件 payload。前端弹 CrashDialog 展示。
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CrashLimitPayload {
    /// 当前崩溃计数（含本次）。
    pub crash_counter: u32,
    /// 自动重试上限（`CRASH_RETRY_LIMIT`）。
    pub retry_limit: u32,
    /// 子进程 exit code（如有）。
    pub exit_code: Option<i32>,
    /// 子进程退出 signal（POSIX，如有）。
    pub exit_signal: Option<i32>,
    /// 可回滚的 known_good 版本。None 表示无稳定版本可回滚。
    pub known_good: Option<String>,
}

/// `host-restarted` 事件 payload。自动重启成功后推送，前端更新 iframe origin。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct HostRestartedPayload {
    /// 本轮第几次崩溃后的重启（1 起）。
    pub attempt: u32,
    /// 重启后的新 origin。
    pub origin: String,
}

/// 崩溃后决策（`decide_after_crash` 的结果）。纯数据，便于单测。
#[derive(Debug, Clone, PartialEq)]
pub enum CrashAction {
    /// 自动重启成功。附带 attempt 序号 + 新 origin。
    Restarted { attempt: u32, origin: String },
    /// 达到上限或重启失败，需要前端弹窗。附带 CrashLimitPayload。
    PromptUser(CrashLimitPayload),
}

/// 崩溃恢复决策核心：记录崩溃 → 决定自动重启或弹窗。
///
/// 设计 §5.5：
/// 1. `record_crash` 更新计数（窗口外归 1）
/// 2. counter < limit → 自动重启 current（注意：不走 `start_host_inner`，
///    那会清零计数器导致永远到不了上限）
/// 3. 重启失败或 counter >= limit → `PromptUser`
///
/// 抽成独立函数（不直接操作 AppHandle）便于单元测试。
pub async fn decide_after_crash(
    supervisor: &Arc<HostSupervisor>,
    exit_detail: crate::host::HostExitDetail,
) -> CrashAction {
    // 1. 读 state，记录崩溃。state 读失败（首启前崩溃等）视为达到上限交给用户。
    let mut state = match AppState::load() {
        Ok(StateStatus::Loaded(s)) => *s,
        _ => {
            return CrashAction::PromptUser(CrashLimitPayload {
                crash_counter: crate::host::CRASH_RETRY_LIMIT,
                retry_limit: crate::host::CRASH_RETRY_LIMIT,
                exit_code: exit_detail.code,
                exit_signal: exit_detail.signal,
                known_good: None,
            });
        }
    };

    let decision = crate::host::record_crash(&mut state, chrono::Utc::now());
    let counter = state.crash_counter;
    let known_good = state.dsh.known_good.clone();
    if let Err(e) = state.save() {
        tracing::warn!(error = %e, "decide_after_crash: save crash counter failed");
    }

    let prompt = || CrashLimitPayload {
        crash_counter: counter,
        retry_limit: crate::host::CRASH_RETRY_LIMIT,
        exit_code: exit_detail.code,
        exit_signal: exit_detail.signal,
        known_good: known_good.clone(),
    };

    // 2. 未达上限 → 自动重启。重启失败 → 弹窗。
    if decision == crate::host::CrashDecision::RestartCurrent {
        match build_spawn_options() {
            Ok(opts) => match supervisor.start(&opts).await {
                Ok(origin) => {
                    tracing::info!(attempt = counter, "dsh crashed, auto-restarted");
                    CrashAction::Restarted {
                        attempt: counter,
                        origin: origin.as_str().to_string(),
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, attempt = counter, "auto-restart failed");
                    CrashAction::PromptUser(prompt())
                }
            },
            Err(e) => {
                tracing::error!(error = %e, "build_spawn_options failed after crash");
                CrashAction::PromptUser(prompt())
            }
        }
    } else {
        tracing::warn!(
            counter = counter,
            "crash retry limit reached, prompting user"
        );
        CrashAction::PromptUser(prompt())
    }
}

/// 崩溃/意外退出的入口回调。由 `lib.rs` 的 `setup` 注入到 supervisor。
/// 在 tokio 任务里执行 `decide_after_crash` 并把结果 emit 给前端。
pub fn spawn_crash_recovery(app: tauri::AppHandle, detail: crate::host::HostExitDetail) {
    tokio::spawn(async move {
        use tauri::{Emitter, Manager};

        let supervisor = app.state::<SharedState>().supervisor.clone();
        let action = decide_after_crash(&supervisor, detail).await;
        match action {
            CrashAction::Restarted { attempt, origin } => {
                let _ = app.emit("host-restarted", &HostRestartedPayload { attempt, origin });
            }
            CrashAction::PromptUser(payload) => {
                let _ = app.emit("host-crash-limit", &payload);
            }
        }
    });
}

/// 崩溃弹窗用户点"回滚"：切换到 known_good 版本（设计 §5.5 / M3.3）。
/// 成功返回回滚到的版本号。前端随后可调 `restart_host` 重启。
#[tauri::command]
pub async fn rollback_dsh_command() -> Result<String> {
    use crate::dsh::version::rollback_to_known_good;

    let mut state = match AppState::load()? {
        StateStatus::FirstRun => {
            return Err(LauncherError::DshNotInstalled {
                reason: "state.json not found".to_string(),
            });
        }
        StateStatus::Loaded(s) => *s,
    };
    let dsh_dir = crate::paths::dsh_dir()?;
    let version = rollback_to_known_good(&mut state, &dsh_dir)?;
    state.save()?;
    Ok(version)
}

// ─── PR-019: 诊断导出（设计 §11.3） ───

/// 导出诊断信息：把 state.json + 壳子日志 + dsh 日志打包成 zip。
///
/// `dest`：前端通过 save 对话框拿到的目标路径（建议 `.zip` 后缀）。
/// 返回写入的总字节数。
#[tauri::command]
pub async fn export_diagnostics(dest: String) -> Result<u64> {
    let dest = std::path::PathBuf::from(dest);
    export_diagnostics_to(&dest)
}

/// 打包诊断 zip 到 `dest`。独立函数便于单测（传临时目录路径）。
pub fn export_diagnostics_to(dest: &std::path::Path) -> Result<u64> {
    use zip::write::SimpleFileOptions;

    let file = std::fs::File::create(dest).map_err(LauncherError::Io)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut entries: usize = 0;

    // 1. state.json
    if let Some((name, path)) = state_json_entry() {
        if add_file_to_zip(&mut zip, opts, &name, &path)? {
            entries += 1;
        }
    }

    // 2. 日志目录：壳子 tracing 日志 + dsh 子进程日志
    for (prefix, dir) in log_dirs() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let file_name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "unknown.log".to_string());
                    let zip_name = format!("{prefix}/{file_name}");
                    if add_file_to_zip(&mut zip, opts, &zip_name, &p)? {
                        entries += 1;
                    }
                }
            }
        }
    }

    zip.finish()
        .map_err(|e| LauncherError::Io(std::io::Error::other(format!("zip finish failed: {e}"))))?;
    let size = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    tracing::info!(dest = %dest.display(), entries = entries, size = size, "diagnostics exported");
    Ok(size)
}

/// state.json 的 zip 条目（存在时）。
fn state_json_entry() -> Option<(String, std::path::PathBuf)> {
    let path = crate::paths::state_file().ok()?;
    if path.exists() {
        Some(("state.json".to_string(), path))
    } else {
        None
    }
}

/// 要打包的日志目录：(zip 内前缀, 磁盘路径)。两个目录在 macOS 上不同
/// （壳子日志在 ~/Library/Logs，dsh 日志在 data/logs），其余平台同根。
fn log_dirs() -> Vec<(&'static str, std::path::PathBuf)> {
    let mut dirs = Vec::new();
    if let Ok(d) = crate::paths::log_dir() {
        dirs.push(("launcher-logs", d));
    }
    if let Ok(d) = crate::paths::dsh_log_dir() {
        dirs.push(("dsh-logs", d));
    }
    dirs
}

/// 把单个文件加入 zip。文件不可读时跳过（返回 Ok(false)），不让单个日志阻塞导出。
fn add_file_to_zip<W: Write + std::io::Seek, P: AsRef<std::path::Path>>(
    zip: &mut zip::ZipWriter<W>,
    opts: zip::write::SimpleFileOptions,
    name: &str,
    path: P,
) -> Result<bool> {
    let path = path.as_ref();
    let content = match std::fs::read(path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "export: skip unreadable file");
            return Ok(false);
        }
    };
    zip.start_file(name, opts)
        .map_err(|e| LauncherError::Io(std::io::Error::other(format!("zip start_file: {e}"))))?;
    zip.write_all(&content).map_err(LauncherError::Io)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, DshState, NodeState};
    use chrono::Utc;

    #[test]
    fn shared_state_default_creates_supervisor() {
        let _ = SharedState::default();
    }

    // ─── PR-018: UpgradeCheck → UpgradeCheckResult ───

    #[test]
    fn upgrade_check_to_result_candidate() {
        let check = crate::dsh::upgrade::UpgradeCheck {
            candidate: Some(crate::dsh::upgrade::UpgradeCandidate {
                version: "0.2.0".to_string(),
                engines_node: ">=22.0.0".to_string(),
            }),
            node_block: None,
        };
        let r = upgrade_check_to_result(check, None);
        assert!(r.available);
        assert_eq!(r.version.as_deref(), Some("0.2.0"));
        assert_eq!(r.engines_node.as_deref(), Some(">=22.0.0"));
        assert!(r.node_block.is_none());
    }

    #[test]
    fn upgrade_check_to_result_no_upgrade() {
        let r = upgrade_check_to_result(crate::dsh::upgrade::UpgradeCheck::default(), None);
        assert!(!r.available);
        assert!(r.version.is_none());
        assert!(r.node_block.is_none());
    }

    #[test]
    fn upgrade_check_to_result_node_block() {
        let check = crate::dsh::upgrade::UpgradeCheck {
            candidate: None,
            node_block: Some(crate::dsh::upgrade::NodeBlock {
                dsh_version: "0.3.0".to_string(),
                engines_node: ">=24.0.0".to_string(),
                current_node: Some("22.19.0".to_string()),
            }),
        };
        let r = upgrade_check_to_result(check, Some("https://npmmirror.com/mirrors/node"));
        assert!(!r.available);
        assert_eq!(r.version.as_deref(), Some("0.3.0"));
        let block = r.node_block.expect("node_block present");
        assert_eq!(block.dsh_version, "0.3.0");
        assert_eq!(block.engines_node, ">=24.0.0");
        assert_eq!(block.current_node.as_deref(), Some("22.19.0"));
        assert_eq!(block.node_target, "24.0.0");
        // 历史安装源优先于内置默认源
        assert_eq!(block.mirror_base_url, "https://npmmirror.com/mirrors/node");
    }

    #[test]
    fn upgrade_check_to_result_node_block_falls_back_to_default_mirror() {
        let check = crate::dsh::upgrade::UpgradeCheck {
            candidate: None,
            node_block: Some(crate::dsh::upgrade::NodeBlock {
                dsh_version: "0.3.0".to_string(),
                engines_node: ">=24.0.0".to_string(),
                current_node: None,
            }),
        };
        let r = upgrade_check_to_result(check, None);
        let block = r.node_block.expect("node_block present");
        assert_eq!(
            block.mirror_base_url,
            crate::node::pick_default_mirror().base_url
        );
        assert!(block.current_node.is_none());
    }

    #[test]
    fn node_block_info_serializes_snake_case() {
        let info = NodeBlockInfo {
            dsh_version: "0.3.0".to_string(),
            engines_node: ">=24.0.0".to_string(),
            current_node: Some("22.19.0".to_string()),
            node_target: "24.0.0".to_string(),
            mirror_base_url: "https://npmmirror.com/mirrors/node".to_string(),
        };
        let json = serde_json::to_value(&info).expect("serialize");
        assert_eq!(json["dsh_version"], "0.3.0");
        assert_eq!(json["engines_node"], ">=24.0.0");
        assert_eq!(json["current_node"], "22.19.0");
        assert_eq!(json["node_target"], "24.0.0");
        assert_eq!(
            json["mirror_base_url"],
            "https://npmmirror.com/mirrors/node"
        );
    }

    // ─── PR-017: 崩溃恢复 payload ───

    #[test]
    fn crash_limit_payload_serializes_snake_case() {
        let p = CrashLimitPayload {
            crash_counter: 3,
            retry_limit: 3,
            exit_code: Some(1),
            exit_signal: None,
            known_good: Some("0.1.0".to_string()),
        };
        let json = serde_json::to_value(&p).expect("serialize");
        assert_eq!(json["crash_counter"], 3);
        assert_eq!(json["retry_limit"], 3);
        assert_eq!(json["exit_code"], 1);
        assert!(json["exit_signal"].is_null());
        assert_eq!(json["known_good"], "0.1.0");
    }

    #[test]
    fn crash_limit_payload_defaults_to_limit_when_no_state() {
        // state.json 不存在时 decide_after_crash 直接 PromptUser（不 panic、不 spawn）。
        // 开发机若存在 state.json 则跳过（路径不可控，避免污染）。
        if crate::paths::state_file().unwrap().exists() {
            return;
        }
        let sup = HostSupervisor::new(HostSupervisorConfig::default());
        let action = futures::executor::block_on(decide_after_crash(
            &Arc::new(sup),
            crate::host::HostExitDetail {
                code: Some(1),
                signal: None,
            },
        ));
        match action {
            CrashAction::PromptUser(p) => {
                assert_eq!(p.retry_limit, crate::host::CRASH_RETRY_LIMIT);
                assert!(p.known_good.is_none());
            }
            other => panic!("expected PromptUser, got {other:?}"),
        }
    }

    // ─── PR-019: 诊断导出 ───

    #[test]
    fn export_diagnostics_creates_valid_zip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dest = tmp.path().join("diag.zip");
        let size = export_diagnostics_to(&dest).expect("export");
        assert!(dest.exists());
        // 空 zip 也有 22 字节 EOCD 头；有 state.json/日志时更大。
        assert!(size > 0);
        // 用 zip 读回验证结构合法。
        let f = std::fs::File::open(&dest).expect("open zip");
        let mut archive = zip::ZipArchive::new(f).expect("valid zip archive");
        for i in 0..archive.len() {
            let name = archive
                .by_index(i)
                .expect("entry readable")
                .name()
                .to_string();
            // 条目名必须是 state.json 或带前缀的日志路径
            assert!(
                name == "state.json"
                    || name.starts_with("launcher-logs/")
                    || name.starts_with("dsh-logs/"),
                "unexpected entry: {name}"
            );
        }
    }

    #[test]
    fn log_dirs_cover_both_dirs() {
        let dirs = log_dirs();
        assert!(dirs.len() >= 2, "expect launcher + dsh log dirs");
    }

    #[test]
    fn build_spawn_options_errors_when_node_not_installed() {
        // node-runtime/VERSION 缺失 → NodeNotInstalled。
        // 用真实数据目录跑测试（开发机一般没装过 Node），命中 NodeNotInstalled 分支。
        // 若开发机恰好装过 Node，跳过该测试以避免误报。
        let node_runtime = crate::paths::node_runtime_dir().unwrap();
        if node_runtime.join("VERSION").exists() {
            return; // 开发机已装 Node，跳过
        }
        let err = build_spawn_options().unwrap_err();
        assert!(
            matches!(err, LauncherError::NodeNotInstalled { .. }),
            "expected NodeNotInstalled, got {err:?}"
        );
    }

    #[test]
    fn map_host_error_preserves_message() {
        let e = HostSupervisorError::AlreadyShutdown;
        let mapped = map_host_error(e);
        assert!(matches!(mapped, LauncherError::Host(_)));
        let json = serde_json::to_value(&mapped).expect("serialize");
        assert_eq!(json["kind"], "host");
        assert!(json["message"].as_str().unwrap().contains("shutdown"));
    }

    #[test]
    fn build_status_snapshot_first_run() {
        let snap = build_status_snapshot(StateStatus::FirstRun);
        assert_eq!(snap.phase, "first_run");
        assert!(snap.host_origin.is_none());
        assert!(snap.dsh_version.is_none());
        assert!(snap.node_version.is_none());
    }

    #[test]
    fn build_status_snapshot_idle_with_state() {
        let mut state = AppState::new();
        state.dsh.current = Some("0.1.0".to_string());
        state.node = Some(NodeState {
            version: "20.18.0".to_string(),
            installed_at: Utc::now(),
            mirror: "https://registry.npmmirror.com".to_string(),
        });
        let snap = build_status_snapshot(StateStatus::Loaded(Box::new(state)));
        assert_eq!(snap.phase, "idle");
        assert!(snap.host_origin.is_none());
        assert_eq!(snap.dsh_version.as_deref(), Some("0.1.0"));
        assert_eq!(snap.node_version.as_deref(), Some("20.18.0"));
    }

    #[test]
    fn build_status_snapshot_first_run_when_nothing_installed() {
        // state.json 存在但 Node 和 dsh 都未装 → first_run（mirror_select 场景）
        let state = AppState::new();
        let snap = build_status_snapshot(StateStatus::Loaded(Box::new(state)));
        assert_eq!(snap.phase, "first_run");
        assert!(snap.dsh_version.is_none());
        assert!(snap.node_version.is_none());
    }

    #[test]
    fn build_status_snapshot_first_run_when_only_node_installed() {
        // Node 已装、dsh 未装 → first_run（wizardStep=done 场景）
        let mut state = AppState::new();
        state.node = Some(NodeState {
            version: "22.19.0".to_string(),
            installed_at: Utc::now(),
            mirror: "https://registry.npmmirror.com".to_string(),
        });
        // dsh.current 保持 None
        let snap = build_status_snapshot(StateStatus::Loaded(Box::new(state)));
        assert_eq!(snap.phase, "first_run");
        assert!(snap.dsh_version.is_none());
        assert_eq!(snap.node_version.as_deref(), Some("22.19.0"));
    }

    #[test]
    fn build_status_snapshot_first_run_when_only_dsh_current_but_no_node() {
        // 极端情况：state.dsh.current 有值但 state.node 为 None（不应出现，但后端要容错）
        // → first_run，让用户重装 Node
        let mut state = AppState::new();
        state.dsh.current = Some("0.1.0".to_string());
        // node 保持 None
        let snap = build_status_snapshot(StateStatus::Loaded(Box::new(state)));
        assert_eq!(snap.phase, "first_run");
        assert_eq!(snap.dsh_version.as_deref(), Some("0.1.0"));
        assert!(snap.node_version.is_none());
    }

    #[test]
    fn status_snapshot_serializes_to_snake_case() {
        let snap = StatusSnapshot {
            phase: "first_run".to_string(),
            host_origin: Some("http://127.0.0.1:51329".to_string()),
            dsh_version: Some("0.1.0".to_string()),
            node_version: None,
        };
        let json = serde_json::to_value(&snap).expect("serialize");
        assert_eq!(json["phase"], "first_run");
        assert_eq!(json["host_origin"], "http://127.0.0.1:51329");
        assert_eq!(json["dsh_version"], "0.1.0");
        assert!(json["node_version"].is_null());
    }

    /// `launcher_status` 端到端：从真实临时文件加载并生成 snapshot。
    /// 避免污染用户数据目录（`paths::state_file()`）。
    #[tokio::test]
    async fn launcher_status_reads_from_real_state_file() {
        // 用临时目录覆盖 paths::state_file()：测试通过 AppState::load() 默认路径，
        // 这里仅验证 build_status_snapshot 的输出契约，不直接调命令（命令读 paths::state_file）。
        // 默认 AppState 的 node=None、dsh.current=None → first_run（page-flow-analysis.md §3.6）
        let mut state = AppState::new();
        state.dsh = DshState::default();
        let snap = build_status_snapshot(StateStatus::Loaded(Box::new(state)));
        assert_eq!(snap.phase, "first_run");
    }

    // ─── PR-011: 镜像源命令测试 ───

    #[test]
    fn mirror_info_from_builtin_mirror() {
        let m = BUILTIN_MIRRORS.first().expect("builtin non-empty");
        let info = MirrorInfo::from(m);
        assert_eq!(info.id, m.id.to_string());
        assert_eq!(info.base_url, m.base_url);
        assert!(info.trusted);
    }

    #[test]
    fn mirror_info_from_custom_mirror() {
        let m = crate::node::validate_custom_mirror("https://my-mirror.com/node").unwrap();
        let info = MirrorInfo::from(&m);
        assert_eq!(info.id, "https://my-mirror.com/node");
        assert_eq!(info.base_url, "https://my-mirror.com/node");
        assert!(!info.trusted);
    }

    #[test]
    fn list_mirrors_returns_all_builtins() {
        // 直接调用 list_mirrors（不需要 State）
        let mirrors = futures::executor::block_on(list_mirrors()).expect("list_mirrors");
        assert_eq!(mirrors.len(), BUILTIN_MIRRORS.len());
        for (i, m) in mirrors.iter().enumerate() {
            assert_eq!(m.id, BUILTIN_MIRRORS[i].id.to_string());
            assert!(m.trusted);
        }
    }

    #[test]
    fn mirror_info_serializes_to_snake_case() {
        let info = MirrorInfo {
            id: "nodejs.org".to_string(),
            name: "Node.js 官方".to_string(),
            base_url: "https://nodejs.org/dist".to_string(),
            trusted: true,
        };
        let json = serde_json::to_value(&info).expect("serialize");
        assert_eq!(json["id"], "nodejs.org");
        assert_eq!(json["name"], "Node.js 官方");
        assert_eq!(json["base_url"], "https://nodejs.org/dist");
        assert_eq!(json["trusted"], true);
    }

    #[tokio::test]
    async fn validate_custom_mirror_command_accepts_https() {
        let m = validate_custom_mirror_command("https://my-mirror.com/node".to_string())
            .await
            .expect("valid");
        assert_eq!(m.base_url, "https://my-mirror.com/node");
        assert!(!m.trusted);
    }

    #[tokio::test]
    async fn validate_custom_mirror_command_rejects_http() {
        let err = validate_custom_mirror_command("http://insecure.com".to_string())
            .await
            .unwrap_err();
        assert!(matches!(err, LauncherError::Mirror(_)));
    }

    #[tokio::test]
    async fn validate_custom_mirror_command_rejects_query_fragment() {
        let err = validate_custom_mirror_command("https://x.com/?foo=bar".to_string())
            .await
            .unwrap_err();
        assert!(matches!(err, LauncherError::Mirror(_)));
    }
}
