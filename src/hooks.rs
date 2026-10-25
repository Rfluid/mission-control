//! `mission-control hook`: Claude Code hooks fill a mission with no agent calls. The hook payload (JSON on stdin) becomes
//! ops, by `[ingest]`: subagent launches (`PreToolUse`/`PostToolUse` of the `Task`/`Agent` tool) → `agents`;
//! `gh pr create` / `gh pr merge` in `Bash` → `prs`; `Stop` → `spend`, totals of the session transcript's token
//! usage per model, priced from `[ingest.prices]` (estimates). Pure: the CLI finds the mission and applies them.

use crate::model::Op;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// Tools that launch subagents (Claude Code has used both names).
const SUBAGENT_TOOLS: [&str; 2] = ["Task", "Agent"];
/// Longest subagent report kept on the `agents` row.
const REPORT_CHARS: usize = 300;
const PER_MILLION: f64 = 1_000_000.0;

fn s<'a>(v: &'a Value, path: &str) -> &'a str {
    v.pointer(path).and_then(Value::as_str).unwrap_or_default()
}

fn on(ingest: &Value, key: &str) -> bool {
    ingest.get(key).and_then(Value::as_bool).unwrap_or(true)
}

/// The tool's output as text, whatever shape the payload gives it (a string, `{stdout}`, `{content: [{text}]}`).
fn output(v: &Value) -> String {
    fn text(x: &Value) -> String {
        match x {
            Value::String(t) => t.clone(),
            Value::Array(parts) => parts.iter().map(text).collect::<Vec<_>>().join("\n"),
            Value::Object(o) => ["stdout", "output", "result", "content", "text"]
                .iter()
                .find_map(|k| o.get(*k))
                .map(text)
                .unwrap_or_default(),
            _ => String::new(),
        }
    }
    v.get("tool_response").map(text).unwrap_or_default()
}

/// Ops for one hook payload. `ingest` is the mission's resolved `[ingest]`; `transcript` the session transcript's
/// text, read by the caller for `Stop`.
pub fn ops(payload: &Value, ingest: &Value, transcript: Option<&str>) -> Vec<Op> {
    let event = s(payload, "/hook_event_name");
    let tool = s(payload, "/tool_name");
    let session = s(payload, "/session_id");
    let short: String = session.chars().take(8).collect();
    match event {
        "PreToolUse" | "PostToolUse" if SUBAGENT_TOOLS.contains(&tool) && on(ingest, "subagents") => {
            let id = payload.get("tool_use_id").and_then(Value::as_str).map(String::from);
            let desc = s(payload, "/tool_input/description");
            let key = id.unwrap_or_else(|| format!("{short}-{}", desc.chars().take(40).collect::<String>()));
            let mut body =
                json!({"description": desc, "type": s(payload, "/tool_input/subagent_type"), "session": session});
            let state = if event == "PreToolUse" {
                "running"
            } else {
                body["last_report"] = json!(output(payload).chars().take(REPORT_CHARS).collect::<String>());
                "finished"
            };
            vec![Op::Upsert { collection: "agents".into(), key, state: Some(state.into()), patch: body }]
        }
        "PostToolUse" if tool == "Bash" && on(ingest, "prs") => {
            pr_ops(s(payload, "/tool_input/command"), &output(payload))
        }
        "Stop" | "SubagentStop" if on(ingest, "spend") => {
            transcript.map_or_else(Vec::new, |t| spend_ops(t, &short, ingest))
        }
        _ => Vec::new(),
    }
}

