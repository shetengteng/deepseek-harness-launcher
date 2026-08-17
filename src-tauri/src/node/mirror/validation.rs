use super::{Mirror, MirrorError, MirrorId};

/// 校验自定义镜像源 URL。
pub fn validate_custom_mirror(raw: &str) -> Result<Mirror, MirrorError> {
    if !raw.starts_with("https://") {
        return Err(MirrorError::NotHttps(raw.to_string()));
    }
    let url = url::Url::parse(raw).map_err(|error| MirrorError::InvalidUrl(error.to_string()))?;
    if url.scheme() != "https" {
        return Err(MirrorError::NotHttps(raw.to_string()));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(MirrorError::InvalidUrl(format!(
            "custom mirror URL must not contain query or fragment: {raw}"
        )));
    }

    let base_url = raw.trim_end_matches('/').to_string();
    Ok(Mirror {
        id: MirrorId::Custom(base_url.clone()),
        name: "自定义",
        base_url: Box::leak(base_url.into_boxed_str()),
        trusted: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_https_custom_mirrors() {
        assert!(matches!(
            validate_custom_mirror("http://example.com"),
            Err(MirrorError::NotHttps(_))
        ));
        assert!(matches!(
            validate_custom_mirror("example.com"),
            Err(MirrorError::NotHttps(_))
        ));
    }

    #[test]
    fn rejects_ambiguous_custom_mirror_urls() {
        assert!(matches!(
            validate_custom_mirror("https://x.com/?foo=bar"),
            Err(MirrorError::InvalidUrl(_))
        ));
        assert!(matches!(
            validate_custom_mirror("https://x.com/#fragment"),
            Err(MirrorError::InvalidUrl(_))
        ));
    }

    #[test]
    fn accepts_and_normalizes_https_custom_mirrors() {
        let mirror = validate_custom_mirror("https://my-mirror.com/node/").unwrap();
        assert!(!mirror.trusted);
        assert_eq!(mirror.base_url, "https://my-mirror.com/node");
        assert!(matches!(mirror.id, MirrorId::Custom(_)));
    }
}
