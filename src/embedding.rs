//! Unstable Oyzu embedding boundary. One context per fresh worker process.
//! The frontend owns configuration, locks, receipts, admission and isolation.
use crate::config::{Config, settings};
use eyre::{Result, ensure};
pub use mise_util::embedding::{HttpFuture, HttpRequest, HttpTransport};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

pub struct Options {
    /// Operation-private roots, never an ambient mise cache or project directory.
    pub state: PathBuf,
    /// The verified Oyzu image used by the worker's parent.
    pub frontend: PathBuf,
    /// Explicit core identifiers admitted by the supervisor (short names).
    pub tools: BTreeSet<String>,
    /// Absent for local operations. No transport means fail closed, not direct HTTP.
    pub transport: Option<Arc<HttpTransport>>,
}

pub struct Session {
    config: Arc<Config>,
    tools: BTreeSet<String>,
}

pub use crate::plugins::core::NodeArchiveFacts;
pub use crate::plugins::core::NodeArchiveMetadata;
pub use crate::plugins::core::{GoArchiveFacts, GoArchiveMetadata};
static STARTED: AtomicBool = AtomicBool::new(false);
static SETTINGS: OnceLock<Arc<settings::Settings>> = OnceLock::new();

impl Session {
    /// Call before any mise API or application-created thread. The parent must
    /// construct a minimal environment; ambient mise overrides are rejected.
    /// Failure consumes the process context. Discard the worker rather than retry.
    pub fn initialize(options: Options) -> Result<Self> {
        ensure!(
            !STARTED.swap(true, Ordering::SeqCst),
            "embedding worker cannot be reused"
        );
        ensure!(
            !settings::is_loaded() && !crate::config::is_loaded(),
            "mise was initialized before embedding context"
        );
        ensure!(
            options.state.is_absolute() && options.frontend.is_absolute(),
            "embedding paths must be absolute"
        );
        for (key, _) in std::env::vars_os() {
            let key = key.to_string_lossy().to_ascii_uppercase();
            // Native macOS conformance observes this text-encoding variable in
            // a child started with env_clear. It is not a mise configuration
            // input; retain the strict allowlist on every other platform.
            ensure!(
                matches!(
                    key.as_str(),
                    "PATH"
                        | "SYSTEMROOT"
                        | "WINDIR"
                        | "COMSPEC"
                        | "PATHEXT"
                        | "HOME"
                        | "USERPROFILE"
                        | "TEMP"
                        | "TMP"
                        | "LANG"
                        | "LC_ALL"
                        | "TERM"
                        | "RUST_BACKTRACE"
                ) || (cfg!(target_os = "macos") && key == "__CF_USER_TEXT_ENCODING"),
                "embedding worker inherited an unapproved environment variable"
            );
        }
        ensure!(
            options
                .tools
                .iter()
                .all(|tool| matches!(tool.as_str(), "node" | "go" | "java" | "python")),
            "embedding tool is not admitted"
        );
        mise_util::embedding::initialize(mise_util::embedding::Context {
            state: options.state.clone(),
            frontend: options.frontend.clone(),
            transport: options.transport,
        })?;
        ensure!(
            *crate::env::HOME == options.state.join("home"),
            "home was initialized before embedding context"
        );
        ensure!(
            *crate::env::MISE_DATA_DIR == options.state.join("data"),
            "data root was initialized before embedding context"
        );
        ensure!(
            *crate::env::MISE_CACHE_DIR == options.state.join("cache"),
            "cache root was initialized before embedding context"
        );
        *crate::env::ARGS.write().unwrap() = vec![options.frontend.to_string_lossy().into_owned()];
        let mut defaults = (*settings::load_defaults()?).clone();
        defaults.auto_install = false;
        defaults.registry_floating = false;
        // The supervisor authorizes publisher metadata through its broker;
        // never silently substitute the public mise aggregation service.
        defaults.use_versions_host = false;
        defaults.lockfile = Some(false);
        let tools = options.tools;
        defaults.enable_tools = Some(tools.clone());
        defaults.disable_backends = vec!["asdf".into(), "vfox".into()];
        defaults.node.compile = Some(false);
        defaults.node.corepack = false;
        defaults.node.npm_shim = false;
        defaults.node.default_packages_file = Some(options.state.join("no-default-node-packages"));
        defaults.go.default_packages_file = options.state.join("no-default-go-packages");
        defaults.python.compile = Some(false);
        defaults.python.default_packages_file =
            Some(options.state.join("no-default-python-packages"));
        let defaults = Arc::new(defaults);
        SETTINGS
            .set(defaults.clone())
            .map_err(|_| eyre::eyre!("embedding settings already initialized"))?;
        settings::set_loader(|| {
            SETTINGS
                .get()
                .cloned()
                .ok_or_else(|| eyre::eyre!("embedding settings absent"))
        });
        settings::store(defaults);
        crate::register_util_hooks();
        crate::frontend::register(crate::frontend::Frontend {
            lockfiles_after_install: |_, _| {
                Box::pin(async { eyre::bail!("embedding frontend owns lockfile publication") })
            },
            subcommand_names: Vec::new,
        });
        Ok(Self {
            config: Config::for_embedding()?,
            tools,
        })
    }

    pub fn config(&self) -> &Arc<Config> {
        &self.config
    }

