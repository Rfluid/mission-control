//! Config writes on every layer: user and project files, dry runs and drafts, undo and history.

use super::*;

async fn in_project(t: &T, title: &str) -> String {
    let proj = t.dir.path().join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let body = json!({"title": title, "template": "minimal", "project_dir": proj});
    let (s, v) = t.post("/api/missions", Some(ADMIN), body).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v["mission"]["id"].as_str().unwrap().to_string()
}

async fn patch(t: &T, id: &str, body: Value) -> (StatusCode, Value) {
    t.post(&format!("/api/missions/{id}/config"), Some(ADMIN), body).await
}

#[tokio::test]
async fn user_and_project_patches_write_their_files_and_reach_every_mission() {
    let t = T::new();
    let (a, other_id) = (in_project(&t, "a").await, t.mission("b").await);
    std::fs::write(t.user.path().join("config.toml"), "# mine\n[vars]\nx = 1 # keep\n").unwrap();
    let (s, v) = patch(&t, &a, json!({"layer": "user", "patch": {"vars": {"y": 2}}, "note": "always y"})).await;
    assert_eq!((s, v["changed"].as_bool()), (StatusCode::OK, Some(true)), "{v}");
    let text = std::fs::read_to_string(t.user.path().join("config.toml")).unwrap();
    assert!(text.starts_with("# mine") && text.contains("x = 1 # keep") && text.contains("y = 2"), "{text}");
    let (_, other) = t.get(&format!("/api/missions/{other_id}/config")).await;
    assert!(other["provenance"]["vars.y"].as_str().unwrap().starts_with("user ("), "every mission sees the user layer");
    let (s, _) = patch(&t, &a, json!({"layer": "project", "patch": {"vars": {"z": 3}}})).await;
    assert_eq!(s, StatusCode::OK);
    assert!(t.dir.path().join("proj/.mission-control/config.toml").is_file());
    let (_, mine) = t.get(&format!("/api/missions/{a}/config")).await;
    assert!(mine["provenance"]["vars.z"].as_str().unwrap().starts_with("project ("));
    let (s, v) = patch(&t, &other_id, json!({"layer": "project", "patch": {"vars": {"z": 3}}})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "b has no project dir: {v}");
    let (_, again) = patch(&t, &a, json!({"layer": "user", "patch": {"vars": {"y": 2}}})).await;
    assert_eq!(again["changed"], false, "same content: no write");
}

#[tokio::test]
async fn file_layers_may_hold_run_actions_but_type_errors_are_refused() {
    let t = T::new();
    let a = in_project(&t, "a").await;
    let run = json!({"actions": {"logs": {"kind": "run", "command": ["ls"]}}, "permissions": {"run_actions": true}});
    assert_eq!(patch(&t, &a, json!({"layer": "user", "patch": run})).await.0, StatusCode::OK);
    let bad = json!({"layer": "project", "patch": {"sections": [{"id": "x", "widget": "feed", "columns": [{"id": "c", "as": "glow"}]}]}});
    let (s, v) = patch(&t, &a, bad).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v[0]["path"], "sections[x].columns[c].as", "{v}");
    assert!(!t.dir.path().join("proj/.mission-control/config.toml").exists(), "nothing written");
}

