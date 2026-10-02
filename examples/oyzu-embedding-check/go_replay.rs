//! Replay independently captured publisher metadata without network acquisition.
//! A fixture is test input, not a signed source admission or artifact approval.
use eyre::{Result, ensure};
use mise::embedding::{Options, Session};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Read,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Deserialize)]
struct Fixture {
    format: u32,
    legal_approval: bool,
    release_ready: bool,
    responses: BTreeMap<String, Response>,
    expected: Vec<Expected>,
}
#[derive(Deserialize)]
struct Response {
    body_utf8: String,
    sha256: String,
    size: usize,
}
#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct Expected {
    version: String,
    target: String,
    archive_url: String,
    declared_size: u64,
    declared_sha256: String,
    catalog_sha256: String,
}

pub(super) fn read(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 32 * 1024 * 1024,
        "Go replay fixture exceeds limit"
    );
    Ok(bytes)
}
fn fixture(state: &Path) -> Result<Fixture> {
    let fixture: Fixture = serde_json::from_slice(&read(&state.join("go-replay.json"))?)?;
    ensure!(
        fixture.format == 1 && !fixture.legal_approval && !fixture.release_ready,
        "invalid or falsely approved replay fixture"
    );
    ensure!(
        !fixture.expected.is_empty()
            && fixture.expected.len() <= 48
            && fixture.responses.len() <= 49,
        "invalid Go replay fixture counts"
    );
    for response in fixture.responses.values() {
        ensure!(
            response.body_utf8.len() == response.size
                && format!(
                    "sha256:{}",
                    mise::hash::hash_sha256_to_str(&response.body_utf8)
                ) == response.sha256,
            "captured Go metadata byte identity mismatch"
        );
    }
    Ok(fixture)
}
pub(super) fn configure(input: &mut Options, state: &Path, calls: Arc<AtomicUsize>) -> Result<()> {
    let responses = Arc::new(fixture(state)?.responses);
    input.transport = Some(Arc::new(move |request| {
        calls.fetch_add(1, Ordering::SeqCst);
        let responses = responses.clone();
        Box::pin(async move {
            let response = responses
                .get(request.url.as_str())
                .ok_or_else(|| eyre::eyre!("uncaptured Go replay request"))?;
            Ok(reqwest::Response::from(
                http::Response::builder()
                    .status(200)
                    .body(response.body_utf8.clone())?,
            ))
        })
    }));
    Ok(())
}
pub(super) async fn check(session: &Session, state: &Path, calls: &AtomicUsize) -> Result<()> {
    let fixture = fixture(state)?;
    for expected in &fixture.expected {
        let selection = session.resolve_go_version(&expected.version, &[]).await?;
        ensure!(
            selection.version == expected.version
                && selection.catalog_sha256 == expected.catalog_sha256,
            "real Go selection or catalog digest mismatch"
        );
        let actual = session
            .go_archive_metadata(&expected.version, &expected.target)
            .await?;
        let actual = Expected {
            version: actual.archive.version,
            target: actual.archive.target,
            archive_url: actual.archive.archive_url,
            declared_size: actual.declared_size,
            declared_sha256: actual.declared_sha256,
            catalog_sha256: actual.catalog_sha256,
        };
        ensure!(
            actual == *expected,
            "real Go target metadata differs: {actual:?}"
        );
    }
    ensure!(
        calls.load(Ordering::SeqCst) == fixture.expected.len() + 1,
        "replay cache/acquisition count differs"
    );
    Ok(())
}
