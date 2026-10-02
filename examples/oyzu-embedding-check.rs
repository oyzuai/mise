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
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).map(|value| value.as_os_str()) == Some(std::ffi::OsStr::new("child")) {
        ensure!(args.len() == 4, "invalid child arguments");
        return child(
            args[2]
                .to_str()
                .ok_or_else(|| eyre::eyre!("scenario is not UTF-8"))?,
            PathBuf::from(&args[3]),
        );
    }
    ensure!(args.len() == 1, "unexpected conformance arguments");
    let directory = tempfile::tempdir()?;
    let root = directory.path().canonicalize()?;
    std::fs::write(root.join("mise.toml"), "this is not valid TOML [")?;
    std::fs::write(root.join(".tool-versions"), "node this-must-not-be-read")?;
    for scenario in [
        "offline",
        "transport",
        "hostile",
        "unadmitted",
        "node-denied",
        "backend",
        "go-facts",
        "java-metadata",
        "catalog",
        "node-resolve",
        "node-resolve-offline",
    ] {
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
    if scenario == "catalog" {
        input.tools = ["node", "go", "java", "python"].map(String::from).into();
    }
    if scenario == "go-facts" {
        input.tools = BTreeSet::from(["go".into()]);
    }
    if scenario == "java-metadata" {
        input.tools = BTreeSet::from(["java".into()]);
    }
    if scenario == "node-denied" {
        input.tools.clear();
    }
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
    if scenario == "go-facts" {
        let calls = calls.clone();
        input.transport = Some(Arc::new(move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { eyre::bail!("Go archive facts must not acquire metadata") })
        }));
    }
    if scenario == "java-metadata" {
        let calls = calls.clone();
        input.transport = Some(Arc::new(move |request| {
            calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                let (target, kind) = match request.url.as_str() {
                    "https://mise-java.jdx.dev/jvm/ga/linux/x86_64.json" => ("linux", "tar.gz"),
                    "https://mise-java.jdx.dev/jvm/ga/macosx/aarch64.json" => ("macos", "tar.gz"),
                    "https://mise-java.jdx.dev/jvm/ga/windows/x86_64.json" => ("windows", "zip"),
                    _ => eyre::bail!("unexpected Java metadata route"),
                };
                let body = serde_json::json!([
                    {"vendor":"temurin", "version":"21.0.9+10", "java_version":"21.0.9", "image_type":"jdk", "jvm_impl":"hotspot", "file_type":kind, "url":format!("https://example.invalid/{target}/jdk.{kind}"), "checksum":format!("sha256:{}", "a".repeat(64))},
                    {"vendor":"temurin", "version":"99.0.0", "java_version":"99.0.0", "image_type":"jdk", "jvm_impl":"hotspot", "file_type":"msi", "url":"https://example.invalid/unsupported.msi"}
                ]).to_string();
                Ok(reqwest::Response::from(
                    http::Response::builder().status(200).body(body)?,
                ))
            })
        }));
    }
    if matches!(scenario, "backend" | "node-resolve") {
        let calls = calls.clone();
        input.transport = Some(Arc::new(move |request| {
            calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                ensure!(
                    request.url.as_str() == "https://nodejs.org/dist/index.json",
                    "unexpected backend metadata URL: {}",
                    request.url
                );
                Ok(reqwest::Response::from(http::Response::builder().status(200).body(
                    r#"[{"version":"v24.1.0","date":"2025-05-19","files":["linux-x64"]},{"version":"v22.15.0","date":"2025-04-22","files":["linux-x64"]},{"version":"v22.14.0","date":"2025-02-11","files":["linux-x64"]}]"#
                )?))
            })
        }));
    }
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
    let session = Session::initialize(input).map_err(|error| {
        // This child was created with env_clear and the fixture's explicit
        // allowlist. Report names only to diagnose native runtime additions;
        // never print values or the parent/developer environment.
        let names: Vec<_> = std::env::vars_os().map(|(name, _)| name).collect();
        eyre::eyre!("{error}; isolated conformance child variable names: {names:?}")
    })?;
    if scenario == "node-resolve" {
        tokio::runtime::Runtime::new()?.block_on(check_node_resolution(&session, calls))?;
        return Ok(());
    }
    if scenario == "node-resolve-offline" {
        ensure!(
            tokio::runtime::Runtime::new()?
                .block_on(session.resolve_node_version("22.14.0", &[]))
                .is_err(),
            "exact pin bypassed absent metadata transport"
        );
        return Ok(());
    }
    if scenario == "catalog" {
        let aliases = session.tool_aliases()?;
        for short in ["node", "go", "java", "python"] {
            let canonical = format!("core:{short}");
            ensure!(aliases.get(short) == Some(&canonical), "missing core alias");
            ensure!(
                aliases.get(&canonical) == Some(&canonical),
                "missing canonical name"
            );
        }
        ensure!(
            !aliases.contains_key("ruby"),
            "unadmitted tool escaped catalog"
        );
        mise::config::settings::clear();
        ensure!(
            !mise::config::Settings::get().registry_floating,
            "floating registry enabled"
        );
        ensure!(
            session.tool_aliases()? == aliases,
            "catalog changed after reload"
        );
        return Ok(());
    }
    if scenario == "java-metadata" {
        tokio::runtime::Runtime::new()?.block_on(check_java_metadata(&session))?;
        ensure!(
            calls.load(Ordering::SeqCst) == 3,
            "Java metadata must use one supplied request per target"
        );
        return Ok(());
    }
    if scenario == "go-facts" {
        tokio::runtime::Runtime::new()?.block_on(check_go_facts(&session))?;
        ensure!(
            calls.load(Ordering::SeqCst) == 0,
            "Go facts acquired metadata"
        );
        return Ok(());
    }
    ensure!(
        session
            .go_archive_facts("1.24.13", "linux/amd64/gnu")
            .is_err(),
        "Go facts escaped session admission"
    );
    if scenario == "node-denied" {
        ensure!(
            tokio::runtime::Runtime::new()?
                .block_on(session.resolve_node_version("22", &[]))
                .is_err(),
            "Node resolution escaped session admission"
        );
        ensure!(
            session.tool_aliases()?.is_empty(),
            "empty admission exposed aliases"
        );
        ensure!(
            session
                .node_archive_facts("22.15.0", "linux/amd64/gnu")
                .is_err(),
            "Node facts escaped session admission"
        );
        return Ok(());
    }
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
    if scenario == "backend" {
        return runtime.block_on(check_backend(&session, calls));
    }
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

