//! `mission-control import`: an old hand-built mission-control `state.json` (one JSON document of arrays, an `activity`
//! log and a `run` summary, rewritten by an agent through `exec`) → typed ops for a mission, plus a suggested
//! project config for the states it uses. Pure: the CLI applies the ops through the daemon.
//!
//! Rules, so any such file maps without per-domain code:
//! - each top-level array is a collection (`needs_you` → `needs`); a row's key is its `id`, `number`, `key` or
//!   `name`, else its position (zero-padded, so rows keep their order); its state is `state`, else `result`;
//!   everything else is the body.
//! - `activity` (`[{at, kind, text}]`) becomes events with their original times.
//! - `run` becomes the record `run/run`: state = `health`, `reason` = `health_reason`.
//! - other objects (`spend`) become a record of their own name (`spend/total`), and their arrays of rows
//!   (`by_package`) rows of that collection.

use crate::model::Op;
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

/// Collections renamed on the way in.
const RENAMES: [(&str, &str); 1] = [("needs_you", "needs")];
/// Fields that can name a row, in order of preference.
const KEY_FIELDS: [&str; 4] = ["id", "number", "key", "name"];
/// Ops per batch the CLI sends (one transaction each).
pub const BATCH: usize = 200;

pub struct Imported {
    pub ops: Vec<Op>,
    /// Every state value seen, for the suggested `[states]`.
    pub states: BTreeSet<String>,
    pub title: Option<String>,
}

fn collection(name: &str) -> String {
    RENAMES.iter().find(|(from, _)| *from == name).map_or(name, |(_, to)| *to).to_string()
}

fn key_of(row: &Map<String, Value>, index: usize, width: usize) -> String {
    for f in KEY_FIELDS {
        match row.get(f) {
            Some(Value::String(s)) if !s.is_empty() => return s.clone(),
            Some(Value::Number(n)) => return n.to_string(),
            _ => {}
        }
    }
    format!("{index:0width$}")
}

fn rows(name: &str, items: &[Value], out: &mut Imported) {
    let coll = collection(name);
    let width = items.len().to_string().len();
    for (i, item) in items.iter().enumerate() {
        let Value::Object(row) = item else { continue };
        let key = key_of(row, i + 1, width);
        let state = row.get("state").or_else(|| row.get("result")).and_then(Value::as_str).map(String::from);
        if let Some(s) = &state {
            out.states.insert(s.clone());
        }
        let mut body = row.clone();
        body.remove("state");
        body.remove("result");
        out.ops.push(Op::Upsert { collection: coll.clone(), key, state, patch: Value::Object(body) });
    }
}

/// Converts a `state.json` document.
pub fn convert(doc: &Value) -> anyhow::Result<Imported> {
    let Value::Object(top) = doc else { anyhow::bail!("state.json must be a JSON object") };
    let mut out = Imported { ops: Vec::new(), states: BTreeSet::new(), title: None };
    for (name, v) in top {
        match (name.as_str(), v) {
            ("activity", Value::Array(evs)) => {
                for e in evs {
                    let text = e.get("text").and_then(Value::as_str).unwrap_or_default();
                    let kind = e.get("kind").and_then(Value::as_str).unwrap_or("log");
                    let at = e.get("at").and_then(Value::as_str).and_then(parse_time);
                    out.ops.push(Op::Log { kind: kind.into(), text: text.into(), data: None, at });
                }
            }
            ("run", Value::Object(run)) => {
                out.title = run.get("title").and_then(Value::as_str).map(String::from);
                let state = run.get("health").and_then(Value::as_str).map(String::from);
                if let Some(s) = &state {
                    out.states.insert(s.clone());
                }
                let mut body = run.clone();
                body.remove("health");
                if let Some(r) = body.remove("health_reason") {
                    body.insert("reason".into(), r);
                }
                out.ops.push(Op::Upsert {
                    collection: "run".into(),
                    key: "run".into(),
                    state,
                    patch: Value::Object(body),
                });
            }
            (_, Value::Array(items)) => rows(name, items, &mut out),
            (_, Value::Object(obj)) => {
                let mut total = Map::new();
                for (k, x) in obj {
                    match x {
                        Value::Array(items) if items.iter().all(Value::is_object) && !items.is_empty() => {
                            rows(name, items, &mut out);
                        }
                        other => {
                            total.insert(k.clone(), other.clone());
                        }
                    }
                }
                out.ops.push(Op::Upsert {
                    collection: collection(name),
                    key: "total".into(),
                    state: None,
                    patch: Value::Object(total),
                });
            }
            _ => {} // scalars (`updated`): nothing to keep
        }
    }
    Ok(out)
}

