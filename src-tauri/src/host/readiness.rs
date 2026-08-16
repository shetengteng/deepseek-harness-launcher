//! Host 就绪行解析。
//!
//! 对应 [host-supervisor.ts](../../../deepseek-harness-desktop/apps/desktop/src/host-supervisor.ts)
//! 的 `createReadinessParser` / `parseReadinessLine`。
//!
//! Web Host 启动时会向 stdout 打印形如 `dsh web: http://127.0.0.1:51329/` 的就绪行。
//! `ReadinessParser` 增量消费 stdout chunk，遇到完整就绪行返回 origin。
//! 同一 Host 不允许输出冲突的就绪 URL。

use std::sync::Arc;

use tokio::sync::Mutex;

/// 就绪行前缀。Web Host 必须以此开头声明自身 URL。
pub const READINESS_PREFIX: &str = "dsh web: ";

/// 启动阶段 stdout 缓冲上限，与 host-supervisor.ts 对齐。
/// 超过则从头部截断（保留尾部最新的日志，对诊断更有价值）。
pub const MAX_STARTUP_OUTPUT_CHARS: usize = 32_768;

/// 解析后的 origin，形如 `http://127.0.0.1:51329`。
///
/// 通过 `parse_readiness_line` 校验后构造，内部字段保证：
/// - `scheme == "http:"`
/// - `host` 为 `127.0.0.1` 或 `localhost`
/// - `port` 为 1..=65535
/// - 无 pathname / search / hash
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin(String);

impl Origin {
    /// 返回 origin 字符串（如 `http://127.0.0.1:51329`）。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 解析单行就绪输出。
/// - 非 `dsh web: ` 前缀 → `None`（视为普通日志行）。
/// - 前缀匹配但 URL 非法、协议/主机/端口/path 不符合约束 → `Err`。
pub fn parse_readiness_line(line: &str) -> Result<Option<Origin>, ReadinessError> {
    if !line.starts_with(READINESS_PREFIX) {
        return Ok(None);
    }
    let rest = &line[READINESS_PREFIX.len()..];
    // 只取第一个空白分隔的 token
    let token = rest.split_whitespace().next().ok_or_else(|| {
        ReadinessError::MalformedLine(format!("readiness line has no URL: {line}"))
    })?;

    Ok(Some(parse_token(token)?))
}

fn parse_token(token: &str) -> Result<Origin, ReadinessError> {
    // 手写解析：URL crate 在闭包里要 alloc，自己拼比引入额外依赖更轻。
    // 期望格式：`http://127.0.0.1:<port>/` 或 `http://localhost:<port>/`
    let (scheme, rest) = token
        .split_once("://")
        .ok_or_else(|| ReadinessError::InvalidUrl(format!("missing scheme: {token}")))?;
    if scheme != "http" {
        return Err(ReadinessError::InvalidUrl(format!(
            "protocol must be http: {token}"
        )));
    }
    // 找到下一个 `/`、`?`、`#` 作为路径起始
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let trailing = &rest[authority_end..];

    // 解析 host:port
    let (host, port_str) = authority
        .rsplit_once(':')
        .ok_or_else(|| ReadinessError::InvalidUrl(format!("missing port: {token}")))?;
    if host.is_empty() {
        return Err(ReadinessError::InvalidUrl(format!("empty host: {token}")));
    }
    if host != "127.0.0.1" && host != "localhost" {
        return Err(ReadinessError::InvalidUrl(format!(
            "host must be loopback: {token}"
        )));
    }
    let port: u16 = port_str
        .parse()
        .map_err(|_| ReadinessError::InvalidUrl(format!("invalid port: {token}")))?;
    if port == 0 {
        return Err(ReadinessError::InvalidUrl(format!(
            "port must be explicit (1..=65535): {token}"
        )));
    }
    // trailing 必须为空或恰好 `/`，且无 query/hash
    if !trailing.is_empty() && trailing != "/" {
        return Err(ReadinessError::InvalidUrl(format!(
            "readiness URL must have pathname '/' and no query/hash: {token}"
        )));
    }

    Ok(Origin(format!("http://{host}:{port}")))
}

/// `ReadinessParser` 的错误。对齐 host-supervisor.ts 抛出的 `Error`。
#[derive(Debug, thiserror::Error)]
pub enum ReadinessError {
    #[error("malformed readiness line: {0}")]
    MalformedLine(String),
    #[error("invalid readiness URL: {0}")]
    InvalidUrl(String),
    #[error("conflicting readiness URLs: {first} and {second}")]
    Conflicting { first: String, second: String },
    #[error("Host exited before emitting its readiness URL")]
    NoReadiness,
}

/// 增量就绪解析器。线程安全（`Arc<Mutex<Inner>>`），可在多个 stdout 监听任务间共享。
///
/// 与 host-supervisor.ts 的 `createReadinessParser` 行为一致：
/// - `push(chunk)`：消费一段 stdout，遇到完整就绪行返回 origin（之后仍可继续校验冲突）。
/// - `finalize()`：流结束时调用，要求已观察到就绪行，否则 `NoReadiness`。
/// - 重复就绪行必须与首次相同，否则 `Conflicting`。
#[derive(Clone)]
pub struct ReadinessParser {
    inner: Arc<Mutex<ParserInner>>,
}

#[derive(Default)]
struct ParserInner {
    pending: String,
    ready: Option<String>,
}

impl ReadinessParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// 消费一段 stdout。如果在此期间观察到完整就绪行，返回 `Ok(Some(origin))`；
    /// 后续调用仍然继续校验冲突 URL，发现冲突返回 `Err`。
    pub async fn push(&self, chunk: &str) -> Result<Option<Origin>, ReadinessError> {
        let mut inner = self.inner.lock().await;
        inner.pending.push_str(chunk);
        loop {
            let Some(idx) = inner.pending.find('\n') else {
                return Ok(inner.ready.as_ref().map(|s| Origin(s.clone())));
            };
            // 切出第一行（不含换行符）
            let line = inner.pending[..idx].to_string();
            // 剩余部分写回 pending
            inner.pending = inner.pending[idx + 1..].to_string();
            let line = line.strip_suffix('\r').unwrap_or(&line).to_string();
            let Some(parsed) = parse_readiness_line(&line)? else {
                // 普通日志行：不影响就绪状态
                continue;
            };
            let parsed_str = parsed.as_str().to_string();
            match &inner.ready {
                Some(existing) if *existing != parsed_str => {
                    return Err(ReadinessError::Conflicting {
                        first: existing.clone(),
                        second: parsed_str,
                    });
                }
                _ => {
                    inner.ready = Some(parsed_str.clone());
                    return Ok(Some(Origin(parsed_str)));
                }
            }
        }
    }

