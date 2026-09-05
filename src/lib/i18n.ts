import { computed, shallowRef } from "vue";

export type Locale = "zh-CN" | "en-US";

const LOCALE_CACHE_KEY = "deepseek-harness-launcher.locale";

const messages = {
  "zh-CN": {
    "language.label": "语言",
    "language.description": "选择启动器界面语言",
    "language.zh": "中文",
    "language.en": "English",
    "language.switchTo": "切换为 {language}",
    "appearance.title": "外观",
    "appearance.theme": "黑白主题",
    "appearance.darkDescription": "黑色背景，白色文字",
    "appearance.lightDescription": "白色背景，黑色文字",
    "theme.switchToDark": "切换为黑白主题",
    "theme.switchToLight": "切换为浅色主题",
    "theme.saving": "正在保存…",
    "firstRun.preparing": "正在准备运行环境",
    "firstRun.description":
      "首次启动需要下载 Node.js 运行时和 DeepSeek Harness。",
    "firstRun.completed": "已完成",
    "firstRun.preparingStatus": "准备中",
    "firstRun.waiting": "等待中",
    "firstRun.switchNpm": "切换 npm 下载源",
    "firstRun.switchDownload": "切换下载来源",
    "firstRun.npmExplanation":
      "安装缓慢时可切换 npm 下载源。重新开始会停止当前安装，并使用所选来源重新下载 DeepSeek Harness。",
    "firstRun.nodeExplanation":
      "下载缓慢时可切换 Node.js 来源。重新开始会停止当前下载，并使用所选来源从头下载。",
    "firstRun.npmRegistry": "npm 下载源",
    "firstRun.selectSource": "选择下载源",
    "firstRun.npmOfficial": "npmjs.com（官方）",
    "firstRun.restarting": "正在重新开始…",
    "firstRun.restartNpm": "重新使用此 npm 来源下载",
    "firstRun.restartSource": "重新使用此来源下载",
    "main.dshVersion": "DeepSeek Harness 版本",
    "main.nodeVersion": "Node 版本",
    "main.notInstalled": "未安装",
    "main.notManaged": "未托管",
    "main.installing": "安装中…",
    "main.installDsh": "安装 DeepSeek Harness",
    "main.starting": "启动中…",
    "main.startDsh": "启动 DeepSeek Harness",
    "main.recovered":
      "DeepSeek Harness 曾意外退出，已自动恢复（第 {attempt} 次）",
    "host.starting": "正在启动 DeepSeek Harness…",
    "settings.back": "返回",
    "settings.backAria": "返回 DeepSeek Harness",
    "settings.title": "设置",
    "settings.launcher": "启动器",
    "settings.plugins": "插件",
    "pluginCommand.title": "插件命令",
    "pluginCommand.managed": "托管运行时",
    "pluginCommand.description":
      "使用当前托管的 dsh 安装或卸载单个插件来源。输入命令后，启动器会先展示将执行的操作。",
    "pluginCommand.commandTitle": "输入命令",
    "pluginCommand.supported": "支持 add / remove",
    "pluginCommand.quickInstall": "安装插件",
    "pluginCommand.quickRemove": "卸载插件",
    "pluginCommand.inputLabel": "插件安装或卸载命令",
    "pluginCommand.hint": "仅支持单条命令，不会执行 shell 语法。",
    "pluginCommand.review": "检查命令",
    "pluginCommand.invalid":
      "使用 dsh plugin --profile <profile> add|remove <source>，来源必须是单个参数。",
    "pluginCommand.install": "安装",
    "pluginCommand.remove": "卸载",
    "pluginCommand.reviewTitle": "确认{action}插件",
    "pluginCommand.reviewDescription":
      "启动器会以独立参数调用托管 dsh，不读取 PATH，也不会执行输入中的其他内容。",
    "pluginCommand.action": "操作",
    "pluginCommand.profile": "Profile",
    "pluginCommand.source": "来源",
    "pluginCommand.edit": "返回编辑",
    "pluginCommand.confirm": "确认{action}",
    "pluginCommand.running": "执行中…",
    "pluginCommand.restarting": "正在重启 dsh…",
    "pluginCommand.completed": "已完成{action}",
    "pluginCommand.completedAndRestarted": "已完成{action}，dsh 已重启",
    "pluginCommand.failed": "插件操作失败，请检查来源与 profile 后重试。",
    "pluginCommand.safety":
      "插件来源由 dsh 处理。安装前请确认来源可信，并在卸载时使用已安装时的同一来源。",
    "pluginList.title": "已安装插件",
    "pluginList.description":
      "当前 web profile 中已安装、可直接卸载的第三方插件。",
    "pluginList.empty": "还没有已安装的第三方插件。",
    "pluginList.failed": "无法读取已安装插件列表。",
    "pluginList.loading": "正在读取插件列表…",
    "pluginList.uninstall": "卸载",
    "pluginList.confirm": "确认卸载",
    "pluginList.cancel": "取消",
    "settings.description":
      "管理 DeepSeek Harness 的运行时、更新源和诊断信息。",
    "environment.title": "运行环境",
    "environment.dshVersion": "正在使用的 DeepSeek Harness 版本",
    "environment.updateAvailable": "可更新版本：",
    "environment.upToDate": "已是最新版本",
    "environment.notInstalled": "尚未安装",
    "environment.installing": "安装中…",
    "environment.installUpdate": "安装新版本",
    "environment.refreshing": "刷新中…",
    "environment.refresh": "刷新",
    "environment.nodeVersion": "Node.js 版本",
    "environment.nodeHint": "仅更新 Node.js；运行中的 dsh 不会重启",
    "environment.preparing": "准备中…",
    "environment.updateNode": "更新 Node",
    "environment.hostAddress": "运行 IP 与端口",
    "environment.hostDescription": "DeepSeek Harness 当前服务地址",
    "environment.notRunning": "尚未运行",
    "environment.launchToken": "启动令牌",
    "environment.launchTokenDescription":
      "本次会话的访问令牌，用于命令行或浏览器直接访问 dsh",
    "environment.showToken": "显示令牌",
    "environment.hideToken": "隐藏令牌",
    "environment.noToken": "无令牌",
    "environment.checking": "正在检查更新…",
    "environment.loadFailed": "无法读取运行环境：{detail}",
    "environment.latestFailed": "无法检查最新版本：{detail}",
    "versions.title": "已下载的版本",
    "versions.verified": "可用",
    "versions.broken": "无法使用",
    "versions.unknown": "状态未知",
    "command.title": "命令行",
    "command.installTitle": "安装 dsh 命令",
    "command.description":
      "安装 dsh 时会自动创建。你也可以在这里恢复被移除的命令。",
    "command.installing": "安装中…",
    "command.install": "安装命令",
    "command.installed": "已安装：",
    "command.installedTitle": "dsh 命令已安装",
    "command.installedDescription":
      "在新终端中管理与启动器相同的 DeepSeek Harness profile。",
    "command.uninstalling": "移除中…",
    "command.uninstall": "移除命令",
    "command.conflictTitle": "已有其他 dsh 命令",
    "command.conflictDescription":
      "该位置不是启动器创建的命令，因此无法覆盖或移除。",
    "command.loading": "正在读取命令状态…",
    "command.loadFailed": "无法读取命令行状态：{detail}",
    "sources.title": "下载来源",
    "sources.node": "Node.js 下载源",
    "sources.nodeDescription": "下次下载或更新 Node.js 时使用",
    "sources.npm": "npm 下载源",
    "sources.npmDescription": "下次安装或更新 DeepSeek Harness 时使用",
    "sources.select": "选择下载源",
    "sources.official": "npmjs.com（官方）",
    "sources.loading": "正在加载下载来源…",
    "sources.loadFailed": "无法加载下载来源：{detail}",
    "support.title": "问题排查",
    "support.exportTitle": "导出排查资料",
    "support.exportDescription": "打包应用状态和日志，便于反馈问题",
    "support.exporting": "导出中…",
    "support.export": "导出",
    "uninstall.title": "重新安装",
    "uninstall.runtime": "重新安装 DeepSeek Harness",
    "uninstall.description":
      "清除当前托管的 DeepSeek Harness、Node.js 运行时和设置，保留启动器与诊断日志。重新打开后即可重新安装。",
    "uninstall.action": "重新安装",
    "uninstall.confirm":
      "确认后将关闭应用并清除当前托管环境。重新打开启动器后，即可重新安装 DeepSeek Harness。",
    "common.cancel": "取消",
    "uninstall.running": "正在准备重新安装…",
    "uninstall.exit": "清除并退出",
    "mirror.label": "镜像源",
    "mirror.autoPicking": "正在选择…",
    "mirror.autoPick": "自动选择最快源",
    "mirror.select": "选择镜像源",
    "mirror.custom": "自定义…",
    "mirror.customUrl": "自定义源 URL",
    "mirror.valid": "校验通过，将使用此源",
    "about.notRunning": "未运行",
    "about.loadFailed": "无法读取运行时信息。",
    "about.launcherVersion": "启动器版本",
    "about.dshVersion": "DeepSeek Harness 版本",
    "about.nodeVersion": "Node.js 版本",
    "about.dataDirectory": "数据目录",
    "about.endpoint": "DeepSeek Harness 端点",
    "about.loading": "读取中…",
    "error.title": "操作失败",
    "error.close": "关闭",
    "error.retryBootstrap": "重新准备运行环境",
    "error.retryNode": "重试安装 Node",
    "error.retryDsh": "重试安装 Harness",
    "error.retryStart": "重试启动",
    "error.retryShutdown": "重试关闭",
    "error.retry": "重试",
    "update.failedTitle": "dsh 更新失败",
    "update.failedDescription":
      "新版本没有安装成功，当前版本仍然可以继续使用。",
    "update.failed": "更新失败",
    "update.changeSource": "更换源",
    "update.nodeRequired": "需要升级 Node",
    "update.nodeRequiredDescription":
      "dsh {dshVersion} 需要 Node {requiredVersion}，当前为 {currentVersion}。",
    "update.currentNode": "当前 Node",
    "update.willInstall": "将安装",
    "update.cancel": "取消更新",
    "update.confirm": "确认升级并继续",
    "update.updating": "正在更新 dsh",
    "update.updatingDescription":
      "正在下载并校验新版本，当前会话继续使用旧版本。",
    "update.current": "当前",
    "update.target": "目标",
    "update.wait": "请稍候",
    "update.cancelling": "正在取消…",
    "update.rollbackNodeFailed": "Node 回滚失败，请检查运行时后重试。",
    "crash.exitCode": "退出码 {value}",
    "crash.signal": "信号 {value}",
    "crash.title": "DeepSeek Harness 反复崩溃",
    "crash.description":
      "Host 在短时间内崩溃了 {count} 次（自动重试上限 {limit} 次）{detail}，已停止自动重启。",
    "crash.hint": "可以重试启动；若新版本存在问题，可回滚到上一个稳定版本。",
    "crash.dismiss": "忽略",
    "crash.processing": "处理中…",
    "crash.rollback": "回滚到 {version}",
    "crash.restarting": "重启中…",
    "crash.retry": "重试启动",
    "crash.exit": "退出应用",
    "settings.nodeComplete": "已完成原子切换",
    "settings.nodeExtracting": "正在解压、校验并切换 Node.js…",
    "settings.nodeDownloading": "正在下载并校验 Node.js 运行时…",
    "settings.nodeOnly": "仅更新 Node",
    "settings.nodeReinstall": "重新安装 Node",
    "settings.rollback": "新版本无法启动，已恢复 {version}。",
    "settings.exportTitle": "导出诊断信息",
    "settings.zip": "ZIP 压缩包",
    "settings.exported": "已导出（{size} KB）：{destination}",
    "settings.nodeSourceSaveFailed": "未能保存 Node.js 下载来源，请重试。",
    "settings.npmSourceSaveFailed": "未能保存 npm 下载源，请重试。",
    "settings.loading": "正在加载设置",
    "settings.loadFailed": "无法加载设置",
    "settings.nodeUpgradeDescription":
      "dsh {dshVersion} 需要 Node {requiredVersion}，当前为 {currentVersion}。确认后将下载 Node {targetVersion} 并继续更新。",
    "settings.upgrading": "升级中…",
    "settings.nodeUpdateTitle": "更新 Node.js",
    "settings.nodeUpdateDescription":
      "将从 {currentVersion} 更新至 {targetVersion}。该操作只更新 Node.js，不安装或切换 dsh，运行中的 dsh 会继续使用当前进程。",
    "settings.compatibility": "兼容要求：",
    "settings.noCompatibility":
      "当前 dsh 未声明 Node.js 兼容范围，将使用 launcher 已验证的版本。",
    "settings.cancelDownload": "取消下载",
    "settings.updating": "更新中…",
    "firstRun.nodeVerified": "SHA-256 已校验",
    "firstRun.nodeResolving": "正在确认 DeepSeek Harness 版本与 Node.js 要求…",
    "firstRun.nodeExtracting": "正在解压并原子切换…",
    "firstRun.nodeDownloading": "正在下载并校验运行时…",
    "firstRun.dshVerified": "完整性已校验",
    "firstRun.dshResolving": "正在准备依赖…",
    "firstRun.dshDownloading": "npm install 进行中，已处理 {count} 个包…",
    "firstRun.dshInstalling": "正在运行安装脚本…",
    "firstRun.dshVerifying": "正在校验安装结果…",
    "firstRun.dshNext": "即将自动安装…",
    "firstRun.dshWaiting": "等待 Node.js 运行时…",
    "firstRun.statusComplete": "✓ 已完成",
    "update.cancellingInstall": "正在取消安装…",
    "update.restartingDsh": "正在重启 dsh…",
    "update.upgradingNode": "正在下载并切换 Node {version}…",
    "update.resolving": "正在从当前下载源获取最新版本…",
    "update.downloading": "npm install 进行中，已处理 {count} 个包…",
    "update.installing": "正在安装依赖…",
    "update.verifying": "正在校验安装结果…",
    "update.rollbackVersion":
      "版本 {version} 无法启动，已恢复 {activeVersion}。",
    "update.startError": "启动失败原因",
    "update.noVersion": "未获取到可安装的新版本，请重新检查更新。",
    "update.running": "更新中…",
    "update.now": "立即更新",
    "update.available": "发现新版本",
    "update.availableDescription":
      "当前 {currentVersion}，可更新至 {targetVersion}。更新会在你确认后开始，并自动重启服务。",
    "bootstrap.noMirror": "未选择镜像源",
    "sources.tsinghua": "tsinghua（清华大学）",
    "toast.dismiss": "关闭通知",
  },
  "en-US": {
    "language.label": "Language",
    "language.description": "Choose the launcher interface language",
    "language.zh": "中文",
    "language.en": "English",
    "language.switchTo": "Switch to {language}",
    "appearance.title": "Appearance",
    "appearance.theme": "Black and white theme",
    "appearance.darkDescription": "Black background, white text",
    "appearance.lightDescription": "White background, black text",
    "theme.switchToDark": "Use black and white theme",
    "theme.switchToLight": "Use light theme",
    "theme.saving": "Saving…",
    "firstRun.preparing": "Preparing your environment",
    "firstRun.description":
      "The first launch downloads the Node.js runtime and DeepSeek Harness.",
    "firstRun.completed": "Complete",
    "firstRun.preparingStatus": "Preparing",
    "firstRun.waiting": "Waiting",
    "firstRun.switchNpm": "Change npm registry",
    "firstRun.switchDownload": "Change download source",
    "firstRun.npmExplanation":
      "If installation is slow, choose another npm registry. Restarting stops the current installation and downloads DeepSeek Harness again from the selected registry.",
    "firstRun.nodeExplanation":
      "If download is slow, choose another Node.js source. Restarting stops the current download and starts again from the selected source.",
    "firstRun.npmRegistry": "npm registry",
    "firstRun.selectSource": "Select a download source",
    "firstRun.npmOfficial": "npmjs.com (official)",
    "firstRun.restarting": "Restarting…",
    "firstRun.restartNpm": "Download again from this npm registry",
    "firstRun.restartSource": "Download again from this source",
    "main.dshVersion": "DeepSeek Harness version",
    "main.nodeVersion": "Node version",
    "main.notInstalled": "Not installed",
    "main.notManaged": "Not managed",
    "main.installing": "Installing…",
    "main.installDsh": "Install DeepSeek Harness",
    "main.starting": "Starting…",
    "main.startDsh": "Start DeepSeek Harness",
    "main.recovered":
      "DeepSeek Harness exited unexpectedly and was recovered automatically (attempt {attempt})",
    "host.starting": "Starting DeepSeek Harness…",
    "settings.back": "Back",
    "settings.backAria": "Back to DeepSeek Harness",
    "settings.title": "Settings",
    "settings.launcher": "Launcher",
    "settings.plugins": "Plugins",
    "pluginCommand.title": "Plugin commands",
    "pluginCommand.managed": "Managed runtime",
    "pluginCommand.description":
      "Install or remove one plugin source with the managed dsh. The launcher shows the exact action before it runs.",
    "pluginCommand.commandTitle": "Enter a command",
    "pluginCommand.supported": "add / remove supported",
    "pluginCommand.quickInstall": "Install a plugin",
    "pluginCommand.quickRemove": "Remove a plugin",
    "pluginCommand.inputLabel": "Plugin install or remove command",
    "pluginCommand.hint": "One command only. Shell syntax is never run.",
    "pluginCommand.review": "Review command",
    "pluginCommand.invalid":
      "Use dsh plugin --profile <profile> add|remove <source>. The source must be one argument.",
    "pluginCommand.install": "Install",
    "pluginCommand.remove": "Remove",
    "pluginCommand.reviewTitle": "Confirm plugin {action}",
    "pluginCommand.reviewDescription":
      "The launcher calls managed dsh with separate arguments. It does not read PATH or run any other text in the input.",
    "pluginCommand.action": "Action",
    "pluginCommand.profile": "Profile",
    "pluginCommand.source": "Source",
    "pluginCommand.edit": "Edit",
    "pluginCommand.confirm": "Confirm {action}",
    "pluginCommand.running": "Running…",
    "pluginCommand.restarting": "Restarting dsh…",
    "pluginCommand.completed": "{action} complete",
    "pluginCommand.completedAndRestarted": "{action} complete, dsh restarted",
    "pluginCommand.failed":
      "The plugin action failed. Check the source and profile, then try again.",
    "pluginCommand.safety":
      "dsh handles plugin sources. Verify the source before installation, and use the same source that was installed when removing it.",
    "pluginList.title": "Installed plugins",
    "pluginList.description":
      "Third-party plugins currently installed in the web profile. Use Uninstall to remove one.",
    "pluginList.empty": "No third-party plugins are installed.",
    "pluginList.failed": "Could not read the installed plugin list.",
    "pluginList.loading": "Reading installed plugins…",
    "pluginList.uninstall": "Uninstall",
    "pluginList.confirm": "Confirm uninstall",
    "pluginList.cancel": "Cancel",
    "settings.description":
      "Manage the DeepSeek Harness runtime, update sources, and diagnostics.",
    "environment.title": "Runtime",
    "environment.dshVersion": "Current DeepSeek Harness version",
    "environment.updateAvailable": "Available update: ",
    "environment.upToDate": "Up to date",
    "environment.notInstalled": "Not installed",
    "environment.installing": "Installing…",
    "environment.installUpdate": "Install update",
    "environment.refreshing": "Refreshing…",
    "environment.refresh": "Refresh",
    "environment.nodeVersion": "Node.js version",
    "environment.nodeHint":
      "Only Node.js will be updated; the running dsh will not restart",
    "environment.preparing": "Preparing…",
    "environment.updateNode": "Update Node",
    "environment.hostAddress": "Host address",
    "environment.hostDescription": "Current DeepSeek Harness service address",
    "environment.notRunning": "Not running",
    "environment.launchToken": "Launch token",
    "environment.launchTokenDescription":
      "Session access token for reaching dsh from a terminal or browser",
    "environment.showToken": "Show token",
    "environment.hideToken": "Hide token",
    "environment.noToken": "No token",
    "environment.checking": "Checking for updates…",
    "environment.loadFailed": "Unable to read runtime settings: {detail}",
    "environment.latestFailed": "Unable to check for updates: {detail}",
    "versions.title": "Downloaded versions",
    "versions.verified": "Ready",
    "versions.broken": "Unavailable",
    "versions.unknown": "Unknown",
    "command.title": "Command line",
    "command.installTitle": "Install the dsh command",
    "command.description":
      "Created automatically when dsh is installed. You can also restore it here after removal.",
    "command.installing": "Installing…",
    "command.install": "Install command",
    "command.installed": "Installed: ",
    "command.installedTitle": "dsh command installed",
    "command.installedDescription":
      "Manage the same DeepSeek Harness profile as the launcher from a new terminal.",
    "command.uninstalling": "Removing…",
    "command.uninstall": "Remove command",
    "command.conflictTitle": "Another dsh command already exists",
    "command.conflictDescription":
      "This command was not created by the launcher, so it cannot be replaced or removed here.",
    "command.loading": "Reading command status…",
    "command.loadFailed": "Unable to read command-line status: {detail}",
    "sources.title": "Download sources",
    "sources.node": "Node.js download source",
    "sources.nodeDescription": "Used for the next Node.js download or update",
    "sources.npm": "npm registry",
    "sources.npmDescription":
      "Used for the next DeepSeek Harness installation or update",
    "sources.select": "Select a download source",
    "sources.official": "npmjs.com (official)",
    "sources.loading": "Loading download sources…",
    "sources.loadFailed": "Unable to load download sources: {detail}",
    "support.title": "Troubleshooting",
    "support.exportTitle": "Export diagnostics",
    "support.exportDescription":
      "Bundle application state and logs to help investigate an issue",
    "support.exporting": "Exporting…",
    "support.export": "Export",
    "uninstall.title": "Reinstall",
    "uninstall.runtime": "Reinstall DeepSeek Harness",
    "uninstall.description":
      "Clear the managed DeepSeek Harness, Node.js runtime, and settings while keeping the launcher and diagnostic logs. Reopen the launcher to install it again.",
    "uninstall.action": "Reinstall",
    "uninstall.confirm":
      "The app will close and clear the current managed environment. Reopen the launcher to install DeepSeek Harness again.",
    "common.cancel": "Cancel",
    "uninstall.running": "Preparing to reinstall…",
    "uninstall.exit": "Clear and quit",
    "mirror.label": "Mirror",
    "mirror.autoPicking": "Selecting…",
    "mirror.autoPick": "Pick the fastest source",
    "mirror.select": "Select a mirror",
    "mirror.custom": "Custom…",
    "mirror.customUrl": "Custom source URL",
    "mirror.valid": "Validated; this source will be used",
    "about.notRunning": "Not running",
    "about.loadFailed": "Unable to read runtime information.",
    "about.launcherVersion": "Launcher version",
    "about.dshVersion": "DeepSeek Harness version",
    "about.nodeVersion": "Node.js version",
    "about.dataDirectory": "Data directory",
    "about.endpoint": "DeepSeek Harness endpoint",
    "about.loading": "Loading…",
    "error.title": "Action failed",
    "error.close": "Close",
    "error.retryBootstrap": "Prepare the environment again",
    "error.retryNode": "Retry Node installation",
    "error.retryDsh": "Retry Harness installation",
    "error.retryStart": "Retry startup",
    "error.retryShutdown": "Retry shutdown",
    "error.retry": "Retry",
    "update.failedTitle": "dsh update failed",
    "update.failedDescription":
      "The new version could not be installed. The current version is still available.",
    "update.failed": "Update failed",
    "update.changeSource": "Change source",
    "update.nodeRequired": "Node upgrade required",
    "update.nodeRequiredDescription":
      "dsh {dshVersion} requires Node {requiredVersion}; you are using {currentVersion}.",
    "update.currentNode": "Current Node",
    "update.willInstall": "Will install",
    "update.cancel": "Cancel update",
    "update.confirm": "Upgrade and continue",
    "update.updating": "Updating dsh",
    "update.updatingDescription":
      "Downloading and verifying the update. Your current session keeps using the old version.",
    "update.current": "Current",
    "update.target": "Target",
    "update.wait": "Please wait",
    "update.cancelling": "Cancelling…",
    "update.rollbackNodeFailed":
      "Node rollback failed; check the runtime and try again.",
    "crash.exitCode": "Exit code {value}",
    "crash.signal": "Signal {value}",
    "crash.title": "DeepSeek Harness keeps crashing",
    "crash.description":
      "The host crashed {count} times in a short period (automatic retry limit: {limit}){detail}. Automatic restarts have stopped.",
    "crash.hint":
      "Try starting again, or roll back to the previous stable version if the new version is the problem.",
    "crash.dismiss": "Ignore",
    "crash.processing": "Processing…",
    "crash.rollback": "Roll back to {version}",
    "crash.restarting": "Restarting…",
    "crash.retry": "Retry startup",
    "crash.exit": "Exit application",
    "settings.nodeComplete": "Atomic switch complete",
    "settings.nodeExtracting": "Extracting, verifying, and switching Node.js…",
    "settings.nodeDownloading":
      "Downloading and verifying the Node.js runtime…",
    "settings.nodeOnly": "Update Node only",
    "settings.nodeReinstall": "Reinstall Node",
    "settings.rollback": "The new version could not start; restored {version}.",
    "settings.exportTitle": "Export diagnostics",
    "settings.zip": "ZIP archive",
    "settings.exported": "Exported ({size} KB): {destination}",
    "settings.nodeSourceSaveFailed":
      "Unable to save the Node.js download source. Try again.",
    "settings.npmSourceSaveFailed":
      "Unable to save the npm registry. Try again.",
    "settings.loading": "Loading settings",
    "settings.loadFailed": "Unable to load settings",
    "settings.nodeUpgradeDescription":
      "dsh {dshVersion} requires Node {requiredVersion}; you are using {currentVersion}. Node {targetVersion} will be downloaded before the update continues.",
    "settings.upgrading": "Upgrading…",
    "settings.nodeUpdateTitle": "Update Node.js",
    "settings.nodeUpdateDescription":
      "Node.js will be updated from {currentVersion} to {targetVersion}. This does not install or switch dsh, and the running dsh keeps using the current process.",
    "settings.compatibility": "Compatibility:",
    "settings.noCompatibility":
      "The current dsh does not declare a Node.js compatibility range. A launcher-verified version will be used.",
    "settings.cancelDownload": "Cancel download",
    "settings.updating": "Updating…",
    "firstRun.nodeVerified": "SHA-256 verified",
    "firstRun.nodeResolving":
      "Checking the DeepSeek Harness version and Node.js requirement…",
    "firstRun.nodeExtracting": "Extracting and switching atomically…",
    "firstRun.nodeDownloading": "Downloading and verifying the runtime…",
    "firstRun.dshVerified": "Integrity verified",
    "firstRun.dshResolving": "Preparing dependencies…",
    "firstRun.dshDownloading":
      "npm install is running; {count} packages processed…",
    "firstRun.dshInstalling": "Running installation scripts…",
    "firstRun.dshVerifying": "Verifying the installation…",
    "firstRun.dshNext": "Installation will start automatically…",
    "firstRun.dshWaiting": "Waiting for the Node.js runtime…",
    "firstRun.statusComplete": "✓ Complete",
    "update.cancellingInstall": "Cancelling installation…",
    "update.restartingDsh": "Restarting dsh…",
    "update.upgradingNode": "Downloading and switching Node {version}…",
    "update.resolving": "Fetching the latest version from the current source…",
    "update.downloading": "npm install is running; {count} packages processed…",
    "update.installing": "Installing dependencies…",
    "update.verifying": "Verifying the installation…",
    "update.rollbackVersion":
      "Version {version} could not start; restored {activeVersion}.",
    "update.startError": "Why it failed to start",
    "update.noVersion":
      "No installable new version was found. Check for updates again.",
    "update.running": "Updating…",
    "update.now": "Update now",
    "update.available": "Update available",
    "update.availableDescription":
      "Current {currentVersion}; update available: {targetVersion}. The update starts after you confirm and automatically restarts the service.",
    "bootstrap.noMirror": "No mirror is selected",
    "sources.tsinghua": "Tsinghua University",
    "toast.dismiss": "Dismiss notification",
  },
} as const;