/// A tone for a state name nobody configured, guessed from common words; `None` if it's a built-in state.
pub fn guess_tone(state: &str) -> &'static str {
    let s = state.to_ascii_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| s.contains(x));
    // Order matters: `fixed_pending_integration` is waiting, `fixing` is moving, `fixed` is done.
    if has(&["fail", "block", "red", "error", "reject", "stop", "abort"]) {
        "block"
    } else if has(&["review"]) {
        "rev"
    } else if has(&["open", "pend", "wait", "risk", "warn", "partial", "queue"]) {
        "warn"
    } else if has(&["progress", "running", "active", "working", "doing"]) || s.ends_with("ing") {
        "prog"
    } else if has(&[
        "pass", "merge", "done", "green", "answer", "approv", "complete", "ok", "track", "fix", "resolv", "mitigat",
        "accept", "closed",
    ]) {
        "done"
    } else {
        "todo"
    }
}

/// `[states]` for the states seen that `known` (the resolved config's states) doesn't cover.
pub fn suggested_states(seen: &BTreeSet<String>, known: &Value) -> Value {
    let mut m = Map::new();
    for s in seen.iter().filter(|s| known.get(s.as_str()).is_none()) {
        m.insert(s.clone(), json!(guess_tone(s)));
    }
    Value::Object(m)
}

/// RFC 3339 (`2026-10-21T15:05:00Z`, fractions and offsets allowed) → Unix ms.
pub fn parse_time(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    let num = |r: std::ops::Range<usize>| text.get(r)?.parse::<i64>().ok();
    if bytes.len() < 19 || bytes[4] != b'-' || bytes[7] != b'-' || !matches!(bytes[10], b'T' | b't' | b' ') {
        return None;
    }
    let (year, month, day) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let secs_of_day = num(11..13)? * 3600 + num(14..16)? * 60 + num(17..19)?;
    let mut pos = 19;
    let mut millis = 0;
    if bytes.get(pos) == Some(&b'.') {
        let start = pos + 1;
        pos = start;
        while bytes.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
        }
        millis = format!("{:0<3}", &text[start..pos])[..3].parse::<i64>().ok()?;
    }
    let offset_min = match bytes.get(pos) {
        Some(b'Z' | b'z') | None => 0,
        Some(sign @ (b'+' | b'-')) => {
            let off = num(pos + 1..pos + 3)? * 60 + num(pos + 4..pos + 6)?;
            if *sign == b'+' { off } else { -off }
        }
        _ => return None,
    };
    // Days from civil (Howard Hinnant).
    let yr = if month <= 2 { year - 1 } else { year };
    let era = yr.div_euclid(400);
    let yoe = yr - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days * 86_400 + secs_of_day - offset_min * 60) * 1000 + millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_parse() {
        assert_eq!(parse_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_time("2026-10-21T15:05:00Z"), Some(1_792_595_100_000));
        assert_eq!(parse_time("2026-10-21T17:05:00.5+02:00"), Some(1_792_595_100_500));
        assert_eq!(parse_time("yesterday"), None);
    }

    #[test]
    fn a_state_file_becomes_records_and_events() {
        let doc = json!({
            "updated": "x",
            "run": {"title": "Build", "health": "at_risk", "health_reason": "drift"},
            "packages": [{"id": "R0", "state": "merged", "wave": "W0"}],
            "prs": [{"number": 17, "state": "open"}],
            "tests": [{"pkg": "R0", "result": "pass"}, {"pkg": "R0", "result": "fail"}],
            "needs_you": [{"id": "N-1", "state": "open", "question": "?"}],
            "spend": {"total_usd": 1.5, "by_package": [{"pkg": "R4", "usd": 1.5}]},
            "activity": [{"at": "1970-01-01T00:00:01Z", "kind": "start", "text": "go"}],
        });
        let i = convert(&doc).unwrap();
        let keys: Vec<String> = i
            .ops
            .iter()
            .filter_map(|o| match o {
                Op::Upsert { collection, key, .. } => Some(format!("{collection}/{key}")),
                _ => None,
            })
            .collect();
        assert_eq!(
            keys,
            ["run/run", "packages/R0", "prs/17", "tests/1", "tests/2", "needs/N-1", "spend/1", "spend/total"]
        );
        assert!(
            matches!(&i.ops[0], Op::Upsert { state: Some(s), patch, .. } if s == "at_risk" && patch["reason"] == "drift")
        );
        assert!(i.ops.iter().any(|o| matches!(o, Op::Log { at: Some(1000), kind, .. } if kind == "start")));
        assert_eq!(i.title.as_deref(), Some("Build"));
        let st = suggested_states(&i.states, &json!({"merged": "done", "open": "warn"}));
        assert_eq!(st, json!({"at_risk": "warn", "fail": "block", "pass": "done"}));
        let tones: Vec<_> =
            ["fixed_pending_integration", "fixing", "fixed", "validating", "queued", "stopped", "observed"]
                .iter()
                .map(|s| guess_tone(s))
                .collect();
        assert_eq!(tones, ["warn", "prog", "done", "prog", "warn", "block", "todo"]);
    }
}