async fn check_go_facts(session: &Session) -> Result<()> {
    use mise::toolset::{ToolRequest, ToolSource, ToolVersion};
    mise::config::settings::clear();
    let settings = mise::config::Settings::get();
    ensure!(
        settings.enable_tools == Some(BTreeSet::from(["go".into()])) && !settings.go.skip_checksum,
        "Go settings reload lost immutable admission or checksum defaults"
    );
    mise::backend::load_tools().await?;
    let argument = Arc::new(mise::args::BackendArg::from("go"));
    let backend =
        mise::backend::get(&argument).ok_or_else(|| eyre::eyre!("core Go backend unavailable"))?;
    for version in ["1.24.13", "1.25.2"] {
        let tv = ToolVersion::new(
            ToolRequest::new(argument.clone(), version, ToolSource::Argument)?,
            version.into(),
        );
        for (target, native, suffix, kind, executable) in [
            (
                "linux/amd64/gnu",
                "linux-x64",
                "linux-amd64",
                "tar.gz",
                "bin/go",
            ),
            (
                "darwin/arm64/native",
                "macos-arm64",
                "darwin-arm64",
                "tar.gz",
                "bin/go",
            ),
            (
                "windows/amd64/msvc",
                "windows-x64",
                "windows-amd64",
                "zip",
                "bin/go.exe",
            ),
        ] {
            let facts = session.go_archive_facts(version, target)?;
            let url = format!("https://dl.google.com/go/go{version}.{suffix}.{kind}");
            ensure!(
                facts.version == version
                    && facts.target == target
                    && facts.archive_url == url
                    && facts.checksum_url == format!("{url}.sha256")
                    && facts.archive_kind == kind
                    && facts.strip_prefix == "go"
                    && facts.go_relative_path == executable
                    && facts.bin_relative_path == "bin"
                    && facts.goroot_relative_path == ".",
                "incorrect Go archive facts: {facts:?}"
            );
            let platform = mise::backend::platform_target::PlatformTarget::new(
                mise::platform::Platform::parse(native)?,
            );
            ensure!(
                backend.get_tarball_url(&tv, &platform).await? == Some(facts.archive_url),
                "Go facts differ from upstream artifact selection"
            );
        }
    }
    for version in [
        "1.24",
        "go1.24.13",
        "latest",
        "1.25.0-rc.1",
        "1.24.13+build",
        "../../1.24.13",
    ] {
        ensure!(
            session
                .go_archive_facts(version, "linux/amd64/gnu")
                .is_err(),
            "nonexact Go version accepted"
        );
    }
    for target in [
        "linux/arm64/gnu",
        "linux/amd64/musl",
        "darwin/amd64/native",
        "windows/arm64/msvc",
        "linux-x64",
    ] {
        ensure!(
            session.go_archive_facts("1.24.13", target).is_err(),
            "unqualified Go target accepted"
        );
    }
    ensure!(
        session
            .node_archive_facts("22.15.0", "linux/amd64/gnu")
            .is_err(),
        "Node escaped Go-only admission"
    );
    Ok(())
}

