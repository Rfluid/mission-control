//! Page events (`POST /api/missions/{id}/events`): answers and button clicks from the mission page.
//!
//! Auth is the mission's own token in `X-Mission-Token` (a custom header forces a CORS preflight, which
//! the daemon never answers with CORS headers). The page names an action by id; what it does comes from
//! the resolved config, never from the request.

use super::App;
use super::config::{Overlay, resolve};
use super::handlers::{Res, found, internal};
use crate::ids::now_ms;
use crate::model::{Op, PageEvent, Record};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Request body limit for page events (set on the route).
pub const MAX_BODY_BYTES: usize = 16 * 1024;
/// One answer or prompt text.
const MAX_TEXT_BYTES: usize = 8 * 1024;
/// Page events per mission per window; more is a runaway script, not a person clicking.
const RATE_MAX: usize = 60;
const RATE_WINDOW: Duration = Duration::from_secs(60);

pub(super) fn bad(msg: impl Into<String>) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg.into())
}

impl App {
    /// Counts one page event for `mission-control`; false once it is over [`RATE_MAX`] per [`RATE_WINDOW`].
    fn allow(&self, mission: &str) -> bool {
        let mut rate = self.rate.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let q = rate.entry(mission.to_string()).or_default();
        let now = Instant::now();
        while q.front().is_some_and(|t| now.duration_since(*t) > RATE_WINDOW) {
            q.pop_front();
        }
        if q.len() >= RATE_MAX {
            return false;
        }
        q.push_back(now);
        true
    }
}

pub async fn event(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(ev): Json<PageEvent>,
) -> Res<Json<Value>> {
    let (mission, token) = found(&app, &id)?;
    if !super::handlers::mission_token(&headers, &token) {
        return Err((StatusCode::UNAUTHORIZED, "mission token required".into()));
    }
    if !app.allow(&id) {
        return Err((StatusCode::TOO_MANY_REQUESTS, format!("over {RATE_MAX} page events a minute")));
    }
    let mut exit = None;
    let ops = match ev {
        PageEvent::Answer { need, collection, answer } => {
            let collection = collection.unwrap_or_else(|| "needs".into());
            text(&answer, true)?;
            // Only collections the page shows as needs can be answered from it.
            let config = resolve(&app, &mission, &Overlay::None)?.config.clone();
            let answerable = collection == "needs"
                || (config.get("sections").and_then(Value::as_array).into_iter().flatten()).any(|s| {
                    s.get("widget").and_then(Value::as_str) == Some("needs")
                        && s.get("collection").and_then(Value::as_str) == Some(collection.as_str())
                });
            if !answerable {
                return Err(bad(format!("`{collection}` isn't shown as needs on this page")));
            }
            let rec = existing(&app, &id, &collection, &need)?;
            let question = rec.body.get("question").cloned().unwrap_or(Value::Null);
            vec![
                Op::Upsert {
                    collection: collection.clone(),
                    key: need.clone(),
                    state: Some("answered".into()),
                    patch: json!({"answer": answer, "answered_at": now_ms()}),
                },
                Op::Log {
                    at: None,
                    kind: "answer".into(),
                    text: format!("{need}: {answer}"),
                    data: Some(json!({"need": need, "collection": collection, "question": question, "answer": answer})),
                },
            ]
        }
        PageEvent::Action { action, collection, key, text: typed } => {
            if let Some(t) = &typed {
                text(t, false)?;
            }
            let resolved = resolve(&app, &mission, &Overlay::None)?;
            let config = &resolved.config;
            let act = config
                .pointer(&format!("/actions/{action}"))
                .cloned()
                .ok_or_else(|| bad(format!("unknown action `{action}`")))?;
            let row = match (&collection, &key) {
                (Some(c), Some(k)) => Some(existing(&app, &id, c, k)?),
                _ => None,
            };
            if act.get("kind").and_then(Value::as_str) == Some("run") {
                let (ops, code) =
                    super::run::execute(&mission, &action, &act, row.as_ref(), config, &resolved.provenance).await?;
                exit = code;
                ops
            } else {
                action_ops(&action, &act, row.as_ref(), typed.as_deref(), config)?
            }
        }
    };
    let applied = app.store().apply(&id, &ops).map_err(|e| bad(format!("{e:#}")))?;
    let seq = applied.events.last().map(|e| e.seq);
    app.publish(&id, applied, false);
    app.retain(&mission);
    Ok(Json(json!({"ok": true, "seq": seq, "exit": exit})))
}

