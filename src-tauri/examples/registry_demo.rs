//! PR-012 本地演示：起一个 mock npm registry，验证 registry 查询 + 缓存。
//!
//! 运行方式：
//! ```bash
//! cd src-tauri
//! cargo run --example registry_demo
//! ```
//!
//! 不会真的联网。在内置 mock 服务器上模拟 registry.npmmirror.com 行为。

use std::time::Instant;

use deepseek_harness_launcher_lib::dsh::{
    fetch_dist_tags, fetch_package_manifest, fetch_package_manifest_with_client,
    fetch_package_metadata, RegistryCache, DSH_PACKAGE_NAME, REGISTRY_CACHE_TTL,
};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::main]
async fn main() {
    println!("=== PR-012 dsh/registry 本地演示 ===\n");

    // 1. 启动 mock npm registry
    let server = MockServer::start().await;
    let registry = server.uri();
    println!("[mock] npm registry 已启动: {registry}");
    println!("[mock] 包名: {DSH_PACKAGE_NAME}");
    println!("[mock] 缓存 TTL: {:?}", REGISTRY_CACHE_TTL);
    println!();

    // 2. 挂载完整包元数据响应（3 个版本）
    let fixture = json!({
        "name": DSH_PACKAGE_NAME,
        "dist-tags": {
            "latest": "0.1.0",
            "next":  "0.1.1-rc.1"
        },
        "versions": {
            "0.0.9": {
                "version": "0.0.9",
                "engines": { "node": ">=20.0.0" },
                "dist": {
                    "integrity": "sha512-aaaaaa==",
                    "tarball":   "https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-0.0.9.tgz"
                }
            },
            "0.1.0-rc.6": {
                "version": "0.1.0-rc.6",
                "engines": { "node": ">=22.0.0" },
                "dist": {
                    "integrity": "sha512-bbbbbb==",
                    "tarball":   "https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-0.1.0-rc.6.tgz"
                }
            },
            "0.1.0": {
                "version": "0.1.0",
                "engines": { "node": ">=22.0.0" },
                "dist": {
                    "integrity": "sha512-cccccc==",
                    "tarball":   "https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-0.1.0.tgz"
                }
            }
        }
    });

    // 只允许命中一次，验证缓存效果
    Mock::given(method("GET"))
        .and(path(format!("/{DSH_PACKAGE_NAME}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture))
        .expect(1) // 关键：期望只调用 1 次
        .mount(&server)
        .await;

    let cache = RegistryCache::new();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();

    // 3. 第一次：fetch_dist_tags，发 HTTP 请求
    println!(">>> 步骤 1: 调用 fetch_dist_tags（首次，发 HTTP）");
    let t0 = Instant::now();
    let tags = fetch_dist_tags(&registry, &cache, &client)
        .await
        .expect("fetch_dist_tags");
    let dt1 = t0.elapsed();
    println!("    latest = {}", tags.latest);
    println!("    next   = {}", tags.others.get("next").unwrap());
    println!("    耗时   = {:.2?}", dt1);
    println!();

    // 4. 第二次：fetch_package_metadata，命中缓存（不再请求）
    println!(">>> 步骤 2: 调用 fetch_package_metadata（应命中缓存）");
    let t1 = Instant::now();
    let metadata = fetch_package_metadata(&registry, &cache, &client)
        .await
        .expect("fetch_package_metadata");
    let dt2 = t1.elapsed();
    println!("    包名     = {}", metadata.name);
    println!("    版本列表 = {:?}", metadata.all_versions());
    println!("    耗时     = {:.2?}（应远小于步骤 1）", dt2);
    println!();

    // 5. 第三次：fetch_package_manifest "0.1.0"，命中缓存
    println!(">>> 步骤 3: 调用 fetch_package_manifest(\"0.1.0\")（应命中缓存）");
    let m = fetch_package_manifest(&registry, "0.1.0", &cache, &client)
        .await
        .expect("manifest");
    println!("    version       = {}", m.version);
    println!("    engines.node  = {}", m.engines.node);
    println!("    integrity     = {}", m.dist.integrity);
    println!("    tarball       = {}", m.dist.tarball);
    println!();

    // 6. 错误场景：请求不存在的版本
    println!(">>> 步骤 4: 请求不存在的版本 9.9.9（应返回 DshRegistry 错误，不污染缓存）");
    let err = fetch_package_manifest(&registry, "9.9.9", &cache, &client)
        .await
        .unwrap_err();
    println!("    错误: {err}");
    println!();

    // 7. 单版本端点：mock `/@deepseek-ai/dsh/0.1.0-rc.6`
    Mock::given(method("GET"))
        .and(path(format!("/{DSH_PACKAGE_NAME}/0.1.0-rc.6")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "version": "0.1.0-rc.6",
            "engines": { "node": ">=22.0.0" },
            "dist": {
                "integrity": "sha512-bbbbbb==",
                "tarball":   "https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-0.1.0-rc.6.tgz"
            }
        })))
        .mount(&server)
        .await;

    println!(
        ">>> 步骤 5: 调用 fetch_package_manifest_with_client(\"0.1.0-rc.6\")（直连单版本端点）"
    );
    let m2 = fetch_package_manifest_with_client(&registry, "0.1.0-rc.6", &client)
        .await
        .expect("single manifest");
    println!("    version       = {}", m2.version);
    println!("    engines.node  = {}", m2.engines.node);
    println!("    tarball       = {}", m2.dist.tarball);
    println!();

    // 8. 失败场景：404 不污染缓存
    Mock::given(method("GET"))
        .and(path(format!("/{DSH_PACKAGE_NAME}/9.9.9")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    println!(">>> 步骤 6: 调用 fetch_package_manifest_with_client(\"9.9.9\")（404，应报错）");
    let err = fetch_package_manifest_with_client(&registry, "9.9.9", &client)
        .await
        .unwrap_err();
    println!("    错误: {err}");
    println!();

    println!("=== 演示完成。验证点：");
    println!("  1. 步骤 1 耗时 > 步骤 2 耗时（缓存命中）");
    println!("  2. mock 期望请求数 = 1（验证缓存命中后不再发请求）");
    println!("  3. 9.9.9 错误不写入缓存");
    println!("  4. wiremock 退出时会校验 expect(1) 是否满足");
}
