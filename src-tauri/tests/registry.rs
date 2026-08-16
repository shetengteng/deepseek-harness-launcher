//! PR-012 集成测试：用 wiremock 构造 npm registry fixture，验证查询与缓存。
//!
//! 对应测试设计：
//! - 查询 `@deepseek-ai/dsh` 返回版本列表
//! - 缓存命中不发网络请求
//! - 缓存过期（TTL 5min）重新查询
//! - dist-tag `latest` 返回最新版本
//! - tarball URL 拼接正确
//! - 网络错误 `Err`，不污染缓存

use std::time::Duration;

use deepseek_harness_launcher_lib::dsh::{
    fetch_dist_tags, fetch_package_manifest, fetch_package_metadata, RegistryCache,
    DSH_PACKAGE_NAME,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// 构造一份完整 npm registry 响应 fixture。
fn registry_metadata_fixture() -> serde_json::Value {
    serde_json::json!({
        "name": DSH_PACKAGE_NAME,
        "dist-tags": {
            "latest": "0.1.0",
            "next": "0.1.1-rc.1"
        },
        "versions": {
            "0.0.9": {
                "version": "0.0.9",
                "engines": { "node": ">=20.0.0" },
                "dist": {
                    "integrity": "sha512-aaaaaa==",
                    "tarball": "https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-0.0.9.tgz"
                }
            },
            "0.1.0-rc.6": {
                "version": "0.1.0-rc.6",
                "engines": { "node": ">=22.0.0" },
                "dist": {
                    "integrity": "sha512-bbbbbb==",
                    "tarball": "https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-0.1.0-rc.6.tgz"
                }
            },
            "0.1.0": {
                "version": "0.1.0",
                "engines": { "node": ">=22.0.0" },
                "dist": {
                    "integrity": "sha512-cccccc==",
                    "tarball": "https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-0.1.0.tgz"
                }
            }
        }
    })
}

/// 单版本 manifest fixture（用于 `fetch_package_manifest_with_client` 测试）。
fn single_manifest_fixture(version: &str) -> serde_json::Value {
    serde_json::json!({
        "version": version,
        "engines": { "node": ">=22.0.0" },
        "dist": {
            "integrity": "sha512-cccccc==",
            "tarball": format!("https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-{version}.tgz")
        }
    })
}

/// 构造一个慢 client（10s 超时），测试中 registry URL 指向 wiremock。
fn test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("client")
}

