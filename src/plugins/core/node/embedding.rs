//! Node metadata selection for the admitted embedding boundary. Reuses mise's
//! catalog, aliases, version ordering and range/prefix matchers; never installs.
use super::{NodeArchiveFacts, NodePlugin};
use eyre::Result;

/// Publisher-declared metadata only. Does not authenticate the manifest, verify
/// artifact content, provide its byte size or authorize acquisition/installation.
#[derive(Debug, serde::Serialize)]
pub struct NodeArchiveMetadata {
    pub archive: NodeArchiveFacts,
    pub declared_sha256: String,
}

impl NodePlugin {
    pub(in crate::plugins::core) async fn embedding_archive_metadata(
        &self,
        version: &str,
        target: &crate::backend::platform_target::PlatformTarget,
        target_key: &str,
    ) -> Result<NodeArchiveMetadata> {
        let archive = self.embedding_archive_facts(version, target, target_key)?;
        let artifact = self.binary_artifact(version, target)?;
        let text = self.shasums(&artifact.mirror, version).await?;
        let checksums = crate::hash::parse_sha256sums_checked(&text)?;
        let checksum = checksums
            .get(&artifact.filename)
            .ok_or_else(|| eyre::eyre!("Node target archive is absent from checksum manifest"))?;
        Ok(NodeArchiveMetadata {
            archive,
            declared_sha256: format!("sha256:{checksum}"),
        })
    }
}
