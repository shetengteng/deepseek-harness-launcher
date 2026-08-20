use super::*;
use std::path::{Path, PathBuf};

use tempfile::tempdir;

fn make_opts(dir: &Path, version: &str) -> InstallDshOptions {
    InstallDshOptions {
        version: version.to_string(),
        registry: "https://registry.npmmirror.com".to_string(),
        dsh_dir: dir.to_path_buf(),
        node_executable: PathBuf::from("node"),
        npm_script: None,
        npm_cache: None,
        npm_userconfig: None,
        log: None,
        timeout_secs: 0,
    }
}

#[test]
fn option_paths_are_under_the_requested_version() {
    let temp = tempdir().unwrap();
    let opts = make_opts(temp.path(), "0.1.0");
    assert_eq!(opts.version_dir(), temp.path().join("0.1.0"));
    assert_eq!(
        opts.package_json_path(),
        temp.path().join("0.1.0/package.json")
    );
    assert_eq!(
        opts.dsh_module_dir(),
        temp.path().join("0.1.0/node_modules/@deepseek-ai/dsh")
    );
    assert_eq!(
        opts.cli_entry_path(),
        opts.dsh_module_dir().join("lib/bin.js")
    );
}

#[test]
fn write_package_json_creates_the_minimal_dependency_manifest() {
    let temp = tempdir().unwrap();
    let opts = make_opts(temp.path(), "0.1.0");
    write_package_json(&opts).unwrap();

    let package: serde_json::Value =
        serde_json::from_slice(&std::fs::read(opts.package_json_path()).unwrap()).unwrap();
    assert_eq!(package["name"], "deepseek-harness-launcher-host");
    assert_eq!(package["private"], true);
    assert_eq!(package["dependencies"][DSH_PACKAGE_NAME], "0.1.0");
    assert!(package.get("devDependencies").is_none());
}

#[test]
fn write_package_json_uses_each_options_version() {
    let temp = tempdir().unwrap();
    write_package_json(&make_opts(temp.path(), "0.1.0")).unwrap();
    let next = make_opts(temp.path(), "0.2.0");
    write_package_json(&next).unwrap();

    let package: serde_json::Value =
        serde_json::from_slice(&std::fs::read(next.package_json_path()).unwrap()).unwrap();
    assert_eq!(package["dependencies"][DSH_PACKAGE_NAME], "0.2.0");
}

#[test]
fn verify_install_requires_the_cli_entry() {
    let temp = tempdir().unwrap();
    let opts = make_opts(temp.path(), "0.1.0");
    assert!(verify_install(&opts)
        .unwrap_err()
        .to_string()
        .contains("not found"));

    let entry = opts.cli_entry_path();
    std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
    std::fs::write(&entry, "// dsh").unwrap();
    verify_install(&opts).unwrap();
}

#[test]
fn cli_entry_helpers_return_and_validate_the_expected_path() {
    let temp = tempdir().unwrap();
    let entry = cli_entry(temp.path());
    assert_eq!(
        entry,
        temp.path().join("node_modules/@deepseek-ai/dsh/lib/bin.js")
    );
    assert!(read_cli_entry(temp.path())
        .unwrap_err()
        .to_string()
        .contains("not found"));

    std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
    std::fs::write(&entry, "// dsh").unwrap();
    assert_eq!(read_cli_entry(temp.path()).unwrap(), entry);
}

#[test]
fn options_from_manifest_uses_managed_node_paths() {
    use crate::dsh::registry::{DistInfo, EnginesField};

    let manifest = PackageManifest {
        version: "0.1.0".to_string(),
        engines: EnginesField::default(),
        dist: DistInfo {
            integrity: "sha512-aaa==".to_string(),
            tarball: "https://example.com/dsh-0.1.0.tgz".to_string(),
        },
    };
    let temp = tempdir().unwrap();
    let node_dir = temp.path().join("node-v22");
    std::fs::create_dir_all(&node_dir).unwrap();

    let opts = options_from_manifest(
        &manifest,
        "https://registry.npmmirror.com",
        temp.path(),
        &node_dir,
    );
    assert_eq!(opts.version, "0.1.0");
    assert_eq!(opts.registry, "https://registry.npmmirror.com");
    assert!(opts.npm_script.is_some());
    assert_eq!(
        opts.npm_cache.as_deref(),
        Some(temp.path().join("npm-cache").as_path())
    );
    assert_eq!(
        opts.npm_userconfig.as_deref(),
        Some(temp.path().join("npmrc").as_path())
    );
}