async fn check_node_resolution(session: &Session, calls: Arc<AtomicUsize>) -> Result<()> {
    for query in [
        "",
        "system",
        "path:/tmp/node",
        "ref:main",
        "ref-main",
        "sub-1:22",
        "prefix:22",
        ">=nope",
        "22\n",
        "$(echo injected)",
    ] {
        ensure!(
            session.resolve_node_version(query, &[]).await.is_err(),
            "unsafe Node selector accepted"
        );
    }
    ensure!(
        session
            .resolve_node_version(&"2".repeat(1025), &[])
            .await
            .is_err(),
        "oversized request accepted"
    );
    ensure!(
        session
            .resolve_node_version("22", &vec!["22".into(); 257])
            .await
            .is_err(),
        "too many constraints accepted"
    );
    ensure!(
        session
            .resolve_node_version("22", &["path:/tmp/node".into()])
            .await
            .is_err(),
        "unsafe constraint accepted"
    );
    ensure!(
        calls.load(Ordering::SeqCst) == 0,
        "invalid input acquired metadata"
    );
    for (query, expected) in [
        ("22", "22.15.0"),
        ("22.14.0", "22.14.0"),
        ("v22.14.0", "22.14.0"),
        ("latest", "24.1.0"),
        ("lts/jod", "22.15.0"),
        (">=22 <24", "22.15.0"),
        ("^22.14.0", "22.15.0"),
        ("22.x", "22.15.0"),
        (">=24 || <22.15", "24.1.0"),
    ] {
        ensure!(
            session.resolve_node_version(query, &[]).await? == expected,
            "Node selector resolved incorrectly: {query}"
        );
    }
    let constraints = [">=22".into(), "<22.15".into()];
    ensure!(
        session.resolve_node_version("latest", &constraints).await? == "22.14.0",
        "constraints were not intersected before selection"
    );
    let reversed = [constraints[1].clone(), constraints[0].clone()];
    ensure!(
        session.resolve_node_version("latest", &reversed).await? == "22.14.0",
        "constraint order changed selection"
    );
    ensure!(
        session
            .resolve_node_version("22", &[">=24".into()])
            .await
            .is_err(),
        "incompatible constraint was ignored"
    );
    for query in ["99", "22.14.9", "22.14.0-rc.1", "22.14.0+build"] {
        ensure!(
            session.resolve_node_version(query, &[]).await.is_err(),
            "uncataloged or nonstable pin accepted"
        );
    }
    ensure!(
        calls.load(Ordering::SeqCst) == 1,
        "resolution did not reuse supplied catalog"
    );
    ensure!(
        session.config().config_files.is_empty(),
        "resolution loaded ambient configuration"
    );
    ensure!(
        !mise::env::MISE_DATA_DIR.join("installs").exists(),
        "resolution installed a tool"
    );
    Ok(())
}

