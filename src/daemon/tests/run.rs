//! `run` buttons: off by default, run only what user config defines, never what the mission layer changed.

use super::*;

fn user_config(t: &T, run_actions: bool) {
    let text = format!(
        "[permissions]\nrun_actions = {run_actions}\n\n[actions.hello]\nlabel = \"Hello\"\nkind = \"run\"\ncommand = [\"sh\", \"-c\", \"echo hi $0\", \"{{row.key}}\"]\n\n[actions.slow]\nkind = \"run\"\ncommand = [\"sleep\", \"5\"]\ntimeout_s = 1\n"
    );
    std::fs::write(t.user.path().join("config.toml"), text).unwrap();
}

async fn click(t: &T, id: &str, action: &str) -> (StatusCode, Value) {
    let tok = t.token(id).await;
    t.page(id, Some(&tok), json!({"kind": "action", "action": action, "collection": "prs", "key": "17"})).await
}

#[tokio::test]
async fn run_is_off_until_the_user_turns_it_on() {
    let t = T::new();
    user_config(&t, false);
    let id = t.mission("m").await;
    t.ops(&id, json!([{"op": "upsert", "collection": "prs", "key": "17", "patch": {}}])).await;
    let (s, v) = click(&t, &id, "hello").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(v.as_str().unwrap().contains("run actions are off"), "{v}");
}

#[cfg(unix)]
#[tokio::test]
async fn run_executes_the_user_argv_and_reports_to_the_agent() {
    let t = T::new();
    user_config(&t, true);
    let id = t.mission("m").await;
    t.ops(&id, json!([{"op": "upsert", "collection": "prs", "key": "17", "patch": {}}])).await;
    let (s, v) = click(&t, &id, "hello").await;
    assert_eq!((s, v["exit"].as_i64()), (StatusCode::OK, Some(0)), "{v}");
    let (s, v) = click(&t, &id, "slow").await;
    assert_eq!((s, v["exit"].clone()), (StatusCode::OK, Value::Null), "{v}");
    let (_, view) = t.get(&format!("/api/missions/{id}")).await;
    let runs: Vec<_> =
        view["events"].as_array().unwrap().iter().filter(|e| e["data"]["kind"] == "run").cloned().collect();
    assert_eq!(runs[0]["text"], "Hello: exit 0");
    assert_eq!(runs[0]["data"]["output"], "hi 17\n");
    assert!(runs[1]["data"]["output"].as_str().unwrap().contains("timed out"));
}

#[tokio::test]
async fn the_mission_layer_cannot_rewrite_or_enable_a_run_action() {
    let t = T::new();
    user_config(&t, false);
    let id = t.mission("m").await;
    let path = format!("/api/missions/{id}/config");
    for patch in [
        json!({"actions": {"hello": {"command": ["rm", "-rf", "/tmp/x"]}}}),
        json!({"permissions": {"run_actions": true}}),
        json!({"actions": {"evil": {"kind": "run", "command": ["id"]}}}),
    ] {
        let (s, v) = t.post(&path, Some(ADMIN), json!({"patch": patch})).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{patch}: {v}");
    }
}
