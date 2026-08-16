//! PR-007 镜像源集成测试。对应测试设计 §PR-006。
//!
//! 用 `wiremock` 起 mock HTTP server，覆盖：
//! - `probe` 200 响应 → `Ok(())`
//! - `probe` 404 → `Err(ProbeFailed)`
//! - `probe` 5xx → `Err(ProbeFailed)`
//! - `probe` 超时（< 1s 配置）→ `Err(ProbeTimeout)`
//! - `probe_mirrors` 顺序回退：第一个 500、第二个 200 → 返回第二个
//! - `probe_mirrors` 全失败 → `AllMirrorsFailed` with tried 列表

use std::time::Duration;

use deepseek_harness_launcher_lib::node::{
    probe_mirror, probe_mirrors, validate_custom_mirror, Mirror, MirrorError, MirrorId,
};
use reqwest::Client;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn build_client() -> Client {
    Client::builder()
        .build()
        .expect("failed to build reqwest client")
}

fn custom_mirror(base_url: String) -> Mirror {
    Mirror {
        id: MirrorId::Custom(base_url.clone()),
        name: "test",
        base_url: Box::leak(base_url.into_boxed_str()),
        trusted: false,
    }
}

#[tokio::test]
async fn probe_returns_ok_on_200() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .mount(&server)
        .await;

    let m = custom_mirror(server.uri());
    let client = build_client();
    probe_mirror(&client, &m, Duration::from_secs(5))
        .await
        .expect("expected Ok on 200");
}

#[tokio::test]
async fn probe_fails_on_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let m = custom_mirror(server.uri());
    let client = build_client();
    let err = probe_mirror(&client, &m, Duration::from_secs(5))
        .await
        .unwrap_err();
    assert!(matches!(err, MirrorError::ProbeFailed { status: 404, .. }));
}

#[tokio::test]
async fn probe_fails_on_500() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let m = custom_mirror(server.uri());
    let client = build_client();
    let err = probe_mirror(&client, &m, Duration::from_secs(5))
        .await
        .unwrap_err();
    assert!(matches!(err, MirrorError::ProbeFailed { status: 500, .. }));
}

#[tokio::test]
async fn probe_times_out_when_server_slow() {
    let server = MockServer::start().await;
    // wiremock 的 delay 响应：100ms 延迟
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("[]")
                .set_delay(Duration::from_millis(500)),
        )
        .mount(&server)
        .await;

    let m = custom_mirror(server.uri());
    let client = build_client();
    // 超时 100ms，服务器延迟 500ms → 必然超时
    let err = probe_mirror(&client, &m, Duration::from_millis(100))
        .await
        .unwrap_err();
    assert!(
        matches!(err, MirrorError::ProbeTimeout { .. }),
        "expected ProbeTimeout, got: {err:?}"
    );
}

#[tokio::test]
async fn probe_network_error_on_connection_refused() {
    // 用一个几乎不可能监听的端口
    let m = custom_mirror("https://127.0.0.1:1".to_string());
    let client = build_client();
    let err = probe_mirror(&client, &m, Duration::from_secs(5))
        .await
        .unwrap_err();
    assert!(
        matches!(err, MirrorError::ProbeNetwork { .. }),
        "expected ProbeNetwork, got: {err:?}"
    );
}

#[tokio::test]
async fn probe_mirrors_returns_first_ok() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .mount(&server)
        .await;

    let m = custom_mirror(server.uri());
    let mirrors = [m.clone()];
    let client = build_client();
    let result = probe_mirrors(&client, &mirrors, Duration::from_secs(5))
        .await
        .expect("expected Ok");
    assert_eq!(result.id, m.id);
}

#[tokio::test]
async fn probe_mirrors_falls_back_to_second_when_first_fails() {
    let bad_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&bad_server)
        .await;

    let good_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .mount(&good_server)
        .await;

    let mirrors = vec![
        custom_mirror(bad_server.uri()),
        custom_mirror(good_server.uri()),
    ];
    let client = build_client();
    let result = probe_mirrors(&client, &mirrors, Duration::from_secs(5))
        .await
        .expect("expected Ok from second mirror");
    assert_eq!(result.id, mirrors[1].id);
}

#[tokio::test]
async fn probe_mirrors_returns_all_failed_when_all_fail() {
    let s1 = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&s1)
        .await;
    let s2 = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s2)
        .await;

    let mirrors = vec![custom_mirror(s1.uri()), custom_mirror(s2.uri())];
    let client = build_client();
    let err = probe_mirrors(&client, &mirrors, Duration::from_secs(5))
        .await
        .unwrap_err();
    match err {
        MirrorError::AllMirrorsFailed { tried } => {
            assert_eq!(tried.len(), 2);
        }
        other => panic!("expected AllMirrorsFailed, got {other:?}"),
    }
}

#[test]
fn validate_custom_mirror_accepts_in_integration_context() {
    // 与单元测试重复但确保 lib crate 导出正确
    let m = validate_custom_mirror("https://my-mirror.example.com/node").unwrap();
    assert!(!m.trusted);
    assert!(matches!(m.id, MirrorId::Custom(_)));
}
