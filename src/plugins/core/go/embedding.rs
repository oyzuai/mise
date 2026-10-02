//! OEP-0003 official Go release catalog adapter. Selection reuses mise rules;
//! this module owns bounded JSON decoding and the exact metadata-byte identity.
use super::GoPlugin;
use crate::plugins::core::embedding_selection;
use eyre::{Result, ensure};
use itertools::Itertools;
use std::collections::BTreeSet;
use versions::Versioning;

/// Selected canonical version plus exact supplied catalog identity. Neither field
/// proves publisher authenticity, target availability or acquisition authority.
#[derive(Debug, serde::Serialize)]
pub struct GoVersionResolution {
    pub version: String,
    pub catalog_sha256: String,
}

#[derive(serde::Deserialize)]
struct Release {
    version: String,
    stable: bool,
}

impl GoPlugin {
    pub(in crate::plugins::core) async fn embedding_resolve_version(
        &self,
        request: &str,
        constraints: &[String],
    ) -> Result<GoVersionResolution> {
        let selectors = embedding_selection::selectors(self, request, constraints)?;
        let catalog = catalog().await?;
        Ok(GoVersionResolution {
            version: embedding_selection::select(self, catalog.versions.clone(), selectors)?,
            catalog_sha256: catalog.digest.clone(),
        })
    }
}

struct Catalog {
    versions: Vec<String>,
    digest: String,
}

async fn catalog() -> Result<&'static Catalog> {
    // The embedding context is once-only per process. Cache only a successfully
    // parsed immutable snapshot; never decode lossy text before hashing bytes.
    static CATALOG: tokio::sync::OnceCell<Catalog> = tokio::sync::OnceCell::const_new();
    CATALOG
        .get_or_try_init(|| async {
            let raw = crate::http::HTTP
                .get_bytes("https://go.dev/dl/?mode=json&include=all")
                .await?;
            ensure!(
                raw.as_ref().len() <= 16 * 1024 * 1024,
                "Go catalog exceeds byte limit"
            );
            let raw = std::str::from_utf8(raw.as_ref())?;
            let releases: Vec<Release> = serde_json::from_str(raw)?;
            ensure!(
                releases.len() <= 100_000,
                "Go catalog exceeds release limit"
            );
            let mut seen = BTreeSet::new();
            let mut versions = Vec::new();
            for release in releases {
                ensure!(
                    release.version.len() <= 128,
                    "Go catalog version exceeds limit"
                );
                ensure!(
                    seen.insert(release.version.clone()),
                    "duplicate Go catalog release"
                );
                if !release.stable {
                    continue;
                }
                let version = release
                    .version
                    .strip_prefix("go")
                    .ok_or_else(|| eyre::eyre!("invalid Go catalog version prefix"))?;
                // Older Go releases lack a patch component. The initial archive
                // contract excludes them rather than inventing a '.0' artifact.
                if !semver::Version::parse(version).is_ok_and(|v| {
                    v.pre.is_empty() && v.build.is_empty() && v.to_string() == version
                }) {
                    continue;
                }
                versions.push(version.to_owned());
            }
            let versions = versions
                .into_iter()
                .sorted_by_cached_key(|v| (Versioning::new(v), v.to_string()))
                .collect();
            Ok(Catalog {
                versions,
                digest: format!("sha256:{}", crate::hash::hash_sha256_to_str(raw)),
            })
        })
        .await
}