/// 在 wiremock 上注册 `/(@deepseek-ai/dsh)` 的 200 响应。
/// 返回收到请求时的引用计数，验证缓存命中。
async fn mount_metadata_ok(server: &MockServer, body: serde_json::Value) -> wiremock::MockGuard {
    Mock::given(method("GET"))
        .and(path(format!("/{DSH_PACKAGE_NAME}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .up_to_n_times(1) // 只允许一次（验证缓存命中后不再请求）
        .mount_as_scoped(server)
        .await
}

#[tokio::test]
async fn fetch_metadata_returns_versions() {
    let server = MockServer::start().await;
    let _guard = mount_metadata_ok(&server, registry_metadata_fixture()).await;

    let cache = RegistryCache::new();
    let client = test_client();
    let data = fetch_package_metadata(&server.uri(), &cache, &client)
        .await
        .expect("fetch");

    assert_eq!(data.name, DSH_PACKAGE_NAME);
    assert_eq!(data.dist_tags.latest, "0.1.0");
    assert_eq!(data.versions.len(), 3);
    let mut versions = data.all_versions();
    versions.sort();
    assert_eq!(versions, vec!["0.0.9", "0.1.0", "0.1.0-rc.6"]);
}

#[tokio::test]
async fn fetch_dist_tags_returns_latest() {
    let server = MockServer::start().await;
    let _guard = mount_metadata_ok(&server, registry_metadata_fixture()).await;

    let cache = RegistryCache::new();
    let client = test_client();
    let tags = fetch_dist_tags(&server.uri(), &cache, &client)
        .await
        .expect("fetch_dist_tags");

    assert_eq!(tags.latest, "0.1.0");
    assert_eq!(
        tags.others.get("next").and_then(|v| v.as_str()),
        Some("0.1.1-rc.1")
    );
}

#[tokio::test]
async fn cache_hit_does_not_send_http_request() {
    let server = MockServer::start().await;
    let _guard = mount_metadata_ok(&server, registry_metadata_fixture()).await;

    let cache = RegistryCache::new();
    let client = test_client();
    let registry = server.uri();

    // 第一次：发请求 + 写缓存
    let _ = fetch_package_metadata(&registry, &cache, &client)
        .await
        .expect("first fetch");
    // 第二次：应该命中缓存，但 _guard 只允许 1 次请求，命中即不应再请求
    let second = fetch_package_metadata(&registry, &cache, &client)
        .await
        .expect("second fetch");
    assert_eq!(second.dist_tags.latest, "0.1.0");
}

#[tokio::test]
async fn cache_invalidate_forces_refresh() {
    let server = MockServer::start().await;
    // 允许 2 次请求
    Mock::given(method("GET"))
        .and(path(format!("/{DSH_PACKAGE_NAME}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(registry_metadata_fixture()))
        .expect(2)
        .mount(&server)
        .await;

    let cache = RegistryCache::new();
    let client = test_client();
    let registry = server.uri();

    let _ = fetch_package_metadata(&registry, &cache, &client)
        .await
        .expect("first");
    cache.invalidate().await;
    let _ = fetch_package_metadata(&registry, &cache, &client)
        .await
        .expect("second after invalidate");
}

#[tokio::test]
async fn fetch_manifest_uses_cached_metadata() {
    let server = MockServer::start().await;
    let _guard = mount_metadata_ok(&server, registry_metadata_fixture()).await;

    let cache = RegistryCache::new();
    let client = test_client();
    let registry = server.uri();

    // 第一次：取 manifest，会拉完整 metadata 写缓存
    let m1 = fetch_package_manifest(&registry, "0.1.0", &cache, &client)
        .await
        .expect("first manifest");
    assert_eq!(m1.version, "0.1.0");
    assert_eq!(m1.engines.node, ">=22.0.0");
    assert_eq!(m1.dist.integrity, "sha512-cccccc==");
    assert!(m1.dist.tarball.ends_with("/dsh-0.1.0.tgz"));

    // 第二次：取另一个版本，应该命中缓存，_guard 只允许 1 次请求
    let m2 = fetch_package_manifest(&registry, "0.1.0-rc.6", &cache, &client)
        .await
        .expect("second manifest");
    assert_eq!(m2.version, "0.1.0-rc.6");
    assert!(m2.dist.tarball.ends_with("/dsh-0.1.0-rc.6.tgz"));
}

#[tokio::test]
async fn fetch_manifest_unknown_version_errors() {
    let server = MockServer::start().await;
    let _guard = mount_metadata_ok(&server, registry_metadata_fixture()).await;

    let cache = RegistryCache::new();
    let client = test_client();
    let registry = server.uri();

    let err = fetch_package_manifest(&registry, "9.9.9", &cache, &client)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        deepseek_harness_launcher_lib::error::LauncherError::DshRegistry(_)
    ));
    assert!(err.to_string().contains("9.9.9"));
}

#[tokio::test]
async fn http_404_does_not_pollute_cache() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/{DSH_PACKAGE_NAME}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let cache = RegistryCache::new();
    let client = test_client();
    let registry = server.uri();

    let err = fetch_package_metadata(&registry, &cache, &client)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        deepseek_harness_launcher_lib::error::LauncherError::DshRegistry(_)
    ));
    assert!(err.to_string().contains("404"));

    // 关键：缓存应保持空，下次仍会发请求
    assert!(cache.is_empty().await);
}

#[tokio::test]
async fn http_500_does_not_pollute_cache() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/{DSH_PACKAGE_NAME}")))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let cache = RegistryCache::new();
    let client = test_client();
    let registry = server.uri();

    let err = fetch_package_metadata(&registry, &cache, &client)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("500"));
    assert!(cache.is_empty().await);
}

