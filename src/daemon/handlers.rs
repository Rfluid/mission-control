//! HTTP handlers. Routes are listed in `super::router` and documented in `docs/PROTOCOL.md`.

use super::App;
use crate::model::{Attach, Created, Health, Mission, NewMission, Op};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

pub(super) type Res<T> = Result<T, (StatusCode, String)>;

pub(super) fn internal(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

/// Token comparison in constant time (no early exit on the first differing byte).
pub(super) fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The page's mission token, from `X-Mission-Token`.
pub(super) fn mission_token(headers: &HeaderMap, token: &str) -> bool {
    headers.get("x-mission-token").and_then(|v| v.to_str().ok()).is_some_and(|t| same(t, token))
}

pub(super) fn admin(app: &App, headers: &HeaderMap) -> Res<()> {
    let ok = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|t| same(t, &app.admin));
    if ok { Ok(()) } else { Err((StatusCode::UNAUTHORIZED, "admin token required".into())) }
}

pub(super) fn found(app: &App, id: &str) -> Res<(Mission, String)> {
    app.store().mission(id).map_err(internal)?.ok_or((StatusCode::NOT_FOUND, format!("no mission `{id}`")))
}

pub async fn health(State(app): State<Arc<App>>) -> Json<Health> {
    Json(Health {
        name: "mission-control".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        pid: std::process::id(),
        started: app.started,
    })
}

#[derive(Deserialize)]
pub struct ListQuery {
    /// Include archived missions.
    #[serde(default)]
    archived: bool,
    /// Add `summary` (open needs, records, last activity, health) to each.
    #[serde(default)]
    summary: bool,
}

pub async fn list(State(app): State<Arc<App>>, Query(q): Query<ListQuery>) -> Res<Json<Value>> {
    let missions = app.store().missions(q.archived).map_err(internal)?;
    let mut out = Vec::new();
    for m in missions {
        let mut v = serde_json::to_value(&m).map_err(internal)?;
        if q.summary {
            v["summary"] = super::home::summary(&app, &m)?;
        }
        out.push(v);
    }
    Ok(Json(Value::Array(out)))
}

#[derive(Deserialize)]
pub struct ArchiveReq {
    #[serde(default = "yes")]
    archived: bool,
}

fn yes() -> bool {
    true
}

/// Archives a mission (or restores it with `archived: false`).
pub async fn archive(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<ArchiveReq>,
) -> Res<Json<Mission>> {
    admin(&app, &headers)?;
    let m = app.store().set_archived(&id, req.archived).map_err(internal)?;
    m.map(Json).ok_or((StatusCode::NOT_FOUND, format!("no mission `{id}`")))
}

#[derive(Deserialize)]
pub struct ForkReq {
    title: String,
    #[serde(default = "yes")]
    records: bool,
}

/// A new mission with this one's config versions (and records, unless `records: false`).
pub async fn fork(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<ForkReq>,
) -> Res<Json<Created>> {
    admin(&app, &headers)?;
    found(&app, &id)?;
    let m = app.store().fork(&id, &req.title, req.records).map_err(internal)?;
    Ok(Json(created(&app, m)))
}

pub async fn create(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(req): Json<NewMission>,
) -> Res<Json<Created>> {
    admin(&app, &headers)?;
    if let Some(t) = &req.template {
        let stack = super::config::stack(&app, req.project_dir.as_deref()).map_err(internal)?;
        if !stack.templates.contains_key(t) {
            let have = stack.template_names().join(", ");
            return Err((StatusCode::BAD_REQUEST, format!("unknown template `{t}` (have: {have})")));
        }
    }
    if let Some(session) = req.session.as_deref().filter(|_| !req.force)
        && let Some(m) = app.store().find_titled(session, &req.title).map_err(internal)?
    {
        let msg = format!(
            "this session already has mission `{}` titled `{}`: attach to it, or pass force to create another",
            m.id, m.title
        );
        return Err((StatusCode::CONFLICT, msg));
    }
    let mission = app.store().create(&req).map_err(internal)?;
    Ok(Json(created(&app, mission)))
}

fn created(app: &App, mission: Mission) -> Created {
    let url = format!("http://127.0.0.1:{}/m/{}", app.port, mission.id);
    Created { mission, url }
}

/// Re-binds a mission to an agent session (a restarted session keeps its id, records and history).
pub async fn attach(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<Attach>,
) -> Res<Json<Created>> {
    admin(&app, &headers)?;
    let m = app.store().attach(&id, req.session.as_deref()).map_err(internal)?;
    let m = m.ok_or((StatusCode::NOT_FOUND, format!("no mission `{id}`")))?;
    Ok(Json(created(&app, m)))
}

pub async fn ops(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(ops): Json<Vec<Op>>,
) -> Res<Json<Value>> {
    admin(&app, &headers)?;
    let (m, _) = found(&app, &id)?;
    let applied = app.store().apply(&id, &ops).map_err(|e| (StatusCode::BAD_REQUEST, format!("{e:#}")))?;
    app.publish(&id, applied, false);
    app.retain(&m);
    Ok(Json(json!({"applied": ops.len()})))
}