    /// 流结束：如果缓冲区还有未换行的尾巴，尝试当作最后一行解析。
    /// 要求至少观察到一次就绪行。
    pub async fn finalize(&self) -> Result<Origin, ReadinessError> {
        let mut inner = self.inner.lock().await;
        if !inner.pending.is_empty() {
            let line = inner.pending.trim_end_matches('\r').to_string();
            if !line.is_empty() {
                if let Some(parsed) = parse_readiness_line(&line)? {
                    let parsed_str = parsed.as_str().to_string();
                    match &inner.ready {
                        Some(existing) if *existing != parsed_str => {
                            return Err(ReadinessError::Conflicting {
                                first: existing.clone(),
                                second: parsed_str,
                            });
                        }
                        _ => inner.ready = Some(parsed_str),
                    }
                }
            }
        }
        inner
            .ready
            .clone()
            .map(Origin)
            .ok_or(ReadinessError::NoReadiness)
    }
}

impl Default for ReadinessParser {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ParserInner::default())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parses_simple_readiness_line() {
        let p = ReadinessParser::new();
        let r = p.push("dsh web: http://127.0.0.1:51329/\n").await.unwrap();
        assert_eq!(
            r.as_ref().map(|o| o.as_str()),
            Some("http://127.0.0.1:51329")
        );
        let origin = p.finalize().await.unwrap();
        assert_eq!(origin.as_str(), "http://127.0.0.1:51329");
    }

    #[tokio::test]
    async fn ignores_non_readiness_lines() {
        let p = ReadinessParser::new();
        let r = p.push("listening on port 0\n").await.unwrap();
        assert_eq!(r, None);
        let r = p
            .push("warmup complete\ndsh web: http://localhost:3000/\n")
            .await
            .unwrap();
        assert_eq!(
            r.as_ref().map(|o| o.as_str()),
            Some("http://localhost:3000")
        );
    }

    #[tokio::test]
    async fn handles_split_chunks() {
        let p = ReadinessParser::new();
        let r = p.push("dsh web: http://127.").await.unwrap();
        assert_eq!(r, None);
        let r = p.push("0.0.1:42").await.unwrap();
        assert_eq!(r, None);
        let r = p.push("/\n").await.unwrap();
        assert_eq!(r.as_ref().map(|o| o.as_str()), Some("http://127.0.0.1:42"));
    }

    #[tokio::test]
    async fn rejects_non_loopback_host() {
        let p = ReadinessParser::new();
        let err = p
            .push("dsh web: http://example.com:8080/\n")
            .await
            .unwrap_err();
        assert!(matches!(err, ReadinessError::InvalidUrl(_)));
    }

    #[tokio::test]
    async fn rejects_https_scheme() {
        let p = ReadinessParser::new();
        let err = p
            .push("dsh web: https://127.0.0.1:8080/\n")
            .await
            .unwrap_err();
        assert!(matches!(err, ReadinessError::InvalidUrl(_)));
    }

    #[tokio::test]
    async fn rejects_missing_port() {
        let p = ReadinessParser::new();
        let err = p.push("dsh web: http://127.0.0.1/\n").await.unwrap_err();
        assert!(matches!(err, ReadinessError::InvalidUrl(_)));
    }

    #[tokio::test]
    async fn rejects_port_zero() {
        let p = ReadinessParser::new();
        let err = p.push("dsh web: http://127.0.0.1:0/\n").await.unwrap_err();
        assert!(matches!(err, ReadinessError::InvalidUrl(_)));
    }

    #[tokio::test]
    async fn rejects_query_string() {
        let p = ReadinessParser::new();
        let err = p
            .push("dsh web: http://127.0.0.1:8080/?x=1\n")
            .await
            .unwrap_err();
        assert!(matches!(err, ReadinessError::InvalidUrl(_)));
    }

    #[tokio::test]
    async fn rejects_hash() {
        let p = ReadinessParser::new();
        let err = p
            .push("dsh web: http://127.0.0.1:8080/#frag\n")
            .await
            .unwrap_err();
        assert!(matches!(err, ReadinessError::InvalidUrl(_)));
    }

    #[tokio::test]
    async fn rejects_conflicting_readiness_urls() {
        let p = ReadinessParser::new();
        p.push("dsh web: http://127.0.0.1:8080/\n").await.unwrap();
        let err = p
            .push("dsh web: http://127.0.0.1:9000/\n")
            .await
            .unwrap_err();
        assert!(matches!(err, ReadinessError::Conflicting { .. }));
    }

    #[tokio::test]
    async fn same_readiness_url_twice_is_idempotent() {
        let p = ReadinessParser::new();
        let r1 = p.push("dsh web: http://127.0.0.1:8080/\n").await.unwrap();
        let r2 = p.push("dsh web: http://127.0.0.1:8080/\n").await.unwrap();
        assert_eq!(r1, r2);
    }

    #[tokio::test]
    async fn finalize_without_readiness_is_error() {
        let p = ReadinessParser::new();
        p.push("just some log\n").await.unwrap();
        let err = p.finalize().await.unwrap_err();
        assert!(matches!(err, ReadinessError::NoReadiness));
    }

    #[tokio::test]
    async fn finalize_parses_pending_tail() {
        let p = ReadinessParser::new();
        // 没有 trailing newline，push 不会触发解析
        p.push("dsh web: http://127.0.0.1:8080/").await.unwrap();
        let origin = p.finalize().await.unwrap();
        assert_eq!(origin.as_str(), "http://127.0.0.1:8080");
    }

    #[tokio::test]
    async fn strips_carriage_return() {
        let p = ReadinessParser::new();
        let r = p.push("dsh web: http://127.0.0.1:8080/\r\n").await.unwrap();
        assert_eq!(
            r.as_ref().map(|o| o.as_str()),
            Some("http://127.0.0.1:8080")
        );
    }
}
