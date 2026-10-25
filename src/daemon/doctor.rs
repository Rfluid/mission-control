//! `GET /api/doctor`: health of every live mission, for `mission-control doctor`: config issues, collections too big to
//! render well, sections that show nothing, actions nothing uses, missions nobody touched in a while.

use super::App;
use super::config::{Overlay, resolve};
use super::handlers::{Res, internal};
use crate::ids::now_ms;
use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Records in one collection past which pages slow down; `brief` and filters help.
const BIG_COLLECTION: usize = 1000;
/// A mission with no event for this long is idle.
const IDLE_MS: i64 = 7 * 24 * 3600 * 1000;
/// Widgets that show something without records of a collection.
const SELF_FILLED: [&str; 4] = ["feed", "markdown", "health", "html"];

pub async fn doctor(State(app): State<Arc<App>>) -> Res<Json<Value>> {
    let missions = app.store().missions(false).map_err(internal)?;
    let mut out = Vec::new();
    for m in missions {
        let r = resolve(&app, &m, &Overlay::None)?;
        let (records, last) = {
            let mut s = app.store();
            (s.records(&m.id).map_err(internal)?, s.events(&m.id, 0, 1).map_err(internal)?.first().map(|e| e.at))
        };
        let mut sizes: BTreeMap<&str, usize> = BTreeMap::new();
        for rec in &records {
            *sizes.entry(rec.collection.as_str()).or_default() += 1;
        }
        let big: Vec<Value> = sizes
            .iter()
            .filter(|(_, n)| **n > BIG_COLLECTION)
            .map(|(c, n)| json!({"collection": c, "records": n}))
            .collect();
        let sections = r.config.get("sections").and_then(Value::as_array).cloned().unwrap_or_default();
        let empty: Vec<&str> = sections
            .iter()
            .filter(|s| {
                let w = s.get("widget").and_then(Value::as_str).unwrap_or_default();
                let coll = s.get("collection").and_then(Value::as_str);
                !SELF_FILLED.contains(&w) && coll.is_some_and(|c| !sizes.contains_key(c))
            })
            .filter_map(|s| s.get("id").and_then(Value::as_str))
            .collect();
        let mut used: BTreeSet<String> = BTreeSet::new();
        let mut collect = |v: Option<&Value>| {
            for a in v.and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                used.insert(a.to_string());
            }
        };
        collect(r.config.pointer("/toolbar/actions"));
        for s in &sections {
            collect(s.get("actions"));
            collect(s.get("row_actions"));
        }
        for w in r.config.get("widgets").and_then(Value::as_object).into_iter().flatten() {
            collect(w.1.pointer("/props/row_actions"));
        }
        let unused: Vec<&String> = (r.config.get("actions").and_then(Value::as_object).into_iter().flatten())
            .map(|(k, _)| k)
            .filter(|k| !used.contains(*k) && !matches!(k.as_str(), "ask" | "customize"))
            .collect();
        let idle = last.is_none_or(|at| now_ms() - at > IDLE_MS);
        out.push(json!({
            "id": m.id, "title": m.title, "template": r.template, "issues": r.issues, "big_collections": big,
            "empty_sections": empty, "unused_actions": unused, "idle": idle, "last_event": last,
            "records": records.len(),
        }));
    }
    Ok(Json(Value::Array(out)))
}
