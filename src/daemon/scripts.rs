//! Script widgets: `GET /m/{id}/widgets/{name}.js` serves `widgets/<file>.js` from the user or project
//! config dir, for a widget whose `script` key came from that layer. The mission layer can't set `script`
//! (validation), and provenance is checked again here.

use super::App;
use super::config::{Overlay, resolve};
use super::handlers::{Res, found};
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

/// Biggest script served.
const MAX_SCRIPT_BYTES: u64 = 512 * 1024;

fn not_found(msg: String) -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, msg)
}

pub async fn widget(State(app): State<Arc<App>>, Path((id, file)): Path<(String, String)>) -> Res<Response> {
    let (m, _) = found(&app, &id)?;
    let name = file.strip_suffix(".js").ok_or_else(|| not_found(format!("no script `{file}`")))?;
    let r = resolve(&app, &m, &Overlay::None)?;
    let script = r.config.pointer(&format!("/widgets/{name}/script")).and_then(Value::as_str);
    let script = script.ok_or_else(|| not_found(format!("widget `{name}` has no script")))?;
    let rel = script.strip_prefix("widgets/").filter(|f| {
        std::path::Path::new(f).extension().is_some_and(|e| e.eq_ignore_ascii_case("js"))
            && f.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            && !f.starts_with('.')
    });
    let rel = rel.ok_or_else(|| not_found(format!("`{script}`: scripts live in widgets/<name>.js")))?;
    let label = r.provenance.get(&format!("widgets.{name}.script")).map_or("", String::as_str);
    let dir: PathBuf = if label.starts_with("user") {
        app.user_dir.clone().ok_or_else(|| not_found("no user config dir".into()))?
    } else if label.starts_with("project") {
        PathBuf::from(m.project_dir.clone().ok_or_else(|| not_found("no project dir".into()))?).join(".mission-control")
    } else {
        return Err((StatusCode::FORBIDDEN, format!("widget `{name}`'s script isn't from a user or project file")));
    };
    let path = dir.join("widgets").join(rel);
    let meta = std::fs::metadata(&path).map_err(|_| not_found(format!("{} not found", path.display())))?;
    if meta.len() > MAX_SCRIPT_BYTES {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, format!("{} is over {MAX_SCRIPT_BYTES} bytes", path.display())));
    }
    let body = std::fs::read_to_string(&path).map_err(|e| not_found(format!("{}: {e}", path.display())))?;
    Ok(([(header::CONTENT_TYPE, "text/javascript"), (header::CACHE_CONTROL, "no-cache")], body).into_response())
}