/// `gh pr create` (number from the URL it prints) and `gh pr merge <n>`.
fn pr_ops(command: &str, out: &str) -> Vec<Op> {
    let url = out.split_whitespace().find(|w| w.starts_with("https://") && w.contains("/pull/"));
    let number = |text: &str| {
        text.rsplit("/pull/").next().map(|n| n.chars().take_while(char::is_ascii_digit).collect::<String>())
    };
    if command.contains("gh pr create") {
        let Some(url) = url else { return Vec::new() };
        let Some(n) = number(url).filter(|n| !n.is_empty()) else { return Vec::new() };
        let mut body = json!({"url": url});
        if let Some(t) = flag(command, "--title").or_else(|| flag(command, "-t")) {
            body["title"] = json!(t);
        }
        return vec![Op::Upsert { collection: "prs".into(), key: n, state: Some("open".into()), patch: body }];
    }
    if command.contains("gh pr merge") {
        let arg =
            command.split("gh pr merge").nth(1).unwrap_or_default().split_whitespace().find(|w| !w.starts_with('-'));
        let n = arg
            .and_then(|a| if a.contains("/pull/") { number(a) } else { Some(a.trim_start_matches('#').to_string()) });
        if let Some(n) = n.filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())) {
            return vec![Op::Upsert {
                collection: "prs".into(),
                key: n,
                state: Some("merged".into()),
                patch: json!({}),
            }];
        }
    }
    Vec::new()
}

/// The value after `name` in a shell command: `--title "feat: x"`, `--title=x`.
fn flag(command: &str, name: &str) -> Option<String> {
    let rest = command.split(&format!("{name} ")).nth(1).or_else(|| command.split(&format!("{name}=")).nth(1))?;
    let rest = rest.trim_start();
    let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'');
    Some(match quote {
        Some(q) => rest[1..].split(q).next().unwrap_or_default().to_string(),
        None => rest.split_whitespace().next().unwrap_or_default().to_string(),
    })
}

#[derive(Default)]
struct Usage {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
}

/// Totals per model from a transcript (JSONL). Streamed messages repeat their id; the last copy wins.
fn usage(transcript: &str) -> BTreeMap<String, Usage> {
    let mut by_id: BTreeMap<String, (String, Value)> = BTreeMap::new();
    for (i, line) in transcript.lines().enumerate() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let Some(msg) = v.get("message") else { continue };
        let (Some(model), Some(u)) = (msg.get("model").and_then(Value::as_str), msg.get("usage")) else { continue };
        let id = msg.get("id").and_then(Value::as_str).map_or_else(|| format!("line{i}"), String::from);
        by_id.insert(id, (model.to_string(), u.clone()));
    }
    let mut out: BTreeMap<String, Usage> = BTreeMap::new();
    for (model, u) in by_id.into_values() {
        let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
        let e = out.entry(model).or_default();
        e.input += n("input_tokens");
        e.output += n("output_tokens");
        e.cache_read += n("cache_read_input_tokens");
        e.cache_write += n("cache_creation_input_tokens");
    }
    out
}

/// `[ingest.prices]` entry for `model`: the longest key that prefixes it. USD per million tokens:
/// `input`, `output`, and optionally `cache_read` (default 0.1 × input) and `cache_write` (1.25 × input).
fn price<'a>(prices: Option<&'a Map<String, Value>>, model: &str) -> Option<&'a Value> {
    prices?.iter().filter(|(k, _)| model.starts_with(k.as_str())).max_by_key(|(k, _)| k.len()).map(|(_, v)| v)
}

