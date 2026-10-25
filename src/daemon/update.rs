//! Update notices for the page: what's new, and dismissing it (one version, or all notices). Reading is
//! open; dismissing needs a mission token (any mission's): it only hides a notice. Ported from redline.

use super::App;
use super::handlers::{Res, found};
use crate::model::UpdateInfo;
use crate::update::{self, Files};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

/// How often the daemon asks whether a release check is due (the check itself runs at most daily).
const UPDATE_TICK: Duration = Duration::from_secs(3600);

pub async fn info(State(app): State<Arc<App>>) -> Json<UpdateInfo> {
    Json(update::info(&app.update))
}

/// `{"version": "0.3.0"}` skips that release; `{"all": true}` turns checks and notices off.
#[derive(Deserialize)]
pub struct Dismiss {
    version: Option<String>,
    #[serde(default)]
    all: bool,
}

pub async fn dismiss(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(d): Json<Dismiss>,
) -> Res<Json<UpdateInfo>> {
    let (_, token) = found(&app, &id)?;
    if !super::handlers::mission_token(&headers, &token) {
        return Err((StatusCode::UNAUTHORIZED, "mission token required".into()));
    }
    let bad = |e: anyhow::Error| (StatusCode::BAD_REQUEST, format!("{e:#}"));
    let u = match (d.all, d.version.as_deref()) {
        (true, _) => update::set_check(&app.update, false).map_err(bad)?,
        (false, Some(v)) => update::skip(&app.update, Some(v)).map_err(bad)?,
        (false, None) => return Err((StatusCode::BAD_REQUEST, "give `version` or `all: true`".into())),
    };
    tracing::info!(version = ?d.version, all = d.all, "update notice dismissed");
    Ok(Json(u))
}

/// Background release checks, at most daily (see [`update::auto_check`]). Never fails the daemon.
pub async fn checks(files: Files) {
    let mut tick = tokio::time::interval(UPDATE_TICK);
    loop {
        tick.tick().await;
        let f = files.clone();
        match tokio::task::spawn_blocking(move || update::auto_check(&f)).await {
            Ok(Some(c)) if c.error.is_some() => tracing::info!(error = ?c.error, "release check failed"),
            Ok(Some(c)) => tracing::info!(latest = ?c.latest, "release check"),
            _ => {}
        }
    }
}
