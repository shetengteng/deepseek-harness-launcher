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
async fn install_dsh_cleans_the_version_directory_after_spawn_failure() {
    let temp = tempdir().unwrap();
    let mut opts = make_opts(temp.path(), "0.1.0");
    opts.node_executable = PathBuf::from("/nonexistent/node");
    opts.npm_script = Some(PathBuf::from("/nonexistent/npm-cli.js"));

    let error = install_dsh(&opts).await.unwrap_err();
    assert!(error.to_string().contains("install failed"));
    assert!(!opts.version_dir().exists());
}

#[test]
fn default_log_accepts_output() {
    default_log()("test line");
}