    /// Resolve an admitted Node request against supplied/cached metadata, using
    /// upstream aliases, prefix matching and npm range semantics. All native
    /// constraints must match. Returns a cataloged canonical stable version,
    /// never an installed/path/ref fallback. Platform/archive verification and
    /// policy remain separate. Metadata transport and private cache may be used.
    pub async fn resolve_node_version(
        &self,
        request: &str,
        native_constraints: &[String],
    ) -> Result<String> {
        ensure!(self.tools.contains("node"), "Node backend is not admitted");
        crate::plugins::core::resolve_node_version(&self.config, request, native_constraints).await
    }

    /// Resolve Go from the admitted upstream tag catalog with all constraints.
    /// Embedded Go uses bounded supplied HTTP pagination, never git execution.
    /// Only cataloged canonical stable versions are returned; archive availability
    /// and publisher verification remain separate operations.
    pub async fn resolve_go_version(
        &self,
        request: &str,
        native_constraints: &[String],
    ) -> Result<String> {
        ensure!(self.tools.contains("go"), "Go backend is not admitted");
        crate::plugins::core::resolve_go_version(&self.config, request, native_constraints).await
    }

    /// Project names from this revision's baked registry onto admitted core
    /// backends. This is not version resolution or permission to install. No
    /// floating registry, ambient aliases, filesystem or network is consulted.
    pub fn tool_aliases(&self) -> Result<BTreeMap<String, String>> {
        let mut aliases = BTreeMap::new();
        for short in &self.tools {
            let tool = crate::registry::baked_registry()
                .get(short)
                .ok_or_else(|| eyre::eyre!("admitted tool absent from pinned registry"))?;
            let canonical = format!("core:{short}");
            ensure!(
                tool.short == short && tool.backends.iter().any(|b| b.full == canonical),
                "pinned registry no longer supplies admitted core backend"
            );
            for alias in std::iter::once(short.as_str())
                .chain(tool.aliases.iter().copied())
                .chain(std::iter::once(canonical.as_str()))
            {
                ensure!(
                    !alias.contains(':') || alias == canonical,
                    "registry alias cannot rebind a canonical backend"
                );
                if let Some(previous) = aliases.insert(alias.to_owned(), canonical.clone()) {
                    ensure!(previous == canonical, "ambiguous pinned registry alias");
                }
            }
        }
        Ok(aliases)
    }

    /// Compute facts for an exact stable version without acquisition,
    /// installation, subprocesses or executing the target. Upstream may inspect
    /// host platform metadata. Availability and authenticity are not implied.
    /// Only the initial three target tuples are exposed here.
    pub fn node_archive_facts(&self, version: &str, target: &str) -> Result<NodeArchiveFacts> {
        let target_platform = self.archive_target("node", version, target)?;
        crate::plugins::core::node_archive_facts(version, &target_platform, target)
    }

    /// Require a unique, syntactically valid publisher-declared SHA-256 for the
    /// exact target archive, through supplied transport/private metadata cache.
    /// No archive bytes are acquired and no signature or source trust is verified.
    pub async fn node_archive_metadata(
        &self,
        version: &str,
        target: &str,
    ) -> Result<NodeArchiveMetadata> {
        let platform = self.archive_target("node", version, target)?;
        crate::plugins::core::node_archive_metadata(version, &platform, target).await
    }

    /// Compute target-aware Go archive facts without Git discovery, acquisition,
    /// installation or execution. The caller must independently verify catalog
    /// membership, exact bytes/checksum and admitted layout before installation.
    pub fn go_archive_facts(&self, version: &str, target: &str) -> Result<GoArchiveFacts> {
        let target_platform = self.archive_target("go", version, target)?;
        Ok(crate::plugins::core::go_archive_facts(
            version,
            &target_platform,
            target,
        ))
    }
    /// Fetch the target's declared SHA-256 through the supplied transport.
    /// Reject malformed metadata; never fetch archive bytes or execute a target.
    /// This does not prove catalog membership, publisher identity or byte integrity.
    pub async fn go_archive_metadata(
        &self,
        version: &str,
        target: &str,
    ) -> Result<GoArchiveMetadata> {
        let platform = self.archive_target("go", version, target)?;
        crate::plugins::core::go_archive_metadata(version, &platform, target).await
    }

    fn archive_target(
        &self,
        tool: &str,
        version: &str,
        target: &str,
    ) -> Result<crate::backend::platform_target::PlatformTarget> {
        ensure!(
            version.len() <= 128 && target.len() <= 64,
            "archive plan input exceeds limit"
        );
        ensure!(
            self.tools.contains(tool),
            "backend is not admitted in this embedding session"
        );
        let parsed = semver::Version::parse(version)?;
        ensure!(
            parsed.pre.is_empty() && parsed.build.is_empty() && parsed.to_string() == version,
            "archive facts require an exact stable version"
        );
        let platform = match target {
            "linux/amd64/gnu" => "linux-x64",
            "darwin/arm64/native" => "macos-arm64",
            "windows/amd64/msvc" => "windows-x64",
            _ => eyre::bail!("archive target is not supported by the embedding boundary"),
        };
        Ok(crate::backend::platform_target::PlatformTarget::new(
            crate::platform::Platform::parse(platform)?,
        ))
    }
}
