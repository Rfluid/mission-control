//! `template_save`: the mission's own customizations (its mission-layer patches, composed into one) saved as
//! a template file in the user or project config, built on the mission's template with `extends`.

use super::App;
use super::config::{Target, stack};
use super::handlers::{Res, admin, found, internal};
use crate::config::{Layer, LayerKind, Severity, compose};
use crate::layerfile::LayerFile;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct SaveTemplate {
    /// `kebab-case` file name: `templates/<name>.toml`.
    name: String,
    /// `user` (default) or `project`.
    #[serde(default)]
    layer: Option<Target>,
    description: Option<String>,
    /// Overwrite an existing file of that name.
    #[serde(default)]
    replace: bool,
}

fn bad(msg: impl Into<String>) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg.into())
}

pub async fn save(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<SaveTemplate>,
) -> Res<Json<Value>> {
    admin(&app, &headers)?;
    let (m, _) = found(&app, &id)?;
    if req.name.is_empty() || !req.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(bad("template name: letters, digits, `-` and `_` only"));
    }
    let layer = req.layer.unwrap_or(Target::User);
    let dir = match layer {
        Target::User => app.user_dir.clone().ok_or_else(|| bad("no user config dir on this machine"))?,
        Target::Project => PathBuf::from(m.project_dir.clone().ok_or_else(|| bad("this mission has no project dir"))?)
            .join(".mission-control"),
        Target::Mission => return Err(bad("templates are saved to `user` or `project`")),
    };
    // Compose every mission-layer patch into one document; the base template becomes `extends`.
    let mut doc = Value::Object(Map::new());
    for (_, patch) in app.store().layers(&id).map_err(internal)? {
        compose(&mut doc, &patch);
    }
    let base = super::config::resolve(&app, &m, &super::config::Overlay::None)?.template.clone();
    let obj = doc.as_object_mut().ok_or_else(|| internal("composed patch isn't a table"))?;
    if let Some(Value::Object(mission)) = obj.get_mut("mission") {
        mission.remove("template");
        mission.remove("title");
    }
    if obj.get("mission").and_then(Value::as_object).is_some_and(Map::is_empty) {
        obj.remove("mission");
    }
    let mut head = Map::new();
    head.insert("name".into(), json!(req.name));
    if let Some(d) = &req.description {
        head.insert("description".into(), json!(d));
    }
    if base != req.name {
        head.insert("extends".into(), json!(base));
    }
    head.extend(std::mem::take(obj));
    let mut doc = Value::Object(head);
    drop_nulls(&mut doc);

    // Check it: a mission on the new template must resolve without errors.
    let file = LayerFile::new(dir.join("templates").join(format!("{}.toml", req.name)), &app.state_dir);
    if !req.replace && file.path.exists() {
        return Err((StatusCode::CONFLICT, format!("{} exists; pass replace to overwrite it", file.path.display())));
    }
    let kind = if layer == Target::User { LayerKind::User } else { LayerKind::Project };
    let mut s = (*stack(&app, m.project_dir.as_deref()).map_err(internal)?).clone();
    s.set_template(&req.name, kind, file.path.display().to_string(), doc.clone());
    let probe = Layer::new(LayerKind::Mission, "probe", json!({"mission": {"template": req.name}}));
    let r = s.resolve(&[probe]).map_err(|e| bad(format!("{e:#}")))?;
    let errors: Vec<_> = r.issues.iter().filter(|i| i.severity == Severity::Error).collect();
    if !errors.is_empty() {
        return Err((StatusCode::BAD_REQUEST, serde_json::to_string(&errors).map_err(internal)?));
    }
    let text = toml::to_string(&doc).map_err(|e| internal(format!("writing TOML: {e}")))?;
    let header = format!("# Saved from mission {} ({}) by `template_save`.\n", m.id, m.title.replace('\n', " "));
    file.write(&format!("{header}{text}"), Some(&format!("template_save {}", req.name))).map_err(internal)?;
    Ok(Json(json!({"template": req.name, "file": file.path, "extends": (base != req.name).then_some(base),
                   "next": format!("Start a mission from it with template \"{}\".", req.name)})))
}

/// TOML has no null: a JSON patch's `null` (delete) means nothing in a template written from scratch.
fn drop_nulls(v: &mut Value) {
    match v {
        Value::Object(o) => {
            o.retain(|_, x| !x.is_null());
            o.values_mut().for_each(drop_nulls);
        }
        Value::Array(a) => {
            a.retain(|x| !x.is_null());
            a.iter_mut().for_each(drop_nulls);
        }
        _ => {}
    }
}
