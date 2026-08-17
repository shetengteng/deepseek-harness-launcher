use std::time::Duration;

use reqwest::Client;

use super::{Mirror, MirrorError};

/// 探活单个镜像源：`GET {base_url}/index.json`，期望 200。
pub async fn probe_mirror(
    client: &Client,
    mirror: &Mirror,
    timeout: Duration,
) -> Result<(), MirrorError> {
    let url = mirror.index_url();
    tracing::debug!(mirror = %mirror.id, %url, "probing mirror");
    let response = client
        .get(&url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                MirrorError::ProbeTimeout {
                    mirror: mirror.id.to_string(),
                    timeout,
                }
            } else {
                MirrorError::ProbeNetwork {
                    mirror: mirror.id.to_string(),
                    cause: error.to_string(),
                }
            }
        })?;
    if response.status() == reqwest::StatusCode::OK {
        tracing::debug!(mirror = %mirror.id, "probe ok");
        return Ok(());
    }
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    Err(MirrorError::ProbeFailed {
        mirror: mirror.id.to_string(),
        status,
        body: body.chars().take(200).collect(),
    })
}

/// 依次探活镜像源列表，返回首个 200 的。
pub async fn probe_mirrors(
    client: &Client,
    mirrors: &[Mirror],
    timeout: Duration,
) -> Result<Mirror, MirrorError> {
    let mut tried = Vec::new();
    for mirror in mirrors {
        let id = mirror.id.to_string();
        match probe_mirror(client, mirror, timeout).await {
            Ok(()) => return Ok(mirror.clone()),
            Err(error) => {
                tracing::warn!(mirror = %id, %error, "probe failed, trying next");
                tried.push(id);
            }
        }
    }
    Err(MirrorError::AllMirrorsFailed { tried })
}