type MessageKey = keyof (typeof messages)["zh-CN"];
type Interpolation = Record<string, string | number>;

function cachedLocale(): Locale {
  if (typeof window === "undefined") return "zh-CN";
  try {
    const locale = window.localStorage.getItem(LOCALE_CACHE_KEY);
    return locale === "zh-CN" || locale === "en-US" ? locale : "zh-CN";
  } catch {
    return "zh-CN";
  }
}

export const locale = shallowRef<Locale>(cachedLocale());

function applyLocale(value: Locale): void {
  if (typeof document !== "undefined") document.documentElement.lang = value;
}

function cacheLocale(value: Locale): void {
  try {
    window.localStorage.setItem(LOCALE_CACHE_KEY, value);
  } catch {
    // 无存储权限时，当前会话仍可切换语言。
  }
}

export function initializeLocale(): void {
  applyLocale(locale.value);
}

export function setLocale(value: Locale): void {
  locale.value = value;
  applyLocale(value);
  cacheLocale(value);
}

export function useI18n() {
  function t(key: MessageKey, interpolation?: Interpolation): string {
    const message = messages[locale.value][key] as string;
    if (!interpolation) return message;
    return message.replace(/\{(\w+)\}/g, (placeholder, name: string) =>
      name in interpolation ? String(interpolation[name]) : placeholder,
    );
  }

  const alternateLocale = computed<Locale>(() =>
    locale.value === "zh-CN" ? "en-US" : "zh-CN",
  );

  return { locale, alternateLocale, setLocale, t };
}
