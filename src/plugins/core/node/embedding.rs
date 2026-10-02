//! Node metadata selection for the admitted embedding boundary. Reuses mise's
//! catalog, aliases, version ordering and range/prefix matchers; never installs.
use super::NodePlugin;
use crate::backend::Backend;
use crate::config::Config;
use crate::toolset::{ToolRequest, ToolSource};
use eyre::{Result, ensure};
use std::sync::Arc;

enum Selector {
    Range(String),
    Version(String),
}

impl NodePlugin {
    pub(in crate::plugins::core) async fn embedding_resolve_version(
        &self,
        config: &Arc<Config>,
        request: &str,
        constraints: &[String],
    ) -> Result<String> {
        ensure!(constraints.len() <= 256, "too many Node constraints");
        // Validate every input before any metadata acquisition or cache access.
        let selectors = std::iter::once(request)
            .chain(constraints.iter().map(String::as_str))
            .map(|query| self.embedding_selector(query))
            .collect::<Result<Vec<_>>>()?;
        let mut versions = self.list_remote_versions(config).await?;
        ensure!(
            versions.len() <= 100_000 && versions.iter().all(|v| v.len() <= 128),
            "Node catalog exceeds embedding selection limits"
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
                    .ok_or_else(|| eyre::eyre!("invalid Node semver range"))?,
                Selector::Version(query) => self.fuzzy_match_filter(versions, &query, true),
            };
        }
        versions.pop().ok_or_else(|| {
            eyre::eyre!("no cataloged stable Node version satisfies request and native constraints")
        })
    }

    fn embedding_selector(&self, query: &str) -> Result<Selector> {
        ensure!(
            !query.trim().is_empty() && query.len() <= 1024 && !query.chars().any(char::is_control),
            "invalid or oversized Node selector"
        );
        if crate::semver::is_npm_semver_range_query(query) {
            ensure!(
                crate::semver::npm_semver_range_filter(&[], query).is_some(),
                "invalid Node semver range"
            );
            return Ok(Selector::Range(query.to_owned()));
        }
        // Do not admit mise's path, system, ref, subtraction or explicit-prefix
        // request modes. Ordinary numeric prefixes remain supported by its matcher.
        ensure!(
            matches!(
                ToolRequest::new(self.ba.clone(), query, ToolSource::Argument)?,
                ToolRequest::Version { .. }
            ),
            "Node embedding requires a version, channel or semver range"
        );
        let aliases = self.get_aliases()?;
        Ok(Selector::Version(
            aliases
                .get(query)
                .cloned()
                .unwrap_or_else(|| query.to_owned()),
        ))
    }
}