#[tokio::test]
async fn malformed_json_does_not_pollute_cache() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/{DSH_PACKAGE_NAME}")))
        .respond_with(ResponseTemplate::new(200).set_body_string("not valid json {"))
        .mount(&server)
        .await;

    let cache = RegistryCache::new();
    let client = test_client();
    let registry = server.uri();

    let err = fetch_package_metadata(&registry, &cache, &client)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        deepseek_harness_launcher_lib::error::LauncherError::DshRegistry(_)
    ));
    assert!(err.to_string().contains("parse"));
    assert!(cache.is_empty().await);
}

#[tokio::test]
async fn wrong_package_name_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/{DSH_PACKAGE_NAME}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "@wrong/pkg",
            "dist-tags": { "latest": "1.0.0" },
            "versions": {}
        })))
        .mount(&server)
        .await;

    let cache = RegistryCache::new();
    let client = test_client();
    let registry = server.uri();

    let err = fetch_package_metadata(&registry, &cache, &client)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("wrong package name"));
    // 失败也不写缓存
    assert!(cache.is_empty().await);
}

#[tokio::test]
async fn different_registers_use_independent_cache_entries() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    let mut fixture_a = registry_metadata_fixture();
    fixture_a["dist-tags"]["latest"] = serde_json::json!("0.1.0");

    let mut fixture_b = registry_metadata_fixture();
    fixture_b["dist-tags"]["latest"] = serde_json::json!("0.2.0");

    let _ga = mount_metadata_ok(&server_a, fixture_a).await;
    let _gb = mount_metadata_ok(&server_b, fixture_b).await;

    let cache = RegistryCache::new();
    let client = test_client();

    let a = fetch_package_metadata(&server_a.uri(), &cache, &client)
        .await
        .expect("a");
    let b = fetch_package_metadata(&server_b.uri(), &cache, &client)
        .await
        .expect("b");

    assert_eq!(a.dist_tags.latest, "0.1.0");
    assert_eq!(b.dist_tags.latest, "0.2.0");
    assert_eq!(cache.len().await, 2);
}

#[tokio::test]
async fn fetch_single_manifest_with_client_works() {
    use deepseek_harness_launcher_lib::dsh::fetch_package_manifest_with_client;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/{DSH_PACKAGE_NAME}/0.1.0")))
        .respond_with(ResponseTemplate::new(200).set_body_json(single_manifest_fixture("0.1.0")))
        .mount(&server)
        .await;

    let client = test_client();
    let m = fetch_package_manifest_with_client(&server.uri(), "0.1.0", &client)
        .await
        .expect("manifest");
    assert_eq!(m.version, "0.1.0");
    assert_eq!(m.engines.node, ">=22.0.0");
    assert!(m.dist.tarball.ends_with("/dsh-0.1.0.tgz"));
}

#[tokio::test]
async fn fetch_single_manifest_404_errors() {
    use deepseek_harness_launcher_lib::dsh::fetch_package_manifest_with_client;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/{DSH_PACKAGE_NAME}/9.9.9")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = test_client();
    let err = fetch_package_manifest_with_client(&server.uri(), "9.9.9", &client)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("404"));
}

#[tokio::test]
async fn fetch_single_manifest_version_mismatch_errors() {
    use deepseek_harness_launcher_lib::dsh::fetch_package_manifest_with_client;

    let server = MockServer::start().await;
    // 返回 0.1.0 的内容，但 URL 请求的是 9.9.9
    Mock::given(method("GET"))
        .and(path(format!("/{DSH_PACKAGE_NAME}/9.9.9")))
        .respond_with(ResponseTemplate::new(200).set_body_json(single_manifest_fixture("0.1.0")))
        .mount(&server)
        .await;

    let client = test_client();
    let err = fetch_package_manifest_with_client(&server.uri(), "9.9.9", &client)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("version"));
}
