//! OEP-0003 official Go release catalog adapter. Selection reuses mise rules;
//! this module owns bounded JSON decoding and the exact metadata-byte identity.
use super::{GoArchiveMetadata, GoPlugin};
use crate::plugins::core::embedding_selection;
use eyre::{Result, ensure};
use itertools::Itertools;
use std::collections::{BTreeMap, BTreeSet};
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
    #[serde(default)]
    files: Vec<ReleaseFile>,
}

#[derive(serde::Deserialize)]
struct ReleaseFile {
    filename: String,
    os: String,
    arch: String,
    version: String,
    sha256: String,
    size: u64,
    kind: String,
}

impl GoPlugin {
    pub(in crate::plugins::core) async fn embedding_archive_metadata(
        &self,
        version: &str,
        target: &crate::backend::platform_target::PlatformTarget,
        target_key: &str,
    ) -> Result<GoArchiveMetadata> {
        let archive = self.embedding_archive_facts(version, target, target_key);
        let catalog = catalog().await?;
        let files = catalog
            .files
            .get(version)
            .ok_or_else(|| eyre::eyre!("Go version is absent from stable release catalog"))?;
        let filename = archive
            .archive_url
            .rsplit('/')
            .next()
            .ok_or_else(|| eyre::eyre!("Go archive URL has no filename"))?;
        let file = files
            .iter()
            .find(|file| file.filename == filename)
            .ok_or_else(|| eyre::eyre!("Go target archive is absent from release catalog"))?;
        let (os, arch, _) = target_key
            .split('/')
            .collect_tuple()
            .ok_or_else(|| eyre::eyre!("invalid Go target tuple"))?;
        ensure!(
            file.os == os
                && file.arch == arch
                && file.version == format!("go{version}")
                && file.kind == "archive"
                && file.size > 0,
            "Go catalog file contradicts target archive identity"
        );
        ensure!(
            file.sha256.len() == 64 && file.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid Go catalog SHA-256"
        );
        let raw = crate::http::HTTP
            .get_bytes_bounded(&archive.checksum_url, 128)
            .await?;
        let text = std::str::from_utf8(&raw)?;
        ensure!(text.len() <= 128, "Go checksum metadata exceeds limit");
        let digest = text.trim();
        ensure!(
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid Go SHA-256 metadata"
        );
        ensure!(
            digest.eq_ignore_ascii_case(&file.sha256),
            "Go catalog and checksum sidecar disagree"
        );
        Ok(GoArchiveMetadata {
            archive,
            declared_sha256: format!("sha256:{}", digest.to_ascii_lowercase()),
            declared_size: file.size,
            catalog_sha256: catalog.digest.clone(),
        })
    }

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
    files: BTreeMap<String, Vec<ReleaseFile>>,
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
                .get_bytes_bounded("https://go.dev/dl/?mode=json&include=all", 16 * 1024 * 1024)
                .await?;
            ensure!(
                raw.len() <= 16 * 1024 * 1024,
                "Go catalog exceeds byte limit"
            );
            let raw = std::str::from_utf8(&raw)?;
            let releases: Vec<Release> = serde_json::from_str(raw)?;
            ensure!(
                releases.len() <= 100_000,
                "Go catalog exceeds release limit"
            );
            let mut seen = BTreeSet::new();
            let mut versions = Vec::new();
            let mut files = BTreeMap::new();
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
                ensure!(
                    release.files.len() <= 4096,
                    "Go release exceeds file count limit"
                );
                let mut filenames = BTreeSet::new();
                for file in &release.files {
                    ensure!(
                        !file.filename.is_empty()
                            && file.filename.len() <= 512
                            && filenames.insert(&file.filename),
                        "invalid or duplicate Go catalog filename"
                    );
                }
                files.insert(version.to_owned(), release.files);
                versions.push(version.to_owned());
            }
            let versions = versions
                .into_iter()
                .sorted_by_cached_key(|v| (Versioning::new(v), v.to_string()))
                .collect();
            Ok(Catalog {
                files,
                versions,
                digest: format!("sha256:{}", crate::hash::hash_sha256_to_str(raw)),
            })
        })
        .await
}
