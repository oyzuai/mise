//! Process-local integration hooks for a single isolated embedding worker.
//! This is not a sandbox: callers must isolate subprocesses and other clients.
use eyre::{Result, ensure, eyre};
use reqwest::{Method, Response, header::HeaderMap};
use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, OnceLock},
};
use url::Url;

pub struct HttpRequest {
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
}

pub type HttpFuture = Pin<Box<dyn Future<Output = Result<Response>> + Send>>;
pub type HttpTransport = dyn Fn(HttpRequest) -> HttpFuture + Send + Sync;

pub struct Context {
    pub state: PathBuf,
    pub frontend: PathBuf,
    pub transport: Option<Arc<HttpTransport>>,
}

static CONTEXT: OnceLock<Context> = OnceLock::new();

pub fn initialize(context: Context) -> Result<()> {
    ensure!(
        context.state.is_absolute() && context.frontend.is_absolute(),
        "embedding paths must be absolute"
    );
    CONTEXT
        .set(context)
        .map_err(|_| eyre!("embedding context is already initialized"))
}

pub fn context() -> Option<&'static Context> {
    CONTEXT.get()
}

pub fn path(key: &str) -> Option<PathBuf> {
    let context = context()?;
    if key == "__MISE_BIN" {
        return Some(context.frontend.clone());
    }
    let relative = match key {
        "XDG_CONFIG_HOME" => "home/.config",
        "XDG_DATA_HOME" => "home/.local/share",
        "XDG_CACHE_HOME" => "home/.cache",
        "XDG_STATE_HOME" => "home/.local/state",
        "MISE_DATA_DIR" => "data",
        "MISE_CACHE_DIR" => "cache",
        "MISE_STATE_DIR" => "state",
        "MISE_CONFIG_DIR" => "config",
        "MISE_SYSTEM_CONFIG_DIR" => "system-config",
        "MISE_SYSTEM_DATA_DIR" => "system-data",
        "MISE_INSTALLS_DIR" => "installs",
        "MISE_DOWNLOADS_DIR" => "downloads",
        "MISE_PLUGINS_DIR" => "plugins",
        "MISE_SHIMS_DIR" => "shims",
        "MISE_TMP_DIR" => "tmp",
        _ => return None,
    };
    Some(context.state.join(relative))
}

pub async fn send(request: HttpRequest) -> Result<Response> {
    let context = context().ok_or_else(|| eyre!("embedding context is absent"))?;
    ensure!(
        matches!(request.url.scheme(), "https" | "http"),
        "embedding URL scheme is not admitted"
    );
    ensure!(
        matches!(request.method, Method::GET | Method::HEAD),
        "embedding HTTP method is not admitted"
    );
    ensure!(
        request.url.username().is_empty() && request.url.password().is_none(),
        "embedding request contains URL credentials"
    );
    for name in request.headers.keys() {
        ensure!(
            matches!(
                name.as_str(),
                "accept"
                    | "accept-encoding"
                    | "user-agent"
                    | "range"
                    | "if-range"
                    | "if-none-match"
                    | "if-modified-since"
            ),
            "embedding request contains an unapproved header"
        );
    }
    let transport = context
        .transport
        .as_ref()
        .ok_or_else(|| eyre!("embedding HTTP transport is not authorized"))?;
    transport(request).await
}
