/// Readiness output line prefix.
pub const READINESS_PREFIX: &str = "dsh web: ";

/// A validated loopback HTTP URL (origin plus any auth query dsh emits).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin(pub(super) String);

impl Origin {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Readiness parsing errors.
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

/// Parses one readiness output line.
pub fn parse_readiness_line(line: &str) -> Result<Option<Origin>, ReadinessError> {
    if !line.starts_with(READINESS_PREFIX) {
        return Ok(None);
    }

    let token = line[READINESS_PREFIX.len()..]
        .split_whitespace()
        .next()
        .ok_or_else(|| {
            ReadinessError::MalformedLine(format!("readiness line has no URL: {line}"))
        })?;
    Ok(Some(parse_token(token)?))
}

fn parse_token(token: &str) -> Result<Origin, ReadinessError> {
    let (scheme, rest) = token
        .split_once("://")
        .ok_or_else(|| ReadinessError::InvalidUrl(format!("missing scheme: {token}")))?;
    if scheme != "http" {
        return Err(ReadinessError::InvalidUrl(format!(
            "protocol must be http: {token}"
        )));
    }

    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let trailing = &rest[authority_end..];
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
    if !trailing.is_empty() && trailing != "/" && !trailing.starts_with("/?") {
        return Err(ReadinessError::InvalidUrl(format!(
            "readiness URL must have pathname '/' and no hash: {token}"
        )));
    }

    let query = match trailing {
        "" | "/" => "",
        _ => trailing, // starts with "/?"
    };
    Ok(Origin(format!("http://{host}:{port}{query}")))
}
