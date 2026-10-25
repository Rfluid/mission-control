//! Config routes: explain, patch any layer (validated; `dry_run` previews and leaves a draft the page can
//! show), undo and history. User and project files are written by [`crate::layerfile`]; the mission layer is
//! versioned in the store. Every change reaches the open pages it affects.

use super::App;
use super::handlers::{Res, admin, found, internal};
pub(super) use super::resolve::{Overlay, resolve, stack};
use crate::config::{LayerKind, Severity};
use crate::ids::now_ms;
use crate::layerfile::{LayerFile, patch_toml};
use crate::model::Mission;
use crate::store::Applied;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Target {
    #[default]
    Mission,
    User,
    Project,
}

/// The file behind a user or project layer.
fn layer_file(app: &App, m: &Mission, target: Target) -> Res<LayerFile> {
    let path = match target {
        Target::User => {
            app.user_dir.clone().ok_or_else(|| bad("no user config dir on this machine"))?.join("config.toml")
        }
        Target::Project => {
            let dir = m.project_dir.clone().ok_or_else(|| bad("this mission has no project dir"))?;
            PathBuf::from(dir).join(".mission-control").join("config.toml")
        }
        Target::Mission => return Err(bad("the mission layer isn't a file")),
    };
    Ok(LayerFile::new(path, &app.state_dir))
}

fn bad(msg: impl Into<String>) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg.into())
}

fn kind_of(t: Target) -> LayerKind {
    match t {
        Target::Mission => LayerKind::Mission,
        Target::User => LayerKind::User,
        Target::Project => LayerKind::Project,
    }
}

pub async fn explain(State(app): State<Arc<App>>, Path(id): Path<String>) -> Res<Json<Value>> {
    let (m, _) = found(&app, &id)?;
    let r = resolve(&app, &m, &Overlay::None)?;
    Ok(Json(serde_json::to_value(r).map_err(internal)?))
}

#[derive(Deserialize)]
pub struct ConfigPatch {
    patch: Value,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    layer: Target,
    /// Check and preview only: nothing is stored; returns the resolved result and a draft id.
    #[serde(default)]
    dry_run: bool,
}

/// Patches a layer. Refused if the result has validation errors (the mission layer's own rules included).
pub async fn patch(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<ConfigPatch>,
) -> Res<Json<Value>> {
    admin(&app, &headers)?;
    let (m, _) = found(&app, &id)?;
    if !req.patch.is_object() {
        return Err(bad("config patch must be a table"));
    }
    let (over, file) = match req.layer {
        Target::Mission => (Overlay::Mission(req.patch.clone()), None),
        t => {
            let f = layer_file(&app, &m, t)?;
            let text = patch_toml(&f.read().map_err(internal)?, &req.patch).map_err(|e| bad(format!("{e:#}")))?;
            let doc = crate::config::parse(&text, "patched file").map_err(|e| bad(format!("{e:#}")))?;
            let label = format!("{} ({})", if t == Target::User { "user" } else { "project" }, f.path.display());
            (Overlay::File(kind_of(t), label, doc), Some((f, text)))
        }
    };
    let r = resolve(&app, &m, &over)?;
    let errors: Vec<_> = r.issues.iter().filter(|i| i.severity == Severity::Error).collect();
    if !errors.is_empty() {
        return Err((StatusCode::BAD_REQUEST, serde_json::to_string(&errors).map_err(internal)?));
    }
    if req.dry_run {
        let draft = app.save_draft(&id, over);
        let preview = format!("http://127.0.0.1:{}/m/{id}?draft={draft}", app.port);
        let text = file.map(|(_, t)| t);
        return Ok(Json(json!({"dry_run": true, "draft": draft, "preview_url": preview, "config": r.config,
                              "issues": r.issues, "file_text": text})));
    }
    let out = match file {
        None => {
            let (version, ev) = app.store().patch_layer(&id, &req.patch, req.note.as_deref()).map_err(internal)?;
            app.cache.forget(&id);
            app.publish(&id, Applied { events: vec![ev], ..Default::default() }, true);
            app.retain(&m);
            json!({"layer": "mission", "version": version, "issues": r.issues})
        }
        Some((f, text)) => {
            let changed = f.write(&text, req.note.as_deref()).map_err(internal)?;
            if changed {
                let what = format!("{} config{}", layer_name(req.layer), note(req.note.as_deref()));
                log_config(&app, &m, &what, &json!({"file": f.path, "patch": req.patch}))?;
                app.config_changed(req.layer, &m);
            }
            json!({"layer": layer_name(req.layer), "file": f.path, "changed": changed, "issues": r.issues})
        }
    };
    Ok(Json(out))
}

fn layer_name(t: Target) -> &'static str {
    match t {
        Target::Mission => "mission",
        Target::User => "user",
        Target::Project => "project",
    }
}

fn note(n: Option<&str>) -> String {
    n.map(|n| format!(": {n}")).unwrap_or_default()
}

/// Logs a file-layer change on the mission it came through, and re-renders its page.
fn log_config(app: &App, m: &Mission, text: &str, data: &Value) -> Res<()> {
    let op = crate::model::Op::Log { at: None, kind: "config".into(), text: text.into(), data: Some(data.clone()) };
    let applied = app.store().apply(&m.id, &[op]).map_err(internal)?;
    app.publish(&m.id, applied, true);
    Ok(())
}

#[derive(Deserialize)]
pub struct LayerReq {
    #[serde(default)]
    layer: Target,
}

/// Undoes the last change to a layer: drops the newest mission version, or restores a file's previous content.
pub async fn undo(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<LayerReq>,
) -> Res<Json<Value>> {
    admin(&app, &headers)?;
    let (m, _) = found(&app, &id)?;
    let out = match req.layer {
        Target::Mission => {
            let undone = app.store().undo_layer(&id).map_err(internal)?;
            app.cache.forget(&id);
            let (version, patch, note) = undone.ok_or_else(|| bad("nothing to undo in the mission layer"))?;
            log_config(&app, &m, &format!("undo config v{version}{}", note_of(note.as_deref())), &patch)?;
            json!({"layer": "mission", "undone": version})
        }
        t => {
            let f = layer_file(&app, &m, t)?;
            let snap =
                f.undo().map_err(internal)?.ok_or_else(|| bad(format!("nothing to undo in {}", f.path.display())))?;
            let text = format!("undo {} config{}", layer_name(t), note_of(snap.note.as_deref()));
            log_config(&app, &m, &text, &json!({"file": f.path}))?;
            app.config_changed(t, &m);
            json!({"layer": layer_name(t), "file": f.path, "restored_from": snap.at})
        }
    };
    Ok(Json(out))
}

fn note_of(n: Option<&str>) -> String {
    note(n.filter(|n| !n.is_empty()))
}

/// The layer's changes, newest first.
pub async fn history(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Query(req): Query<LayerReq>,
) -> Res<Json<Value>> {
    let (m, _) = found(&app, &id)?;
    let out = match req.layer {
        Target::Mission => {
            let mut v = app.store().layer_history(&id).map_err(internal)?;
            v.reverse();
            json!({"layer": "mission", "versions": v})
        }
        t => {
            let f = layer_file(&app, &m, t)?;
            json!({"layer": layer_name(t), "file": f.path, "versions": f.history(), "now": now_ms()})
        }
    };
    Ok(Json(out))
}
