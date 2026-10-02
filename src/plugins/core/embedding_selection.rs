//! Shared admitted stable-version selection, using each backend's catalog and aliases.
//! This layer validates selectors and intersects metadata; it never installs.
use crate::backend::Backend;
use crate::config::Config;
use crate::toolset::{ToolRequest, ToolSource};
use eyre::{Result, ensure};
use std::sync::Arc;

enum Selector {
    Range(String),
    Version(String),
}

pub(super) async fn resolve_version(
    backend: &dyn Backend,
    config: &Arc<Config>,
    request: &str,
    constraints: &[String],
) -> Result<String> {
    ensure!(constraints.len() <= 256, "too many tool constraints");
    // Validate every input before any metadata acquisition or cache access.
    let selectors = std::iter::once(request)
        .chain(constraints.iter().map(String::as_str))
        .map(|query| selector(backend, query))
        .collect::<Result<Vec<_>>>()?;
    let mut versions = backend.list_remote_versions(config).await?;
    ensure!(
        versions.len() <= 100_000 && versions.iter().all(|v| v.len() <= 128),
        "tool catalog exceeds embedding selection limits"
    );
    // The initial archive contract admits canonical stable versions only.
    // Catalog membership is mandatory even for a fully specified pin.
    versions.retain(|version| {
        semver::Version::parse(version)
            .is_ok_and(|v| v.pre.is_empty() && v.build.is_empty() && v.to_string() == *version)
    });
    for selector in selectors {
        versions = match selector {
            Selector::Range(query) => crate::semver::npm_semver_range_filter(&versions, &query)
                .ok_or_else(|| eyre::eyre!("invalid tool semver range"))?,
            Selector::Version(query) => backend.fuzzy_match_filter(versions, &query, true),
        };
    }
    versions.pop().ok_or_else(|| {
        eyre::eyre!("no cataloged stable tool version satisfies request and native constraints")
    })
}

fn selector(backend: &dyn Backend, query: &str) -> Result<Selector> {
    ensure!(
        !query.trim().is_empty() && query.len() <= 1024 && !query.chars().any(char::is_control),
        "invalid or oversized tool selector"
    );
    if crate::semver::is_npm_semver_range_query(query) {
        ensure!(
            crate::semver::npm_semver_range_filter(&[], query).is_some(),
            "invalid tool semver range"
        );
        return Ok(Selector::Range(query.to_owned()));
    }
    // Do not admit mise's path, system, ref, subtraction or explicit-prefix
    // request modes. Ordinary numeric prefixes remain supported by its matcher.
    ensure!(
        matches!(
            ToolRequest::new(backend.ba().clone(), query, ToolSource::Argument)?,
            ToolRequest::Version { .. }
        ),
        "tool embedding requires a version, channel or semver range"
    );
    let aliases = backend.get_aliases()?;
    Ok(Selector::Version(
        aliases
            .get(query)
            .cloned()
            .unwrap_or_else(|| query.to_owned()),
    ))
}
