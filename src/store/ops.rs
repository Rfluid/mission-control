//! Typed writes: a batch of ops in one transaction, returning what changed (for live pages and `wait`).

use super::Store;
use super::rows::{EventRow, RecordRow};
use super::schema::{events, records};
use crate::ids::now_ms;
use crate::model::{Event, Op, Record, RecordKey};
use anyhow::{Result, bail};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use serde::Serialize;
use serde_json::{Value, json};

/// Longest collection name or record key accepted.
const MAX_NAME: usize = 200;

/// What a batch changed: records as they are now, records removed, events appended.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Applied {
    pub records: Vec<Record>,
    pub removed: Vec<RecordKey>,
    pub events: Vec<Event>,
}

impl Applied {
    pub fn is_empty(&self) -> bool {
        self.records.is_empty() && self.removed.is_empty() && self.events.is_empty()
    }
}

impl Store {
    /// Applies `ops` in one transaction: all or nothing.
    pub fn apply(&mut self, mission: &str, ops: &[Op]) -> Result<Applied> {
        self.conn.transaction(|c| apply_all(c, mission, ops))
    }
}

/// Applies `ops` on `c` (the caller owns the transaction).
pub(super) fn apply_all(c: &mut SqliteConnection, mission: &str, ops: &[Op]) -> Result<Applied> {
    let mut out = Applied::default();
    for op in ops {
        match op {
            Op::Upsert { collection, key, state, patch } => {
                let before = record(c, mission, collection, key)?;
                let r = upsert(c, mission, collection, key, state.as_deref(), patch)?;
                if let Some(ev) = state_change(c, mission, before.as_ref(), &r)? {
                    out.events.push(ev);
                }
                out.removed.retain(|k| !(k.collection == r.collection && k.key == r.key));
                out.records.retain(|x| !(x.collection == r.collection && x.key == r.key));
                out.records.push(r);
            }
            Op::Remove { collection, key } => {
                check_name("collection", collection)?;
                check_name("key", key)?;
                diesel::delete(records::table.find((mission, collection, key))).execute(c)?;
                out.records.retain(|x| !(&x.collection == collection && &x.key == key));
                out.removed.push(RecordKey { collection: collection.clone(), key: key.clone() });
            }
            Op::Log { kind, text, data, at } => out.events.push(log_at(c, mission, kind, text, data.as_ref(), *at)?),
        }
    }
    Ok(out)
}

/// The collection whose records are questions for the user.
const NEEDS: &str = "needs";

/// The event a state change leaves, so the feed and a record's history show it without the agent logging it:
/// `state` when an existing record's state changes ("S1 Storage adapter → blocked"), `need` when a question
/// opens. New records don't log (a plan of twenty tasks isn't twenty events), nor do needs being answered
/// (the page's `answer` event says that).
fn state_change(c: &mut SqliteConnection, mission: &str, before: Option<&Record>, r: &Record) -> Result<Option<Event>> {
    let from = before.and_then(|b| b.state.clone());
    if from == r.state {
        return Ok(None);
    }
    let Some(to) = r.state.as_deref() else { return Ok(None) };
    let data = json!({"collection": r.collection, "key": r.key, "from": from, "to": to});
    if r.collection == NEEDS {
        if to != "open" {
            return Ok(None);
        }
        let q = r.body.get("question").and_then(Value::as_str).unwrap_or("");
        return log(c, mission, "need", &format!("{} opened: {q}", r.key), Some(&data)).map(Some);
    }
    if before.is_none() {
        return Ok(None);
    }
    let title = ["title", "name"].iter().find_map(|f| r.body.get(*f).and_then(Value::as_str)).unwrap_or("");
    let label = if title.is_empty() { r.key.clone() } else { format!("{} {title}", r.key) };
    log(c, mission, "state", &format!("{label} → {}", to.replace('_', " ")), Some(&data)).map(Some)
}

/// Merges `patch` into a record's body (creating it), setting `state` when given. Returns the record.
pub(super) fn upsert(
    c: &mut SqliteConnection,
    mission: &str,
    collection: &str,
    key: &str,
    state: Option<&str>,
    patch: &Value,
) -> Result<Record> {
    check_name("collection", collection)?;
    check_name("key", key)?;
    let found = records::table.find((mission, collection, key)).select(RecordRow::as_select()).first(c).optional()?;
    let (mut body, old_state) = match found {
        Some(r) => (serde_json::from_str(&r.body)?, r.state),
        None => (json!({}), None),
    };
    merge_patch(&mut body, patch);
    let row = RecordRow {
        mission: mission.into(),
        collection: collection.into(),
        key: key.into(),
        state: state.map(String::from).or(old_state),
        body: body.to_string(),
        updated: now_ms(),
    };
    diesel::replace_into(records::table).values(&row).execute(c)?;
    row.try_into()
}

/// The record `(collection, key)`, if it exists.
pub(super) fn record(c: &mut SqliteConnection, mission: &str, collection: &str, key: &str) -> Result<Option<Record>> {
    let row = records::table.find((mission, collection, key)).select(RecordRow::as_select()).first(c).optional()?;
    row.map(TryInto::try_into).transpose()
}

/// Appends an event and returns it.
pub(super) fn log(
    c: &mut SqliteConnection,
    mission: &str,
    kind: &str,
    text: &str,
    data: Option<&Value>,
) -> Result<Event> {
    log_at(c, mission, kind, text, data, None)
}

/// [`log`] at a given time (an import keeps the original's).
fn log_at(
    c: &mut SqliteConnection,
    mission: &str,
    kind: &str,
    text: &str,
    data: Option<&Value>,
    at: Option<i64>,
) -> Result<Event> {
    let row: EventRow = diesel::insert_into(events::table)
        .values((
            events::mission.eq(mission),
            events::kind.eq(kind),
            events::text.eq(text),
            events::data.eq(data.map(Value::to_string)),
            events::at.eq(at.unwrap_or_else(now_ms)),
        ))
        .returning(EventRow::as_returning())
        .get_result(c)?;
    row.try_into()
}

/// Collection names and keys: non-empty, at most [`MAX_NAME`] bytes.
fn check_name(what: &str, v: &str) -> Result<()> {
    if v.is_empty() || v.len() > MAX_NAME {
        bail!("{what} must be 1 to {MAX_NAME} bytes, got {}", v.len());
    }
    Ok(())
}

/// RFC 7386: objects merge, `null` deletes, anything else replaces.
fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(p) = patch else {
        if !patch.is_null() {
            *target = patch.clone();
        }
        return;
    };
    if !target.is_object() {
        *target = json!({});
    }
    let t = target.as_object_mut().expect("just made an object");
    for (k, v) in p {
        if v.is_null() {
            t.remove(k);
        } else {
            merge_patch(t.entry(k.clone()).or_insert(Value::Null), v);
        }
    }
}