/// What a configured action does when clicked: ops to apply (always ending in a log the agent can `wait` on).
fn action_ops(id: &str, act: &Value, row: Option<&Record>, typed: Option<&str>, config: &Value) -> Res<Vec<Op>> {
    let kind = act.get("kind").and_then(Value::as_str).unwrap_or("");
    let label = act.get("label").and_then(Value::as_str).unwrap_or(id);
    let row_json = row.map(|r| json!({"collection": r.collection, "key": r.key, "state": r.state, "body": r.body}));
    match kind {
        "ask_agent" => {
            let tpl = act.get("message").and_then(Value::as_str).unwrap_or("");
            let message = fill(tpl, row, config);
            let line = match (message.is_empty(), typed) {
                (false, Some(t)) if !t.trim().is_empty() => format!("{message}\n{t}"),
                (false, _) => message.clone(),
                (true, Some(t)) if !t.trim().is_empty() => t.to_string(),
                _ => format!("{label} clicked"),
            };
            Ok(vec![Op::Log {
                at: None,
                kind: "action".into(),
                text: line,
                data: Some(
                    json!({"action": id, "kind": kind, "label": label, "row": row_json, "message": message, "text": typed}),
                ),
            }])
        }
        "customize" => {
            let t = typed.filter(|t| !t.trim().is_empty()).ok_or_else(|| bad("say what should change"))?;
            Ok(vec![Op::Log {
                at: None,
                kind: "customize".into(),
                text: t.to_string(),
                data: Some(json!({"action": id, "text": t, "row": row_json})),
            }])
        }
        "set" => {
            let r = row.ok_or_else(|| bad(format!("action `{id}` needs a row")))?;
            let state = act.get("state").and_then(Value::as_str).map(String::from);
            let field = act.get("field").and_then(Value::as_str);
            let patch = match field {
                Some(f) => json!({ f: act.get("value").cloned().unwrap_or(Value::Null) }),
                None => json!({}),
            };
            if state.is_none() && field.is_none() {
                return Err(bad(format!("action `{id}` sets neither `state` nor `field`")));
            }
            let what = state.clone().unwrap_or_else(|| field.unwrap_or_default().to_string());
            Ok(vec![
                Op::Upsert {
                    collection: r.collection.clone(),
                    key: r.key.clone(),
                    state: state.clone(),
                    patch: patch.clone(),
                },
                Op::Log {
                    at: None,
                    kind: "action".into(),
                    text: format!("{label}: {} {} → {what}", r.collection, r.key),
                    data: Some(
                        json!({"action": id, "kind": kind, "label": label, "row": row_json, "state": state, "patch": patch}),
                    ),
                },
            ])
        }
        "open" | "copy" => Err(bad(format!("`{kind}` actions run in the page"))),
        "answer" => Err(bad("answer a need with kind `answer`")),
        other => Err(bad(format!("unknown action kind `{other}`"))),
    }
}

/// `{row.<field>}`, `{row.key}`, `{row.state}`, `{vars.<name>}` in an action message.
pub(super) fn fill(tpl: &str, row: Option<&Record>, config: &Value) -> String {
    let mut out = String::new();
    let mut rest = tpl;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('}') else {
            out.push_str(&rest[i..]);
            return out;
        };
        let name = &rest[i + 1..i + j];
        let v = if let Some(f) = name.strip_prefix("row.") {
            row.map(|r| match f {
                "key" => Value::String(r.key.clone()),
                "state" => r.state.clone().map_or(Value::Null, Value::String),
                _ => r.body.get(f).cloned().unwrap_or(Value::Null),
            })
        } else if let Some(v) = name.strip_prefix("vars.") {
            config.pointer(&format!("/vars/{v}")).cloned()
        } else {
            None
        };
        match v {
            Some(Value::String(s)) => out.push_str(&s),
            Some(Value::Null) | None if !(name.starts_with("row.") || name.starts_with("vars.")) => {
                out.push_str(&rest[i..=i + j]);
            }
            Some(Value::Null) | None => {}
            Some(v) => out.push_str(&v.to_string()),
        }
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    out
}

fn text(t: &str, required: bool) -> Res<()> {
    if required && t.trim().is_empty() {
        return Err(bad("empty answer"));
    }
    if t.len() > MAX_TEXT_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("text is {} bytes; the limit is {MAX_TEXT_BYTES}", t.len()),
        ));
    }
    Ok(())
}

fn existing(app: &App, id: &str, collection: &str, key: &str) -> Res<Record> {
    app.store()
        .record(id, collection, key)
        .map_err(internal)?
        .ok_or((StatusCode::NOT_FOUND, format!("no record `{collection}/{key}`")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_fill_from_the_row_and_vars() {
        let row = Record {
            collection: "prs".into(),
            key: "17".into(),
            state: Some("open".into()),
            body: json!({"title": "T", "n": 3}),
            updated: 0,
        };
        let config = json!({"vars": {"repo": "a/b"}});
        let got = fill(
            "Re-run {row.key} ({row.title}, {row.n}, {row.state}) in {vars.repo} {row.nope}{x}",
            Some(&row),
            &config,
        );
        assert_eq!(got, "Re-run 17 (T, 3, open) in a/b {x}");
        assert_eq!(fill("open {", None, &config), "open {");
    }
}
