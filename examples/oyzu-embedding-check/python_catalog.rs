//! Public Session conformance: only catalog transport is permitted.
use eyre::{Result, ensure};
use mise::embedding::{Options, Session};
use std::{
    collections::BTreeSet,
    io::Write,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const TARGETS: [(&str, &str); 3] = [
    ("linux/amd64/gnu", "x86_64-unknown-linux-gnu"),
    ("darwin/arm64/native", "aarch64-apple-darwin"),
    ("windows/amd64/msvc", "x86_64-pc-windows-msvc"),
];

pub fn run(scenario: &str, state: PathBuf) -> Result<()> {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut options = Options {
        state,
        frontend: std::env::current_exe()?,
        tools: if scenario == "python-unadmitted" {
            BTreeSet::new()
        } else {
            ["python".into()].into()
        },
        transport: None,
    };
    if scenario != "python-offline" {
        let calls = calls.clone();
        let checksum_mode = scenario.starts_with("python-checksums");
        let duplicate = scenario == "python-checksums-duplicate";
        let missing = scenario == "python-checksums-missing";
        options.transport = Some(Arc::new(move |request| {
            calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if checksum_mode
                    && request.url.as_str()
                        == "https://github.com/astral-sh/python-build-standalone/releases/download/20250323/SHA256SUMS"
                {
                    let mut body = TARGETS
                        .iter()
                        .map(|(_, platform)| {
                            format!(
                                "{}  cpython-3.12.13+20250323-{platform}-install_only.tar.gz\n",
                                "a".repeat(64)
                            )
                        })
                        .collect::<String>();
                    if duplicate {
                        body = body.repeat(2);
                    }
                    if missing {
                        body.clear();
                    }
                    return Ok(reqwest::Response::from(
                        http::Response::builder().status(200).body(body)?,
                    ));
                }
                let platform = TARGETS.iter().find_map(|(_, platform)| {
                    (request.url.as_str() == format!("https://mise-versions.jdx.dev/tools/python-precompiled-{platform}.gz"))
                        .then_some(*platform)
                }).ok_or_else(|| eyre::eyre!("unexpected request: only Python catalog routes allowed"))?;
                let manifest = format!(
                    "cpython-3.12.13+20250323-{platform}-install_only.tar.gz\n\
                     cpython-3.12.13+20260805-{platform}-install_only_stripped.tar.gz\n\
                     cpython-3.12.14+20260805-{platform}-install_only.tar.gz/evil\n"
                );
                let mut encoder =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                encoder.write_all(manifest.as_bytes())?;
                Ok(reqwest::Response::from(
                    http::Response::builder()
                        .status(200)
                        .body(encoder.finish()?)?,
                ))
            })
        }));
    }
    let session = Session::initialize(options)?;
    tokio::runtime::Runtime::new()?.block_on(async {
        if scenario.starts_with("python-checksums") {
            for (target, platform) in TARGETS {
                let locked = format!("cpython-3.12.13+20250323-{platform}-install_only.tar.gz");
                let result = session
                    .python_archive_metadata("3.12.13", target, Some(&locked))
                    .await;
                if scenario != "python-checksums" {
                    ensure!(result.is_err(), "ambiguous or missing checksum accepted");
                } else {
                    let metadata = result?;
                    ensure!(
                        metadata.artifact.filename == locked,
                        "checksum bound to wrong artifact"
                    );
                    ensure!(
                        metadata.declared_sha256 == format!("sha256:{}", "a".repeat(64)),
                        "wrong declared checksum"
                    );
                    ensure!(
                        metadata
                            .checksum_manifest_url
                            .ends_with("/20250323/SHA256SUMS"),
                        "wrong checksum route"
                    );
                    ensure!(
                        metadata.checksum_manifest_sha256.len() == 71,
                        "missing checksum snapshot digest"
                    );
                }
            }
            ensure!(
                calls.load(Ordering::SeqCst) == 6,
                "expected catalog and checksum requests only"
            );
            return Ok(());
        }
        if scenario != "python-catalog" {
            ensure!(
                session
                    .python_catalog_artifact("3.12.13", TARGETS[0].0, None)
                    .await
                    .is_err(),
                "missing capability accepted"
            );
            ensure!(
                calls.load(Ordering::SeqCst) == 0,
                "denial performed transport"
            );
            return Ok(());
        }
        for (version, target, locked) in [
            ("latest", TARGETS[0].0, None),
            ("3.12.13", "linux/arm64/gnu", None),
            ("3.12.13", TARGETS[0].0, Some("../artifact")),
        ] {
            ensure!(
                session
                    .python_catalog_artifact(version, target, locked)
                    .await
                    .is_err(),
                "invalid query accepted"
            );
        }
        ensure!(
            calls.load(Ordering::SeqCst) == 0,
            "invalid input performed transport"
        );
        for (target, platform) in TARGETS {
            let locked = format!("cpython-3.12.13+20250323-{platform}-install_only.tar.gz");
            let facts = session
                .python_catalog_artifact("3.12.13", target, Some(&locked))
                .await?;
            ensure!(
                facts.filename == locked && facts.version == "3.12.13" && facts.target == target,
                "locked identity changed"
            );
            ensure!(
                facts.release == "20250323"
                    && facts.archive_url.ends_with(&format!("/20250323/{locked}")),
                "wrong artifact URL"
            );
            ensure!(
                facts.catalog_sha256.starts_with("sha256:") && facts.catalog_sha256.len() == 71,
                "missing catalog digest"
            );
            let update = session
                .python_catalog_artifact("3.12.13", target, None)
                .await?;
            ensure!(
                update.release == "20260805",
                "upstream update ordering changed"
            );
            for (version, filename) in [
                ("3.12.13", Some("absent.tar.gz")),
                ("3.12.14", None),
                ("3.12.99", None),
            ] {
                ensure!(
                    session
                        .python_catalog_artifact(version, target, filename)
                        .await
                        .is_err(),
                    "invalid artifact accepted"
                );
            }
        }
        ensure!(
            calls.load(Ordering::SeqCst) == 15,
            "unexpected acquisition or missing catalog request"
        );
        Ok(())
    })
}