#[tokio::test]
async fn dry_runs_store_nothing_and_leave_a_previewable_draft() {
    let t = T::new();
    let a = in_project(&t, "a").await;
    for layer in ["mission", "user"] {
        let body =
            json!({"layer": layer, "dry_run": true, "patch": {"sections": [{"id": "activity", "title": "Log"}]}});
        let (s, v) = patch(&t, &a, body).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let draft = v["draft"].as_str().unwrap();
        assert!(v["preview_url"].as_str().unwrap().ends_with(&format!("/m/{a}?draft={draft}")));
        let (_, now) = t.get(&format!("/api/missions/{a}")).await;
        let title = |view: &Value| {
            view["config"]["sections"].as_array().unwrap().iter().find(|s| s["id"] == "activity").unwrap()["title"]
                .clone()
        };
        assert_eq!(title(&now), "Activity", "{layer}: not applied");
        let (_, preview) = t.get(&format!("/api/missions/{a}?draft={draft}")).await;
        assert_eq!(title(&preview), "Log", "{layer}: the draft shows it");
    }
    assert!(!t.user.path().join("config.toml").exists());
    assert_eq!(t.get(&format!("/api/missions/{a}?draft=drf_nope")).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn undo_walks_back_each_layer_and_history_lists_changes() {
    let t = T::new();
    let mid = in_project(&t, "a").await;
    patch(&t, &mid, json!({"patch": {"vars": {"m": 1}}, "note": "m one"})).await;
    patch(&t, &mid, json!({"layer": "user", "patch": {"vars": {"u": 1}}, "note": "u one"})).await;
    patch(&t, &mid, json!({"layer": "user", "patch": {"vars": {"u": 2}}, "note": "u two"})).await;
    let (_, h) = t.get(&format!("/api/missions/{mid}/config/history?layer=user")).await;
    let notes: Vec<_> = h["versions"].as_array().unwrap().iter().map(|v| v["note"].clone()).collect();
    assert_eq!(notes, [json!("u two"), json!("u one")]);
    let (_, h) = t.get(&format!("/api/missions/{mid}/config/history")).await;
    assert_eq!(h["versions"][0]["note"], "m one", "newest first");
    let undo_path = format!("/api/missions/{mid}/config/undo");
    let undo = |layer: &str| t.post(&undo_path, Some(ADMIN), json!({"layer": layer}));
    assert_eq!(undo("user").await.0, StatusCode::OK);
    let (_, c) = t.get(&format!("/api/missions/{mid}/config")).await;
    assert_eq!(c["config"]["vars"]["u"], 1);
    assert_eq!(undo("mission").await.1["undone"], 2);
    let (_, c) = t.get(&format!("/api/missions/{mid}/config")).await;
    assert!(c["config"]["vars"].get("m").is_none());
    assert_eq!(undo("user").await.0, StatusCode::OK);
    assert_eq!(undo("user").await.0, StatusCode::BAD_REQUEST, "nothing left to undo");
    assert!(!t.user.path().join("config.toml").exists(), "back to no file");
    let (s, _) = t.post(&format!("/api/missions/{mid}/config/undo"), None, json!({})).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_config_cache_notices_hand_edits_and_new_templates() {
    let t = T::new();
    let mid = t.mission("m").await;
    let path = format!("/api/missions/{mid}/config");
    assert!(t.get(&path).await.1["config"]["vars"].get("hand").is_none());
    std::fs::write(t.user.path().join("config.toml"), "[vars]\nhand = 1\n").unwrap();
    assert_eq!(t.get(&path).await.1["config"]["vars"]["hand"], 1, "edited by hand, picked up");
    std::fs::write(t.user.path().join("config.toml"), "[vars]\nhand = 22\n").unwrap();
    assert_eq!(t.get(&path).await.1["config"]["vars"]["hand"], 22);
    std::fs::create_dir_all(t.user.path().join("templates")).unwrap();
    std::fs::write(t.user.path().join("templates/minimal.toml"), "[vars]\nfrom_template = true\n").unwrap();
    assert_eq!(t.get(&path).await.1["config"]["vars"]["from_template"], true, "a new template file, picked up");
}

#[tokio::test]
async fn a_customized_mission_saves_as_a_template_new_missions_start_from() {
    let t = T::new();
    let mid = in_project(&t, "a").await;
    for p in [
        json!({"sections": [{"id": "status", "remove": true}, {"id": "prs", "widget": "table", "collection": "prs", "after": "needs"}]}),
        json!({"sections": [{"id": "prs", "title": "PRs"}], "vars": {"repo": "a/b"}, "mission": {"refresh": "poll:3"}}),
    ] {
        assert_eq!(patch(&t, &mid, json!({"patch": p})).await.0, StatusCode::OK);
    }
    let save_path = format!("/api/missions/{mid}/templates");
    let save = |body: Value| t.post(&save_path, Some(ADMIN), body);
    let (s, v) = save(json!({"name": "standup", "layer": "project", "description": "Daily"})).await;
    assert_eq!((s, v["extends"].as_str()), (StatusCode::OK, Some("minimal")), "{v}");
    let text = std::fs::read_to_string(t.dir.path().join("proj/.mission-control/templates/standup.toml")).unwrap();
    assert!(text.contains("extends = \"minimal\"") && !text.contains("template ="), "{text}");
    assert_eq!(save(json!({"name": "standup", "layer": "project"})).await.0, StatusCode::CONFLICT);
    assert_eq!(save(json!({"name": "../x"})).await.0, StatusCode::BAD_REQUEST);
    let proj = t.dir.path().join("proj");
    let (s, v) =
        t.post("/api/missions", Some(ADMIN), json!({"title": "b", "template": "standup", "project_dir": proj})).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let fresh = v["mission"]["id"].as_str().unwrap();
    let ids =
        |c: &Value| c["config"]["sections"].as_array().unwrap().iter().map(|s| s["id"].clone()).collect::<Vec<_>>();
    let (_, old) = t.get(&format!("/api/missions/{mid}/config")).await;
    let (_, new) = t.get(&format!("/api/missions/{fresh}/config")).await;
    assert_eq!(ids(&new), ids(&old));
    assert_eq!(new["config"]["vars"]["repo"], "a/b");
    assert_eq!(new["config"]["mission"]["refresh"], "poll:3");
    assert!(new["provenance"]["sections[prs].title"].as_str().unwrap().starts_with("project template"));
}
