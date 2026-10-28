//! What the missions home shows for each mission (`GET /api/missions?summary=true`), computed from the
//! mission's resolved config so the cards follow each template: health (state, tone, reason), open needs (what
//! each blocks), progress (the phases of its `plan` section, else the states of its main collection) and stats
//! (its status strip's cells). One request renders the home page.

use super::App;
use super::config::{Overlay, resolve};
use super::handlers::{Res, internal};
use crate::model::{Mission, Record};
use serde_json::{Value, json};

/// Default collections of the strip's health record and of the questions for the user.
const RUN: (&str, &str) = ("run", "run");
const NEEDS: &str = "needs";
/// Tone order of a progress bar: settled first.
const ORDER: [&str; 6] = ["done", "rev", "prog", "warn", "block", "todo"];
/// Strip cells shown as stats on a card.
const STATS: usize = 3;

/// The `summary` of one mission: the old fields (`open_needs`, `records`, `last_activity`, `health`) plus
/// `template`, `health_tone`, `reason`, `needs`, `progress` and `stats`.
pub(super) fn summary(app: &App, m: &Mission) -> Res<Value> {
    // The store guard ends with this block: resolving reads the store too.
    let (mut out, records) = {
        let mut s = app.store();
        (s.summary(&m.id).map_err(internal)?, s.records(&m.id).map_err(internal)?)
    };
    let Ok(r) = resolve(app, m, &Overlay::None) else { return Ok(out) };
    let c = &r.config;
    let tone = |state: Option<&str>| -> String {
        state
            .and_then(|s| c.pointer(&format!("/states/{}", esc_ptr(s))))
            .and_then(Value::as_str)
            .unwrap_or("todo")
            .to_string()
    };
    let strip = c.get("strip").cloned().unwrap_or(Value::Null);
    let (hc, hk) = (
        strip.pointer("/health/collection").and_then(Value::as_str).unwrap_or(RUN.0),
        strip.pointer("/health/key").and_then(Value::as_str).unwrap_or(RUN.1),
    );
    let health = records.iter().find(|x| x.collection == hc && x.key == hk);
    out["template"] = json!(r.template);
    out["health"] = json!(health.and_then(|h| h.state.clone()));
    out["health_tone"] = json!(tone(health.and_then(|h| h.state.as_deref())));
    out["reason"] = json!(health.and_then(|h| h.body.get("reason")).and_then(Value::as_str));
    let needs_coll = strip.get("needs").and_then(Value::as_str).unwrap_or(NEEDS);
    out["needs"] = Value::Array(
        records
            .iter()
            .filter(|n| n.collection == needs_coll && n.state.as_deref() != Some("answered"))
            .map(|n| {
                json!({"key": n.key, "question": n.body.get("question").cloned().unwrap_or(Value::Null),
                       "blocks": n.body.get("blocks").cloned().unwrap_or(Value::Null), "since": n.updated})
            })
            .collect(),
    );
    out["progress"] = progress(c, &records, &tone);
    out["stats"] = Value::Array(
        strip
            .get("cells")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|cell| cell.get("plan").is_none())
            .filter_map(|cell| stat(cell, &records))
            .take(STATS)
            .collect(),
    );
    Ok(out)
}

/// `~1` and `~0` escaping for a JSON pointer segment.
fn esc_ptr(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

/// A string or a list of strings, as a list.
fn many(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(String::from).collect(),
        _ => Vec::new(),
    }
}

fn field<'a>(r: &'a Record, f: &str) -> Option<&'a Value> {
    r.body.get(f)
}