#[allow(clippy::cast_precision_loss)] // token counts stay far below 2^52
fn spend_ops(transcript: &str, session: &str, ingest: &Value) -> Vec<Op> {
    let prices = ingest.get("prices").and_then(Value::as_object);
    usage(transcript)
        .into_iter()
        .map(|(model, u)| {
            let usd = price(prices, &model).map(|p| {
                let f = |k: &str| p.get(k).and_then(Value::as_f64);
                let input = f("input").unwrap_or(0.0);
                let (read, write) = (f("cache_read").unwrap_or(input * 0.1), f("cache_write").unwrap_or(input * 1.25));
                (u.input as f64 * input + u.output as f64 * f("output").unwrap_or(0.0) + u.cache_read as f64 * read
                    + u.cache_write as f64 * write)
                    / PER_MILLION
            });
            Op::Upsert {
                collection: "spend".into(),
                key: format!("{session}-{model}"),
                state: None,
                patch: json!({"model": model, "session": session, "input_tokens": u.input, "output_tokens": u.output,
                    "cache_read_tokens": u.cache_read, "cache_write_tokens": u.cache_write, "usd": usd, "estimated": true}),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upsert(op: &Op) -> (&str, &str, Option<&str>, &Value) {
        match op {
            Op::Upsert { collection, key, state, patch } => (collection, key, state.as_deref(), patch),
            _ => panic!("not an upsert"),
        }
    }

    #[test]
    fn subagents_become_agents_rows() {
        let ing = json!({});
        let pre = json!({"hook_event_name": "PreToolUse", "tool_name": "Task", "tool_use_id": "tu_1", "session_id": "abc",
            "tool_input": {"description": "Write tests", "subagent_type": "general"}});
        let got = ops(&pre, &ing, None);
        let (c, k, st, body) = upsert(&got[0]);
        assert_eq!((c, k, st, body["description"].as_str()), ("agents", "tu_1", Some("running"), Some("Write tests")));
        let post = json!({"hook_event_name": "PostToolUse", "tool_name": "Agent", "tool_use_id": "tu_1",
            "tool_response": {"content": [{"type": "text", "text": "Done: 12 tests"}]}});
        let got = ops(&post, &ing, None);
        let (_, _, st, body) = upsert(&got[0]);
        assert_eq!((st, body["last_report"].as_str()), (Some("finished"), Some("Done: 12 tests")));
        assert!(ops(&pre, &json!({"subagents": false}), None).is_empty(), "[ingest] subagents = false");
    }

    #[test]
    fn gh_pr_commands_become_prs_rows() {
        let create = json!({"hook_event_name": "PostToolUse", "tool_name": "Bash",
            "tool_input": {"command": "gh pr create --title \"feat: x\" --body y"},
            "tool_response": {"stdout": "https://github.com/a/b/pull/42\n"}});
        let got = ops(&create, &json!({}), None);
        let (c, k, st, body) = upsert(&got[0]);
        assert_eq!((c, k, st, body["title"].as_str()), ("prs", "42", Some("open"), Some("feat: x")));
        let merge = json!({"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {"command": "gh pr merge 42 --squash"}});
        assert_eq!(upsert(&ops(&merge, &json!({}), None)[0]).2, Some("merged"));
        let other = json!({"hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_input": {"command": "ls"}});
        assert!(ops(&other, &json!({}), None).is_empty());
    }

    #[test]
    fn stop_totals_transcript_usage_per_model() {
        let t = [
            r#"{"type":"assistant","message":{"id":"m1","model":"claude-opus-5-5","usage":{"input_tokens":10,"output_tokens":1}}}"#,
            r#"{"type":"assistant","message":{"id":"m1","model":"claude-opus-5-5","usage":{"input_tokens":1000000,"output_tokens":100000,"cache_read_input_tokens":1000000}}}"#,
            r#"{"type":"assistant","message":{"id":"m2","model":"claude-haiku-5-5","usage":{"input_tokens":5,"output_tokens":5}}}"#,
            "not json",
        ]
        .join("\n");
        let ing = json!({"prices": {"claude-opus-5": {"input": 4.0, "output": 20.0, "cache_read": 0.2}, "claude-opus": {"input": 99.0}}});
        let out = ops(&json!({"hook_event_name": "Stop", "session_id": "sess1234567"}), &ing, Some(&t));
        assert_eq!(out.len(), 2);
        let (_, k, _, body) = upsert(&out[1]);
        assert_eq!(k, "sess1234-claude-opus-5-5");
        assert_eq!(body["input_tokens"], 1_000_000, "the last copy of a streamed message wins");
        assert!((body["usd"].as_f64().unwrap() - 6.2).abs() < 1e-9, "4 + 2 + 0.2 with the longest prefix: {body}");
        assert_eq!(upsert(&out[0]).3["usd"], Value::Null, "no price, no estimate");
    }
}
