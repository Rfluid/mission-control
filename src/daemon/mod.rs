//! `mission-control serve`: the local daemon. Serves every mission page and the agent API. Binds 127.0.0.1 only.
//!
//! Auth: agent writes carry the admin token (`admin.token` in the state dir) as a bearer token. Browser writes
//! carry the mission's own token in `X-Mission-Token`; a custom header forces a CORS preflight, and the
//! daemon allows no cross-origin requests, so other local pages can't write.

mod assets;
mod brief;
mod cache;
mod config;
mod doctor;
mod handlers;
mod home;
mod host;
mod live;
mod page;
mod resolve;
mod review;
mod run;
mod scripts;
mod templates;
mod update;
mod view;

use crate::ids::now_ms;
use crate::model::DaemonState;
use crate::paths;
use crate::store::Store;
use anyhow::Result;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing_subscriber::EnvFilter;

const DEFAULT_LOG_FILTER: &str = "mission_control=info";
/// How often the daemon checks that `daemon.json` still names it.
const ORPHAN_CHECK: Duration = Duration::from_secs(10);
/// Changes buffered per subscriber; a page further behind gets a `reset` and re-reads the view.
const CHANGES_BUFFER: usize = 256;

pub struct App {
    store: Mutex<Store>,
    admin: String,
    port: u16,
    /// Unix ms at startup; clients compare it with their binary's mtime (see `crate::lifecycle`).
    started: i64,
    /// The user config layer's directory (injected so tests never read the real one).
    user_dir: Option<PathBuf>,
    /// Every change, to open pages (SSE) and waiting agents.
    tx: broadcast::Sender<live::Change>,
    /// Recent page events per mission, for the rate limit.
    rate: Mutex<HashMap<String, VecDeque<Instant>>>,
    /// Where file-layer history goes (the state dir).
    state_dir: PathBuf,
    /// Dry-run config changes the page can preview, by draft id.
    drafts: Mutex<HashMap<String, resolve::Draft>>,
    /// Resolved configs until their files or mission layer change.
    cache: cache::Cache,
    /// The update settings and the cached release check.
    update: crate::update::Files,
}

impl App {
    #[must_use]
    pub fn new(
        store: Store,
        admin_token: String,
        port: u16,
        user_dir: Option<PathBuf>,
        state_dir: PathBuf,
    ) -> Arc<Self> {
        let (tx, _) = broadcast::channel(CHANGES_BUFFER);
        let update = crate::update::Files {
            config: user_dir.clone().unwrap_or_default().join("config.toml"),
            cache: state_dir.join("update.json"),
        };
        Arc::new(Self {
            store: Mutex::new(store),
            admin: admin_token,
            port,
            started: now_ms(),
            user_dir,
            tx,
            rate: Mutex::default(),
            state_dir,
            drafts: Mutex::default(),
            cache: cache::Cache::default(),
            update,
        })
    }

    fn store(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/", get(assets::index))
        .route("/m/{id}", get(view::page))
        .route("/m/{id}/{page}", get(view::page_at))
        .route("/m/{id}/widgets/{file}", get(scripts::widget))
        .route("/assets/{file}", get(assets::asset))
        .route("/assets/fonts/{file}", get(assets::font))
        .route("/assets/vendor/{file}", get(assets::vendor))
        .route("/api/missions", get(handlers::list).post(handlers::create))
        .route("/api/doctor", get(doctor::doctor))
        .route("/api/update", get(update::info))
        .route("/api/missions/{id}/update/dismiss", post(update::dismiss))
        .route("/api/missions/{id}", get(view::view))
        .route("/api/missions/{id}/attach", post(handlers::attach))
        .route("/api/missions/{id}/archive", post(handlers::archive))
        .route("/api/missions/{id}/fork", post(handlers::fork))
        .route("/api/missions/{id}/export", get(view::export))
        .route("/api/missions/{id}/ops", post(handlers::ops))
        .route("/api/missions/{id}/config", get(config::explain).post(config::patch))
        .route("/api/missions/{id}/config/undo", post(config::undo))
        .route("/api/missions/{id}/config/history", get(config::history))
        .route("/api/missions/{id}/templates", post(templates::save))
        .route(
            "/api/missions/{id}/events",
            get(live::older).post(page::event).layer(DefaultBodyLimit::max(page::MAX_BODY_BYTES)),
        )
        .route("/api/missions/{id}/stream", get(live::stream))
        .route("/api/missions/{id}/wait", get(live::wait))
        .route("/api/missions/{id}/brief", get(brief::brief))
        .layer(axum::middleware::from_fn_with_state(app.clone(), host::guard))
        .with_state(app)
}

/// Opens the store in the state dir, binds `127.0.0.1:port`, writes `daemon.json`, serves until stopped.
pub async fn serve(port: u16) -> Result<()> {
    init_logging();
    let state = paths::state_dir()?;
    let store = Store::open(&state.join("mission.db"))?;
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    let port = listener.local_addr()?.port();
    let app = App::new(store, paths::admin_token()?, port, paths::user_config_dir(), state);
    let pid = std::process::id();
    let version = env!("CARGO_PKG_VERSION");
    paths::write_daemon_state(&DaemonState { port, pid, version: version.into(), started: app.started })?;
    tracing::info!(port, pid, version, "listening on 127.0.0.1");
    // `mission-control stop`: SIGTERM on Unix. Windows has no SIGTERM (stop kills the process there); Ctrl-Break
    // still reaches a daemon run in a console.
    #[cfg(unix)]
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    #[cfg(windows)]
    let mut term = tokio::signal::windows::ctrl_break()?;
    tokio::spawn(update::checks(app.update.clone()));
    tokio::select! {
        r = axum::serve(listener, router(app)) => r?,
        () = orphaned(pid) => tracing::info!("daemon.json no longer names this process (state dir gone, or replaced); stopping"),
        _ = term.recv() => tracing::info!("terminated, stopping"),
        _ = tokio::signal::ctrl_c() => tracing::info!("interrupted, stopping"),
    }
    // Don't remove a newer daemon's state file.
    if paths::read_daemon_state().is_some_and(|s| s.pid == pid) {
        paths::remove_daemon_state()?;
    }
    Ok(())
}

/// Resolves once `daemon.json` stops naming `pid`: the state dir was deleted (tests, a user's cleanup) or
/// another daemon took over. A daemon nobody can find shouldn't keep running.
async fn orphaned(pid: u32) {
    let mut tick = tokio::time::interval(ORPHAN_CHECK);
    tick.tick().await;
    loop {
        tick.tick().await;
        let path = paths::state_dir_path().join("daemon.json");
        let mine = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<DaemonState>(&b).ok())
            .is_some_and(|s| s.pid == pid);
        if !mine {
            return;
        }
    }
}

/// Logs to stderr (`daemon.log` when auto-started), or appends to `$MISSION_CONTROL_LOG_FILE` when set (the Windows
/// auto-start can't redirect stderr). `RUST_LOG` overrides the default filter.
fn init_logging() {
    use tracing_subscriber::fmt::writer::BoxMakeWriter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));
    let file = std::env::var_os("MISSION_CONTROL_LOG_FILE")
        .and_then(|p| std::fs::OpenOptions::new().create(true).append(true).open(p).ok());
    let writer = match file {
        Some(f) => BoxMakeWriter::new(std::sync::Mutex::new(f)),
        None => BoxMakeWriter::new(std::io::stderr),
    };
    // Already set (tests, embedding): keep the existing subscriber.
    let _ = tracing_subscriber::fmt().with_env_filter(filter).with_ansi(false).with_writer(writer).try_init();
}

#[cfg(test)]
mod tests;
