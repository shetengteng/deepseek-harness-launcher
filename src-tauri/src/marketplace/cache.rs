use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::error::{LauncherError, Result};

use super::types::CatalogCache;

pub const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const CACHE_FILE: &str = "catalog-cache-v1.json";

pub fn cache_path() -> Result<PathBuf> {
    Ok(crate::paths::data_dir()?
        .join("marketplace")
        .join(CACHE_FILE))
}

pub fn load() -> Result<Option<CatalogCache>> {
    load_from(&cache_path()?)
}

pub fn load_from(path: &Path) -> Result<Option<CatalogCache>> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|error| {
            LauncherError::Marketplace(format!("catalog cache is invalid: {error}"))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(LauncherError::Io(error)),
    }
}

pub fn save(cache: &CatalogCache) -> Result<()> {
    save_to(&cache_path()?, cache)
}

pub fn save_to(path: &Path, cache: &CatalogCache) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        LauncherError::Marketplace(format!("catalog cache has no parent: {}", path.display()))
    })?;
    std::fs::create_dir_all(parent)?;
    let temporary = path.with_file_name(format!(".{}.tmp", CACHE_FILE));
    std::fs::write(&temporary, serde_json::to_vec_pretty(cache)?)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

pub fn is_stale(fetched_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now.signed_duration_since(fetched_at)
        .to_std()
        .map_or(true, |age| age > CACHE_TTL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::types::{CatalogPlugin, InstallSpec, Popularity};

    fn cache() -> CatalogCache {
        CatalogCache {
            provider_id: "test".to_string(),
            etag: Some("etag".to_string()),
            fetched_at: Utc::now(),
            response_version: Some("v1".to_string()),
            plugins: vec![CatalogPlugin {
                id: "owner/repo".to_string(),
                name: "plugin".to_string(),
                repository_url: "https://github.com/owner/repo".to_string(),
                install_spec: InstallSpec {
                    owner: "owner".to_string(),
                    repository: "repo".to_string(),
                    subdirectory: None,
                    reference: None,
                },
                description: "test".to_string(),
                category: None,
                tags: vec![],
                source_updated_at: None,
                validated_at: None,
                popularity: Popularity {
                    marketplace_rank: None,
                    ranking_updated_at: None,
                    github_stars: None,
                    stars_fetched_at: None,
                },
            }],
        }
    }

    #[test]
    fn cache_round_trips_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("catalog-cache-v1.json");
        save_to(&path, &cache()).unwrap();
        assert_eq!(load_from(&path).unwrap().unwrap().provider_id, "test");
        assert!(!temp.path().join(".catalog-cache-v1.json.tmp").exists());
    }

    #[test]
    fn stale_after_one_day() {
        let now = Utc::now();
        assert!(!is_stale(now - chrono::Duration::hours(23), now));
        assert!(is_stale(now - chrono::Duration::hours(25), now));
    }
}
