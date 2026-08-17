use std::sync::Arc;

use tokio::sync::Mutex;

use super::{parse_readiness_line, Origin, ReadinessError};

/// Incrementally parses host stdout for its readiness origin.
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

    pub async fn push(&self, chunk: &str) -> Result<Option<Origin>, ReadinessError> {
        let mut inner = self.inner.lock().await;
        inner.pending.push_str(chunk);

        loop {
            let Some(index) = inner.pending.find('\n') else {
                return Ok(inner.ready.as_ref().map(|origin| Origin(origin.clone())));
            };
            let line = inner.pending[..index].to_owned();
            inner.pending = inner.pending[index + 1..].to_owned();
            let line = line.strip_suffix('\r').unwrap_or(&line);
            let Some(parsed) = parse_readiness_line(line)? else {
                continue;
            };
            return record_origin(&mut inner, parsed);
        }
    }

    pub async fn finalize(&self) -> Result<Origin, ReadinessError> {
        let mut inner = self.inner.lock().await;
        if !inner.pending.is_empty() {
            let line = inner.pending.trim_end_matches('\r').to_owned();
            if !line.is_empty() {
                if let Some(parsed) = parse_readiness_line(&line)? {
                    record_origin(&mut inner, parsed)?;
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

fn record_origin(
    inner: &mut ParserInner,
    parsed: Origin,
) -> Result<Option<Origin>, ReadinessError> {
    let parsed_str = parsed.as_str().to_owned();
    match &inner.ready {
        Some(existing) if *existing != parsed_str => Err(ReadinessError::Conflicting {
            first: existing.clone(),
            second: parsed_str,
        }),
        _ => {
            inner.ready = Some(parsed_str.clone());
            Ok(Some(Origin(parsed_str)))
        }
    }
}