async fn check_backend(session: &Session, calls: Arc<AtomicUsize>) -> Result<()> {
    use mise::toolset::{ResolveOptions, ToolRequest, ToolSource};
    mise::backend::load_tools().await?;
    let argument = Arc::new(mise::args::BackendArg::from("node"));
    let backend = mise::backend::get(&argument)
        .ok_or_else(|| eyre::eyre!("core Node backend unavailable"))?;
    let versions = backend.list_remote_versions(session.config()).await?;
    ensure!(
        versions == ["22.14.0", "22.15.0", "24.1.0"],
        "upstream Node catalog normalization failed: {versions:?}"
    );
    let request = ToolRequest::new(argument, "22", ToolSource::Argument)?;
    let resolved = request
        .resolve(
            session.config(),
            &ResolveOptions {
                latest_versions: true,
                latest_versions_for_all_requests: true,
                use_locked_version: false,
                ..Default::default()
            },
        )
        .await?;
    ensure!(
        resolved.version == "22.15.0",
        "upstream Node prefix resolution failed: {}",
        resolved.version
    );
    ensure!(
        calls.load(Ordering::SeqCst) > 0,
        "backend bypassed metadata transport"
    );
    ensure!(
        session.config().config_files.is_empty(),
        "backend discovered ambient configuration"
    );
    let previous_calls = calls.load(Ordering::SeqCst);
    for (target, native, suffix, kind, node, npm, bin) in [
        (
            "linux/amd64/gnu",
            "linux-x64",
            "linux-x64",
            "tar.gz",
            "bin/node",
            "bin/npm",
            "bin",
        ),
        (
            "darwin/arm64/native",
            "macos-arm64",
            "darwin-arm64",
            "tar.gz",
            "bin/node",
            "bin/npm",
            "bin",
        ),
        (
            "windows/amd64/msvc",
            "windows-x64",
            "win-x64",
            "zip",
            "node.exe",
            "npm.cmd",
            ".",
        ),
    ] {
        let facts = session.node_archive_facts(&resolved.version, target)?;
        let slug = format!("node-v22.15.0-{suffix}");
        let url = format!("https://nodejs.org/dist/v22.15.0/{slug}.{kind}");
        ensure!(
            facts.version == "22.15.0"
                && facts.target == target
                && facts.archive_url == url
                && facts.archive_kind == kind
                && facts.strip_prefix == slug
                && facts.node_relative_path == node
                && facts.npm_launcher_relative_path == npm
                && facts.bin_relative_path == bin
                && facts.checksums_url == "https://nodejs.org/dist/v22.15.0/SHASUMS256.txt"
                && facts.signature_url == "https://nodejs.org/dist/v22.15.0/SHASUMS256.txt.sig",
            "incorrect Node target facts: {facts:?}"
        );
        let platform = mise::backend::platform_target::PlatformTarget::new(
            mise::platform::Platform::parse(native)?,
        );
        ensure!(
            backend.get_tarball_url(&resolved, &platform).await? == Some(facts.archive_url),
            "Node facts differ from upstream lock artifact selection"
        );
        let second = session.node_archive_facts("24.1.0", target)?;
        ensure!(
            second.archive_url.contains("/v24.1.0/node-v24.1.0-"),
            "version leaked between plans"
        );
    }
    for version in [
        "22",
        "latest",
        "v22.15.0",
        "22.15.0-rc.1",
        "22.15.0+build",
        "../../22.15.0",
    ] {
        ensure!(
            session
                .node_archive_facts(version, "linux/amd64/gnu")
                .is_err(),
            "unresolved/unsafe version accepted"
        );
    }
    for target in [
        "linux/amd64/musl",
        "linux/arm64/gnu",
        "darwin/amd64/native",
        "windows/arm64/msvc",
        "linux-x64",
    ] {
        ensure!(
            session.node_archive_facts("22.15.0", target).is_err(),
            "unqualified target accepted"
        );
    }
    ensure!(
        calls.load(Ordering::SeqCst) == previous_calls,
        "layout facts performed acquisition"
    );
    Ok(())
}

async fn check_java_metadata(session: &Session) -> Result<()> {
    use mise::toolset::{ToolRequest, ToolSource, ToolVersion};
    mise::config::settings::clear();
    ensure!(
        mise::config::Settings::get().enable_tools == Some(BTreeSet::from(["java".into()])),
        "Java admission changed after settings reload"
    );
    ensure!(
        session.config().config_files.is_empty(),
        "Java discovered configuration"
    );
    ensure!(
        session
            .node_archive_facts("22.15.0", "linux/amd64/gnu")
            .is_err(),
        "Java session admitted Node"
    );
    mise::backend::load_tools().await?;
    let argument = Arc::new(mise::args::BackendArg::from("java"));
    let backend = mise::backend::get(&argument)
        .ok_or_else(|| eyre::eyre!("core Java backend unavailable"))?;
    for (target, label, kind) in [
        ("linux-x64", "linux", "tar.gz"),
        ("macos-arm64", "macos", "tar.gz"),
        ("windows-x64", "windows", "zip"),
    ] {
        let target = mise::backend::platform_target::PlatformTarget::new(
            mise::platform::Platform::parse(target)?,
        );
        for version in ["temurin-21.0.9+10", "temurin-99.0.0", "temurin-17.0.0"] {
            let tv = ToolVersion::new(
                ToolRequest::new(argument.clone(), version, ToolSource::Argument)?,
                version.into(),
            );
            let result = backend.resolve_lock_info(&tv, &target).await;
            if version == "temurin-21.0.9+10" {
                let info = result?;
                ensure!(
                    info.url == Some(format!("https://example.invalid/{label}/jdk.{kind}")),
                    "Java target URL changed"
                );
                ensure!(
                    info.checksum == Some(format!("sha256:{}", "a".repeat(64))),
                    "Java checksum metadata changed"
                );
            } else {
                ensure!(
                    result.is_err(),
                    "Java accepted unavailable or unsupported metadata"
                );
            }
        }
    }
    Ok(())
}
