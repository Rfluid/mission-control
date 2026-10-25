//! `brief`: what an agent needs to pick a mission back up, as text within a character budget: open needs, page
//! events it hasn't received yet (peeked, not consumed), then each `[brief].collections` by state, newest first.
//! Cheaper than `view` for re-orienting after a restart or a long gap.

use super::App;
use super::config::{Overlay, resolve};
use super::handlers::{Res, found, internal};
use super::live::PAGE_KINDS;
use crate::model::Record;
use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

const DEFAULT_MAX_CHARS: usize = 4000;
/// Unseen page events listed at most.
const EVENTS_MAX: i64 = 20;
/// One record's line is cut at this length.
const LINE_MAX: usize = 160;

#[derive(Deserialize)]
pub struct BriefQuery {
    session: Option<String>,
    max_chars: Option<usize>,
}

/// A one-line summary of a record: key, state, and its best title-ish field.
fn line(r: &Record) -> String {
    let title = ["question", "title", "name", "text", "description"]
        .iter()
        .find_map(|f| r.body.get(*f).and_then(Value::as_str))
        .unwrap_or_default();
    let state = r.state.as_deref().map(|s| format!(" [{s}]")).unwrap_or_default();
    let mut l = format!("- {}{state} {title}", r.key);
    if l.chars().count() > LINE_MAX {
        l = l.chars().take(LINE_MAX - 1).collect::<String>() + "…";
    }
    l
}

/// Appends `chunk` if it fits in `max`; returns false once the budget is spent.
fn push(out: &mut String, chunk: &str, max: usize) -> bool {
    if out.chars().count() + chunk.chars().count() > max {
        return false;
    }
    out.push_str(chunk);
    true
}

pub async fn brief(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Query(q): Query<BriefQuery>,
) -> Res<Json<Value>> {
    let (m, _) = found(&app, &id)?;
    let r = resolve(&app, &m, &Overlay::None)?;
    let max = q
        .max_chars
        .or_else(|| r.config.pointer("/brief/max_chars").and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok()))
        .unwrap_or(DEFAULT_MAX_CHARS);
    let colls: Vec<String> = (r.config.pointer("/brief/collections").and_then(Value::as_array).into_iter().flatten())
        .filter_map(|c| c.as_str().map(String::from))
        .collect();
    let session = q.session.unwrap_or_else(|| "default".into());
    let (records, events) = {
        let mut s = app.store();
        let cursor = s.cursor(&id, &session).map_err(internal)?;
        (s.records(&id).map_err(internal)?, s.events_of(&id, cursor, &PAGE_KINDS, EVENTS_MAX).map_err(internal)?)
    };
    let url = format!("http://127.0.0.1:{}/m/{}", app.port, m.id);
    let mut out = String::new();
    let mut truncated = false;
    let _ = writeln!(out, "# {} ({}) · {} · {url}", m.title, m.id, r.template);

    let needs: Vec<&Record> =
        records.iter().filter(|x| x.collection == "needs" && x.state.as_deref() != Some("answered")).collect();
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    sections.push((format!("\n## Waiting on the user ({})\n", needs.len()), needs.iter().map(|x| line(x)).collect()));
    let evs: Vec<String> =
        events.iter().map(|e| format!("- #{} {}: {}", e.seq, e.kind, e.text.replace('\n', " "))).collect();
    sections.push((format!("\n## New from the page, not yet received ({})\n", evs.len()), evs));
    for c in colls.iter().filter(|c| c.as_str() != "needs") {
        let mut rows: Vec<&Record> = records.iter().filter(|x| &x.collection == c).collect();
        rows.sort_by_key(|x| std::cmp::Reverse(x.updated));
        let mut by_state: BTreeMap<&str, usize> = BTreeMap::new();
        for x in &rows {
            *by_state.entry(x.state.as_deref().unwrap_or("-")).or_default() += 1;
        }
        let counts: Vec<String> = by_state.iter().map(|(s, n)| format!("{s} {n}")).collect();
        sections.push((
            format!("\n## {c} ({}: {})\n", rows.len(), counts.join(", ")),
            rows.iter().map(|x| line(x)).collect(),
        ));
    }
    for (head, lines) in sections {
        if !push(&mut out, &head, max) {
            truncated = true;
            break;
        }
        for (i, l) in lines.iter().enumerate() {
            if !push(&mut out, &format!("{l}\n"), max) {
                let more = format!("… and {} more\n", lines.len() - i);
                let _ = push(&mut out, &more, max);
                truncated = true;
                break;
            }
        }
    }
    Ok(Json(json!({"text": out, "chars": out.chars().count(), "truncated": truncated})))
}
