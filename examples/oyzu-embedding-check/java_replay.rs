//! Replay captured Java catalogs through the actual core backend, without network.
//! Expected metadata is fixture evidence, never publisher or distribution approval.
use eyre::{Result, ensure};
use mise::embedding::{Options, Session};
use serde::Deserialize;
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
    responses: BTreeMap<String, String>,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    target: String,
    catalog_url: String,
    catalog_sha256: String,
    catalog_size: usize,
    canonical_version: String,
    archive_url: String,
    declared_sha256: String,
}
pub(super) fn read(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 32 * 1024 * 1024,
        "Java replay fixture exceeds limit"
    );
    Ok(bytes)
}
fn fixture(state: &Path) -> Result<Fixture> {
    let fixture: Fixture = serde_json::from_slice(&read(&state.join("java-replay.json"))?)?;
    ensure!(
        fixture.format == 1 && !fixture.legal_approval && !fixture.release_ready,
        "invalid or falsely approved Java fixture"
    );
    ensure!(
        fixture.responses.len() == 3 && fixture.cases.len() == 3,
        "expected three Java targets"
    );
    for case in &fixture.cases {
        let body = fixture
            .responses
            .get(&case.catalog_url)
            .ok_or_else(|| eyre::eyre!("missing captured Java catalog"))?;
        ensure!(
            body.len() == case.catalog_size
                && body.len() <= 16 * 1024 * 1024
                && format!("sha256:{}", mise::hash::hash_sha256_to_str(body))
                    == case.catalog_sha256,
            "captured Java catalog identity mismatch"
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
            let body = responses
                .get(request.url.as_str())
                .ok_or_else(|| eyre::eyre!("uncaptured Java replay request"))?;
            Ok(reqwest::Response::from(
                http::Response::builder().status(200).body(body.clone())?,
            ))
        })
    }));
    Ok(())
}
pub(super) async fn check(session: &Session, state: &Path, calls: &AtomicUsize) -> Result<()> {
    use mise::toolset::{ToolRequest, ToolSource, ToolVersion};
    ensure!(
        session.config().config_files.is_empty(),
        "Java replay discovered config"
    );
    let fixture = fixture(state)?;
    mise::backend::load_tools().await?;
    let argument = Arc::new(mise::args::BackendArg::from("java"));
    let backend =
        mise::backend::get(&argument).ok_or_else(|| eyre::eyre!("Java backend unavailable"))?;
    for case in &fixture.cases {
        let native = match case.target.as_str() {
            "linux/amd64/gnu" => "linux-x64",
            "darwin/arm64/native" => "macos-arm64",
            "windows/amd64/msvc" => "windows-x64",
            _ => eyre::bail!("unsupported Java fixture target"),
        };
        let target = mise::backend::platform_target::PlatformTarget::new(
            mise::platform::Platform::parse(native)?,
        );
        let version = ToolVersion::new(
            ToolRequest::new(
                argument.clone(),
                &case.canonical_version,
                ToolSource::Argument,
            )?,
            case.canonical_version.clone(),
        );
        let actual = backend.resolve_lock_info(&version, &target).await?;
        ensure!(
            actual.url.as_deref() == Some(case.archive_url.as_str())
                && actual.checksum.as_deref() == Some(case.declared_sha256.as_str()),
            "real Java target metadata differs"
        );
        let unavailable = ToolVersion::new(
            ToolRequest::new(argument.clone(), "temurin-0.0.0", ToolSource::Argument)?,
            "temurin-0.0.0".into(),
        );
        ensure!(
            backend
                .resolve_lock_info(&unavailable, &target)
                .await
                .is_err(),
            "missing Java version accepted"
        );
    }
    ensure!(
        calls.load(Ordering::SeqCst) == 3,
        "Java replay acquisition/cache count differs"
    );
    Ok(())
}
