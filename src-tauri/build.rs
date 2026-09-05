// IPC 命令清单来自 src/ipc_commands.rs（单一事实源），
// 此处声明后 tauri-build 为每个命令生成 allow-<command> ACL 权限。
include!("src/ipc_commands.rs");

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(IPC_COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to run tauri-build");
}
