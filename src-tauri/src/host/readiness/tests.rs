use super::*;

#[tokio::test]
async fn parses_simple_readiness_line() {
    let parser = ReadinessParser::new();
    let result = parser
        .push("dsh web: http://127.0.0.1:51329/\n")
        .await
        .unwrap();
    assert_eq!(
        result.as_ref().map(Origin::as_str),
        Some("http://127.0.0.1:51329")
    );
    assert_eq!(
        parser.finalize().await.unwrap().as_str(),
        "http://127.0.0.1:51329"
    );
}

#[tokio::test]
async fn ignores_non_readiness_lines() {
    let parser = ReadinessParser::new();
    assert_eq!(parser.push("listening on port 0\n").await.unwrap(), None);
    let result = parser
        .push("warmup complete\ndsh web: http://localhost:3000/\n")
        .await
        .unwrap();
    assert_eq!(
        result.as_ref().map(Origin::as_str),
        Some("http://localhost:3000")
    );
}

#[tokio::test]
async fn handles_split_chunks() {
    let parser = ReadinessParser::new();
    assert_eq!(parser.push("dsh web: http://127.").await.unwrap(), None);
    assert_eq!(parser.push("0.0.1:42").await.unwrap(), None);
    assert_eq!(
        parser
            .push("/\n")
            .await
            .unwrap()
            .as_ref()
            .map(Origin::as_str),
        Some("http://127.0.0.1:42")
    );
}

#[tokio::test]
async fn rejects_invalid_readiness_urls() {
    for line in [
        "dsh web: http://example.com:8080/\n",
        "dsh web: https://127.0.0.1:8080/\n",
        "dsh web: http://127.0.0.1/\n",
        "dsh web: http://127.0.0.1:0/\n",
        "dsh web: http://127.0.0.1:8080/?x=1\n",
        "dsh web: http://127.0.0.1:8080/#frag\n",
    ] {
        let error = ReadinessParser::new().push(line).await.unwrap_err();
        assert!(matches!(error, ReadinessError::InvalidUrl(_)));
    }
}

#[tokio::test]
async fn rejects_conflicting_readiness_urls() {
    let parser = ReadinessParser::new();
    parser
        .push("dsh web: http://127.0.0.1:8080/\n")
        .await
        .unwrap();
    let error = parser
        .push("dsh web: http://127.0.0.1:9000/\n")
        .await
        .unwrap_err();
    assert!(matches!(error, ReadinessError::Conflicting { .. }));
}

#[tokio::test]
async fn duplicate_readiness_url_is_idempotent() {
    let parser = ReadinessParser::new();
    let first = parser
        .push("dsh web: http://127.0.0.1:8080/\n")
        .await
        .unwrap();
    let second = parser
        .push("dsh web: http://127.0.0.1:8080/\n")
        .await
        .unwrap();
    assert_eq!(first, second);
}

#[tokio::test]
async fn finalize_requires_readiness() {
    let parser = ReadinessParser::new();
    parser.push("just some log\n").await.unwrap();
    assert!(matches!(
        parser.finalize().await.unwrap_err(),
        ReadinessError::NoReadiness
    ));
}

#[tokio::test]
async fn finalize_parses_pending_tail() {
    let parser = ReadinessParser::new();
    parser
        .push("dsh web: http://127.0.0.1:8080/")
        .await
        .unwrap();
    assert_eq!(
        parser.finalize().await.unwrap().as_str(),
        "http://127.0.0.1:8080"
    );
}

#[tokio::test]
async fn strips_carriage_return() {
    let parser = ReadinessParser::new();
    let result = parser
        .push("dsh web: http://127.0.0.1:8080/\r\n")
        .await
        .unwrap();
    assert_eq!(
        result.as_ref().map(Origin::as_str),
        Some("http://127.0.0.1:8080")
    );
}