/// The phases of the first `plan` section (each `done`, `now` or ahead), else the tones of the main
/// collection's records (the first `[brief]` collection that isn't needs), with a one-line caption.
fn progress(c: &Value, records: &[Record], tone: &dyn Fn(Option<&str>) -> String) -> Value {
    let sections = c.get("sections").and_then(Value::as_array).cloned().unwrap_or_default();
    if let Some(plan) = sections.iter().find(|s| s.get("widget").and_then(Value::as_str) == Some("plan")) {
        let nodes_coll = plan.pointer("/nodes/collection").and_then(Value::as_str).unwrap_or("");
        let mut phase_fields = many(plan.pointer("/nodes/phase"));
        if phase_fields.is_empty() {
            phase_fields = vec!["phase".into(), "wave".into()];
        }
        let nodes: Vec<&Record> = records.iter().filter(|r| r.collection == nodes_coll).collect();
        let phase_of = |r: &Record| {
            phase_fields.iter().find_map(|f| field(r, f).and_then(Value::as_str)).unwrap_or("").to_string()
        };
        let pcolls = many(plan.pointer("/phases/collection"));
        let pcoll = pcolls.iter().find(|pc| records.iter().any(|r| &r.collection == *pc));
        let mut phases: Vec<(String, String)> = if let Some(pc) = pcoll {
            records
                .iter()
                .filter(|r| &r.collection == pc)
                .map(|r| (r.key.clone(), field(r, "title").and_then(Value::as_str).unwrap_or(&r.key).to_string()))
                .collect()
        } else {
            let mut ks: Vec<String> = nodes.iter().map(|n| phase_of(n)).filter(|k| !k.is_empty()).collect();
            ks.sort();
            ks.dedup();
            ks.into_iter().map(|k| (k.clone(), k)).collect()
        };
        phases.sort_by(|a, b| natural(&a.0, &b.0));
        if !phases.is_empty() {
            // "Now" is the phase being built (the page's track does the same): the first with a record in progress,
            // else the first with work not started (review and waiting run async), else the first not done.
            let tone_of = |r: &&Record| tone(r.state.as_deref());
            let tests: [&dyn Fn(&&Record) -> bool; 3] =
                [&|r| tone_of(r) == "prog", &|r| !matches!(tone_of(r).as_str(), "done" | "rev" | "warn"), &|r| {
                    tone_of(r) != "done"
                }];
            let current = tests
                .iter()
                .find_map(|test| phases.iter().position(|(k, _)| nodes.iter().any(|n| phase_of(n) == *k && test(n))));
            let at = current.unwrap_or(phases.len() - 1);
            let segments: Vec<&str> = (0..phases.len())
                .map(|i| {
                    // A phase behind "now" can still hold review or blocked work; only a finished one fills.
                    let finished = || nodes.iter().all(|n| phase_of(n) != phases[i].0 || tone_of(n) == "done");
                    if current.is_none() || (i < at && finished()) {
                        "done"
                    } else if i == at {
                        "now"
                    } else {
                        ""
                    }
                })
                .collect();
            let caption = format!("Phase {} of {} · {}", at + 1, phases.len(), phases[at].1);
            return json!({"kind": "phases", "segments": segments, "caption": caption});
        }
    }
    let main = many(c.pointer("/brief/collections")).into_iter().find(|x| x != NEEDS);
    let Some(main) = main else { return Value::Null };
    let recs: Vec<&Record> = records.iter().filter(|r| r.collection == main).collect();
    if recs.is_empty() {
        return Value::Null;
    }
    let mut tones: Vec<String> = recs.iter().map(|r| tone(r.state.as_deref())).collect();
    tones.sort_by_key(|t| ORDER.iter().position(|o| o == t).unwrap_or(ORDER.len()));
    let done = tones.iter().filter(|t| *t == "done").count();
    json!({"kind": "share", "segments": tones, "caption": format!("{done} of {} {main} done", recs.len())})
}

/// Natural order (`P2` < `P10`), like the page's `h.compare`.
fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |s: &str| {
        let i = s.find(|c: char| c.is_ascii_digit()).unwrap_or(s.len());
        (s[..i].to_string(), s[i..].parse::<u64>().unwrap_or(0), s.to_string())
    };
    split(a).cmp(&split(b))
}

/// One strip cell as `{label, value}` text ("3/8 done", "$14.20"), like the page's strip.
fn stat(cell: &Value, records: &[Record]) -> Option<Value> {
    let label = cell.get("label").and_then(Value::as_str)?;
    let n = if let Some(spec) = cell.get("count") {
        count(spec, records)
    } else if let Some(spec) = cell.get("sum") {
        sum(spec, records)
    } else {
        return cell.get("value").map(|v| json!({"label": label, "value": v}));
    };
    if cell.get("hide_zero").and_then(Value::as_bool) == Some(true) && n == 0.0 {
        return None;
    }
    let mut text = fmt(n, cell.get("format").and_then(Value::as_str));
    if let Some(of) = cell.get("of") {
        text = format!("{text}/{}", fmt(count(of, records), None));
    }
    if let Some(s) = cell.get("suffix").and_then(Value::as_str) {
        text = format!("{text} {s}");
    }
    Some(json!({"label": label, "value": text, "tone": cell.get("tone")}))
}