#[tokio::test]
async fn run_npm_install_requires_the_manifest() {
    let temp = tempdir().unwrap();
    let error = run_npm_install(&make_opts(temp.path(), "0.1.0"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("package.json not found"));
}

#[tokio::test]
async fn run_npm_install_requires_managed_npm() {
    let temp = tempdir().unwrap();
    let opts = make_opts(temp.path(), "0.1.0");
    write_package_json(&opts).unwrap();
    let error = run_npm_install(&opts).await.unwrap_err();
    assert!(error.to_string().contains("managed npm"));
}

#[tokio::test]
async fn install_dsh_cleans_the_version_directory_after_spawn_failure() {
    let temp = tempdir().unwrap();
    let mut opts = make_opts(temp.path(), "0.1.0");
    opts.node_executable = PathBuf::from("/nonexistent/node");
    opts.npm_script = Some(PathBuf::from("/nonexistent/npm-cli.js"));

    let error = install_dsh(&opts).await.unwrap_err();
    assert!(error.to_string().contains("install failed"));
    assert!(!opts.version_dir().exists());
}

#[tokio::test]
async fn cancelled_install_cleans_target_without_touching_the_current_version() {
    let temp = tempdir().unwrap();
    let current_entry = temp
        .path()
        .join("0.1.0/node_modules/@deepseek-ai/dsh/lib/bin.js");
    std::fs::create_dir_all(current_entry.parent().unwrap()).unwrap();
    std::fs::write(&current_entry, "// current dsh").unwrap();
    let opts = make_opts(temp.path(), "0.2.0");
    let operations = DshInstallOperations::default();
    let active = operations.register("update-0.2.0").unwrap();
    let cancellation = active.cancellation();
    assert!(operations.cancel("update-0.2.0"));
    assert!(cancellation.check().is_err());

    let error = install_dsh_cancellable(&opts, Some(&cancellation))
        .await
        .unwrap_err();

    assert!(error.to_string().contains("cancelled"));
    assert!(!opts.version_dir().exists());
    assert_eq!(
        std::fs::read_to_string(current_entry).unwrap(),
        "// current dsh"
    );
}

#[test]
fn default_log_accepts_output() {
    default_log()("test line");
}

#[test]
fn npm_install_path_puts_managed_node_first() {
    let node = PathBuf::from("/managed/node/bin/node");
    let path = super::process::path_with_managed_node(&node).unwrap();
    let parts: Vec<_> = std::env::split_paths(&path).collect();
    assert_eq!(
        parts.first().map(PathBuf::as_path),
        Some(Path::new("/managed/node/bin"))
    );
    assert!(parts
        .iter()
        .any(|part| part == Path::new("/usr/bin") || part.ends_with("System32")));
    assert!(!parts
        .iter()
        .any(|part| part == Path::new("/opt/homebrew/bin")));
}

#[test]
fn npm_install_path_ignores_bare_node_executable() {
    let path = super::process::path_with_managed_node(Path::new("node")).unwrap();
    let parts: Vec<_> = std::env::split_paths(&path).collect();
    assert!(!parts.iter().any(|part| part.as_os_str().is_empty()));
    assert_ne!(parts.first().map(PathBuf::as_path), Some(Path::new("node")));
}

#[test]
fn npm_install_path_uses_directory_of_node_exe() {
    let node = PathBuf::from("node-runtime/node-v24.18.1/node.exe");
    let path = super::process::path_with_managed_node(&node).unwrap();
    assert_eq!(
        std::env::split_paths(&path).next().as_deref(),
        Some(Path::new("node-runtime/node-v24.18.1"))
    );
}

#[tokio::test]
async fn npm_install_exposes_managed_node_to_lifecycle_scripts() {
    let real_node = std::process::Command::new("node")
        .args(["-p", "process.execPath"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|path| PathBuf::from(path.trim()))
        .expect("node must be on PATH to test npm lifecycle PATH");

    let temp = tempdir().unwrap();
    let managed_bin = temp.path().join("bin");
    std::fs::create_dir_all(&managed_bin).unwrap();
    let managed_node = managed_bin.join("node");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real_node, &managed_node).unwrap();
    #[cfg(windows)]
    std::fs::copy(&real_node, managed_bin.join("node.exe")).unwrap();
    #[cfg(windows)]
    let managed_node = managed_bin.join("node.exe");

    let npm_script = temp.path().join("npm-cli.js");
    std::fs::write(
        &npm_script,
        r#"
const { spawnSync } = require('child_process');
const fs = require('fs');
fs.writeFileSync('path-out.txt', process.env.PATH || '');
const result = spawnSync('node', ['-e', 'process.exit(0)'], { encoding: 'utf8' });
if (result.error) {
  console.error(result.error.message);
  process.exit(127);
}
process.exit(result.status === null ? 127 : result.status);
"#,
    )
    .unwrap();

    let mut opts = make_opts(temp.path(), "0.1.0");
    opts.node_executable = managed_node;
    opts.npm_script = Some(npm_script);
    write_package_json(&opts).unwrap();
    run_npm_install(&opts).await.unwrap();

    let path_out = std::fs::read_to_string(opts.version_dir().join("path-out.txt")).unwrap();
    let first = std::env::split_paths(&path_out).next().unwrap();
    assert_eq!(first, managed_bin);
}
