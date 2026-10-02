//! Replay retained binary Python catalogs and checksum declarations, without network.
use base64::Engine;
use eyre::{Result, ensure};
use mise::embedding::{Options, Session};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Deserialize)]
struct Fixture {
    format: u32,
    response_encoding: String,
    legal_approval: bool,
    release_ready: bool,
    responses: BTreeMap<String, String>,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    target: String,
    catalog_url: String,
    compressed_sha256: String,
    compressed_size: usize,
    version: String,
    filename: String,
    release: String,
    checksum_url: String,
    checksum_sha256: String,
    checksum_size: usize,
    declared_sha256: String,
}
pub(super) fn read(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(100 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 100 * 1024 * 1024,
        "Python fixture exceeds limit"
    );
    Ok(bytes)
}
pub(super) fn run(state: PathBuf) -> Result<()> {
    let fixture: Fixture = serde_json::from_slice(&read(&state.join("python-replay.json"))?)?;
    ensure!(
        fixture.format == 1
            && fixture.response_encoding == "base64"
            && !fixture.legal_approval
            && !fixture.release_ready,
        "invalid Python fixture"
    );
    ensure!(
        fixture.cases.len() == 3 && (4..=6).contains(&fixture.responses.len()),
        "unexpected Python fixture shape"
    );
    let mut responses = BTreeMap::new();
    for (url, body) in fixture.responses {
        let raw = base64::engine::general_purpose::STANDARD.decode(body)?;
        ensure!(
            raw.len() <= 16 * 1024 * 1024,
            "replay response exceeds limit"
        );
        responses.insert(url, raw);
    }
    for (case, target) in fixture.cases.iter().zip([
        "linux/amd64/gnu",
        "darwin/arm64/native",
        "windows/amd64/msvc",
    ]) {
        ensure!(
            case.target == target && case.version == "3.12.13",
            "unexpected target/version"
        );
        for (url, size, hash) in [
            (
                &case.catalog_url,
                case.compressed_size,
                &case.compressed_sha256,
            ),
            (
                &case.checksum_url,
                case.checksum_size,
                &case.checksum_sha256,
            ),
        ] {
            let raw = responses
                .get(url)
                .ok_or_else(|| eyre::eyre!("missing replay bytes"))?;
            ensure!(
                raw.len() == size
                    && format!("sha256:{}", hex::encode(Sha256::digest(raw))) == *hash,
                "replay identity mismatch"
            );
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let responses = Arc::new(responses);
    let session = Session::initialize(Options {
        state,
        frontend: std::env::current_exe()?,
        tools: ["python".into()].into(),
        transport: Some(Arc::new(move |request| {
            count.fetch_add(1, Ordering::SeqCst);
            let responses = responses.clone();
            Box::pin(async move {
                let body = responses
                    .get(request.url.as_str())
                    .ok_or_else(|| eyre::eyre!("uncaptured Python request"))?;
                Ok(reqwest::Response::from(
                    http::Response::builder().status(200).body(body.clone())?,
                ))
            })
        })),
    })?;
    tokio::runtime::Runtime::new()?.block_on(async {
        for case in fixture.cases {
            let result = session
                .python_archive_metadata(&case.version, &case.target, Some(&case.filename))
                .await?;
            ensure!(
                result.artifact.filename == case.filename
                    && result.artifact.release == case.release
                    && result.artifact.target == case.target,
                "artifact identity changed"
            );
            ensure!(
                result.artifact.catalog_url == case.catalog_url
                    && result.artifact.catalog_sha256 == case.compressed_sha256,
                "catalog identity changed"
            );
            ensure!(
                result.declared_sha256 == case.declared_sha256
                    && result.checksum_manifest_url == case.checksum_url
                    && result.checksum_manifest_sha256 == case.checksum_sha256,
                "checksum identity changed"
            );
        }
        ensure!(
            calls.load(Ordering::SeqCst) == 6,
            "expected only catalog and checksum requests"
        );
        Ok(())
    })
}
