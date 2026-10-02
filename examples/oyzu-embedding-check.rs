//! Library-only process-boundary conformance; never builds/runs the mise CLI.
use eyre::{Result, ensure};
use mise::embedding::{Options, Session};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

fn options(state: PathBuf) -> Result<Options> {
    Ok(Options {
        state,
        frontend: std::env::current_exe()?,
        tools: BTreeSet::from(["node".into()]),
        transport: None,
    })
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("child") {
        return child(&args[2], PathBuf::from(&args[3]));
    }
    let directory = tempfile::tempdir()?;
    let root = directory.path().canonicalize()?;
    std::fs::write(root.join("mise.toml"), "this is not valid TOML [")?;
    std::fs::write(root.join(".tool-versions"), "node this-must-not-be-read")?;
    for scenario in ["offline", "transport", "hostile", "unadmitted"] {
        let state = root.join(scenario);
        std::fs::create_dir_all(state.join("home"))?;
        std::fs::write(
            state.join("home/.mise.toml"),
            "invalid global configuration [",
        )?;
        let mut command = Command::new(std::env::current_exe()?);
        command
            .env_clear()
            .current_dir(&root)
            .args(["child", scenario])
            .arg(&state);
        for name in ["PATH", "SYSTEMROOT", "WINDIR", "TEMP", "TMP"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .env("HOME", state.join("home"))
            .env("USERPROFILE", state.join("home"));
        if scenario == "hostile" {
            command.env("MISE_CONFIG_FILE", root.join("mise.toml"));
        }
        let output = command.output()?;
        ensure!(
            output.status.success(),
            "{scenario}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        println!("{scenario}: passed");
    }
    Ok(())
}

fn child(scenario: &str, state: PathBuf) -> Result<()> {
    let mut input = options(state.clone())?;
    if scenario == "hostile" {
        ensure!(
            Session::initialize(input).is_err(),
            "hostile environment was accepted"
        );
        return Ok(());
    }
    if scenario == "unadmitted" {
        input.tools.insert("asdf:node".into());
        ensure!(
            Session::initialize(input).is_err(),
            "unadmitted backend was accepted"
        );
        return Ok(());
    }
    let calls = Arc::new(AtomicUsize::new(0));
    if scenario == "transport" {
        let calls = calls.clone();
        input.transport = Some(Arc::new(move |request| {
            calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                ensure!(
                    request.url.as_str() == "https://example.invalid/catalog",
                    "unexpected request"
                );
                Ok(reqwest::Response::from(
                    http::Response::builder()
                        .status(200)
                        .body("broker-response")?,
                ))
            })
        }));
    }
    let session = Session::initialize(input)?;
    mise::config::settings::clear();
    let settings = mise::config::Settings::get();
    ensure!(
        !settings.auto_install && settings.lockfile == Some(false),
        "settings reload lost embedding restrictions"
    );
    ensure!(
        settings.enable_tools == Some(BTreeSet::from(["node".into()])),
        "settings reload lost admission"
    );
    ensure!(
        session.config().config_files.is_empty(),
        "discovered project config"
    );
    ensure!(
        session.config().project_root.is_none(),
        "discovered project root"
    );
    ensure!(*mise::env::HOME == state.join("home"), "ambient home");
    ensure!(
        *mise::env::MISE_INSTALLS_DIR == state.join("installs"),
        "ambient install root"
    );
    ensure!(
        mise::shims::mise_bin_for_shims() == std::env::current_exe()?,
        "ambient mise executable"
    );
    let path: mise::path_env::PathEnv = [state.join("a"), state.join("a"), state.join("b")]
        .into_iter()
        .collect();
    ensure!(
        std::env::split_paths(&path.join_verbatim()?).count() == 3,
        "lost duplicate PATH entry"
    );
    ensure!(
        Session::initialize(options(state)?).is_err(),
        "worker was reused"
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(mise::http::HTTP.get_text("https://example.invalid/catalog"));
    ensure!(
        mise::http::HTTP.reqwest().is_err(),
        "direct HTTP client escaped embedding"
    );
    if scenario == "transport" {
        ensure!(
            result? == "broker-response" && calls.load(Ordering::SeqCst) == 1,
            "transport callback was bypassed"
        );
        for (method, url, credential) in [
            (
                reqwest::Method::POST,
                "https://example.invalid/catalog",
                false,
            ),
            (
                reqwest::Method::GET,
                "https://user:secret@example.invalid/catalog",
                false,
            ),
            (reqwest::Method::GET, "file:///catalog", false),
            (
                reqwest::Method::GET,
                "https://example.invalid/catalog",
                true,
            ),
        ] {
            let mut headers = reqwest::header::HeaderMap::new();
            if credential {
                headers.insert(reqwest::header::AUTHORIZATION, "Bearer forbidden".parse()?);
            }
            let result =
                runtime.block_on(mise_util::embedding::send(mise::embedding::HttpRequest {
                    method,
                    url: url.parse()?,
                    headers,
                }));
            ensure!(result.is_err(), "unapproved request reached transport");
        }
        ensure!(
            calls.load(Ordering::SeqCst) == 1,
            "transport admission was bypassed"
        );
    } else {
        ensure!(
            result
                .unwrap_err()
                .to_string()
                .contains("embedding HTTP transport is not authorized"),
            "direct HTTP fallback"
        );
        ensure!(calls.load(Ordering::SeqCst) == 0, "unexpected callback");
    }
    Ok(())
}