fn matches(spec: &Value, r: &Record) -> bool {
    let coll = spec.get("collection").and_then(Value::as_str).unwrap_or("");
    let states = many(spec.get("state"));
    let state_ok = states.is_empty() || r.state.as_ref().is_some_and(|s| states.contains(s));
    let field_ok = spec.get("field").and_then(Value::as_str).is_none_or(|f| {
        let v = match f {
            "key" => Some(Value::String(r.key.clone())),
            "state" => r.state.clone().map(Value::String),
            _ => r.body.get(f).cloned(),
        };
        v.as_ref() == spec.get("value")
    });
    r.collection == coll && state_ok && field_ok
}

#[allow(clippy::cast_precision_loss)] // record counts are far below 2^52
fn count(spec: &Value, records: &[Record]) -> f64 {
    records.iter().filter(|r| matches(spec, r)).count() as f64
}

fn sum(spec: &Value, records: &[Record]) -> f64 {
    let fields = many(spec.get("field"));
    let pick = json!({"collection": spec.get("collection"), "state": spec.get("state")});
    records
        .iter()
        .filter(|r| matches(&pick, r))
        .flat_map(|r| fields.iter().filter_map(|f| r.body.get(f).and_then(Value::as_f64)))
        .sum()
}

/// `usd` → "$14.20", `tokens` → "3.1M", else a plain number (no decimals for whole numbers).
fn fmt(n: f64, format: Option<&str>) -> String {
    match format {
        Some("usd") => format!("${n:.2}"),
        Some("tokens") if n >= 1e6 => format!("{:.1}M", n / 1e6),
        Some("tokens") if n >= 1e3 => format!("{:.1}K", n / 1e3),
        _ if n.fract() == 0.0 => format!("{n:.0}"),
        _ => format!("{n:.2}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_format_like_the_page() {
        assert_eq!(fmt(14.2, Some("usd")), "$14.20");
        assert_eq!(fmt(3_100_000.0, Some("tokens")), "3.1M");
        assert_eq!(fmt(3.0, None), "3");
        assert_eq!(natural("P2", "P10"), std::cmp::Ordering::Less);
    }

    #[test]
    fn the_current_phase_is_where_work_is_in_progress_not_where_review_waits() {
        let rec = |collection: &str, key: &str, state: &str, phase: &str| Record {
            collection: collection.into(),
            key: key.into(),
            state: Some(state.into()),
            body: json!({"phase": phase, "title": key}),
            updated: 0,
        };
        let config = json!({"sections": [{"widget": "plan", "phases": {"collection": "phases"}, "nodes": {"collection": "tasks"}}]});
        let tone = |state: Option<&str>| -> String {
            match state {
                Some("done") => "done",
                Some("in_review") => "rev",
                Some("in_progress") => "prog",
                _ => "todo",
            }
            .into()
        };
        let mut records = vec![
            rec("phases", "P1", "", ""),
            rec("phases", "P2", "", ""),
            rec("phases", "P3", "", ""),
            rec("tasks", "a", "in_review", "P1"),
            rec("tasks", "b", "in_progress", "P2"),
            rec("tasks", "c", "todo", "P3"),
        ];
        let now = |records: &[Record]| progress(&config, records, &tone)["caption"].as_str().unwrap_or("").to_string();
        assert_eq!(now(&records), "Phase 2 of 3 · P2", "a PR in review does not hold the phase");
        let segments = |records: &[Record]| progress(&config, records, &tone)["segments"].clone();
        assert_eq!(segments(&records), json!(["", "now", ""]), "a phase behind now with work in review is not done");
        records[4].state = Some("done".into());
        assert_eq!(now(&records), "Phase 3 of 3 · P3", "then the first with work not started");
        records[5].state = Some("done".into());
        assert_eq!(now(&records), "Phase 1 of 3 · P1", "then the first not done");
    }
}
