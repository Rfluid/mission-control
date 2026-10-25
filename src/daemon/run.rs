//! `run` buttons: an argv from the user's or the project's config, run on this machine after the user confirms.
//!
//! Three gates, each enough on its own: validation refuses `run` actions, their command keys and `permissions`
//! in the mission layer; `permissions.run_actions` must be true; and here, provenance must show that the
//! action's command and the permission came from user/project/builtin files, never a mission version.

use super::handlers::Res;
use super::page::{bad, fill};
use crate::config::Provenance;
use crate::model::{Mission, Op, Record};
use axum::http::StatusCode;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

const DEFAULT_TIMEOUT_S: u64 = 60;
const MAX_TIMEOUT_S: u64 = 600;
/// Output kept for the agent: the tail, where errors are.
const OUTPUT_TAIL: usize = 4000;

/// Runs action `id` and returns the log op reporting it, and the exit code (`None`: killed or timed out).
pub(super) async fn execute(
    mission: &Mission,
    id: &str,
    act: &Value,
    row: Option<&Record>,
    config: &Value,
    provenance: &Provenance,
) -> Res<(Vec<Op>, Option<i32>)> {
    if config.pointer("/permissions/run_actions").and_then(Value::as_bool) != Some(true) {
        return Err(bad("run actions are off: set `permissions.run_actions = true` in your user or project config"));
    }
    let from_mission = |prefix: &str| {
        provenance
            .iter()
            .any(|(k, v)| (k == prefix || k.starts_with(&format!("{prefix}."))) && v.starts_with("mission"))
    };
    let keys = ["kind", "command", "cwd", "env", "timeout_s"].map(|k| format!("actions.{id}.{k}"));
    if from_mission("permissions.run_actions") || keys.iter().any(|k| from_mission(k)) {
        return Err((StatusCode::FORBIDDEN, format!("action `{id}` was changed by the mission layer; not running it")));
    }
    let argv: Vec<String> = act
        .get("command")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|v| v.as_str().map(|s| fill(s, row, config))).collect::<Option<Vec<_>>>())
        .unwrap_or_default()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| bad(format!("action `{id}`: `command` must be a non-empty array of strings")))?;
    let secs = act.get("timeout_s").and_then(Value::as_u64).unwrap_or(DEFAULT_TIMEOUT_S).min(MAX_TIMEOUT_S);
    let mut cmd = tokio::process::Command::new(&argv[0]);
    cmd.args(&argv[1..]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    let cwd = act.get("cwd").and_then(Value::as_str).map(String::from).or_else(|| mission.project_dir.clone());
    if let Some(d) = cwd.filter(|d| Path::new(d).is_dir()) {
        cmd.current_dir(d);
    }
    if let Some(env) = act.get("env").and_then(Value::as_object) {
        for (k, v) in env {
            cmd.env(k, v.as_str().map_or_else(|| v.to_string(), |s| fill(s, row, config)));
        }
    }
    cmd.env("MISSION_CONTROL_ID", &mission.id);
    tracing::info!(mission = %mission.id, action = id, program = %argv[0], "run action");
    let (exit, output) = match cmd.spawn() {
        Err(e) => (None, format!("could not start `{}`: {e}", argv[0])),
        Ok(child) => match tokio::time::timeout(Duration::from_secs(secs), child.wait_with_output()).await {
            Err(_) => (None, format!("timed out after {secs}s (killed)")),
            Ok(Err(e)) => (None, e.to_string()),
            Ok(Ok(o)) => {
                let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
                text.push_str(&String::from_utf8_lossy(&o.stderr));
                (o.status.code(), tail(&text))
            }
        },
    };
    let label = act.get("label").and_then(Value::as_str).unwrap_or(id);
    let status = exit.map_or_else(|| "failed".to_string(), |c| format!("exit {c}"));
    let row_json = row.map(|r| json!({"collection": r.collection, "key": r.key, "state": r.state, "body": r.body}));
    let op = Op::Log {
        at: None,
        kind: "action".into(),
        text: format!("{label}: {status}"),
        data: Some(json!({"action": id, "kind": "run", "label": label, "row": row_json, "command": argv,
                          "exit": exit, "output": output})),
    };
    Ok((vec![op], exit))
}

/// The last [`OUTPUT_TAIL`] bytes, cut at a char boundary.
fn tail(s: &str) -> String {
    let mut start = s.len().saturating_sub(OUTPUT_TAIL);
    while !s.is_char_boundary(start) {
        start += 1;
    }
    s[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn an_action_the_mission_layer_touched_never_runs() {
        let m = Mission {
            id: "msn_1".into(),
            title: "t".into(),
            session: None,
            project_dir: None,
            created: 0,
            archived: false,
        };
        let act = json!({"kind": "run", "command": ["true"]});
        let config = json!({"permissions": {"run_actions": true}, "actions": {"x": act}});
        let mut prov = Provenance::new();
        prov.insert("permissions.run_actions".into(), "user (/c)".into());
        prov.insert("actions.x.command".into(), "mission v2".into());
        let err = execute(&m, "x", &act, None, &config, &prov).await.unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        prov.insert("actions.x.command".into(), "project (/p)".into());
        assert_eq!(execute(&m, "x", &act, None, &config, &prov).await.unwrap().1, Some(0));
    }

    #[test]
    fn tail_keeps_the_end_on_a_char_boundary() {
        let s = format!("{}é{}", "a".repeat(10), "b".repeat(OUTPUT_TAIL - 1));
        let t = tail(&s);
        assert!(t.len() <= OUTPUT_TAIL && t.ends_with('b'));
        assert_eq!(tail("short"), "short");
    }
}
