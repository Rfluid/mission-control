//! `brief`: needs, unread page events (peeked, not consumed), `[brief].collections`, within the budget.

use super::*;

#[tokio::test]
async fn brief_lists_what_matters_within_budget_and_leaves_the_cursor() {
    let t = T::new();
    let id = t.mission("Brief me").await;
    t.post(
        &format!("/api/missions/{id}/config"),
        Some(ADMIN),
        json!({"patch": {"brief": {"collections": ["needs", "prs"]}}}),
    )
    .await;
    let mut ops = vec![
        json!({"op": "upsert", "collection": "needs", "key": "N-1", "state": "open", "patch": {"question": "Ship it?"}}),
    ];
    for n in 0..40 {
        ops.push(json!({"op": "upsert", "collection": "prs", "key": format!("{n}"), "state": if n % 2 == 0 { "open" } else { "merged" }, "patch": {"title": format!("PR number {n}")}}));
    }
    t.ops(&id, Value::Array(ops)).await;
    let tok = t.token(&id).await;
    t.page(&id, Some(&tok), json!({"kind": "action", "action": "customize", "text": "hide spend"})).await;
    let (s, v) = t.get(&format!("/api/missions/{id}/brief?session=s&max_chars=600")).await;
    assert_eq!(s, StatusCode::OK);
    let text = v["text"].as_str().unwrap();
    assert!(text.starts_with("# Brief me ("), "{text}");
    assert!(text.contains("N-1 [open] Ship it?") && text.contains("customize: hide spend"), "{text}");
    assert!(text.contains("## prs (40: merged 20, open 20)"), "{text}");
    assert!(
        v["truncated"].as_bool().unwrap() && v["chars"].as_u64().unwrap() <= 600 && text.contains("more"),
        "{text}"
    );
    let (_, w) = t.call(Method::GET, &format!("/api/missions/{id}/wait?session=s&timeout=0"), Some(ADMIN), None).await;
    assert_eq!(w["events"].as_array().unwrap().len(), 1, "brief peeks; the event is still there for wait");
    let (_, v) = t.get(&format!("/api/missions/{id}/brief?session=s")).await;
    assert!(v["text"].as_str().unwrap().contains("not yet received (0)"));
}

#[tokio::test]
async fn doctor_reports_issues_empty_sections_and_unused_actions() {
    let t = T::new();
    let id = t.mission("Doc").await;
    let patch = json!({"patch": {
        "actions": {"lonely": {"kind": "ask_agent", "label": "Lonely"}},
        "sections": [{"id": "prs", "widget": "table", "collection": "prs"}, {"id": "odd", "widget": "nope"}],
    }});
    assert_eq!(t.post(&format!("/api/missions/{id}/config"), Some(ADMIN), patch).await.0, StatusCode::OK);
    let (s, v) = t.get("/api/doctor").await;
    assert_eq!(s, StatusCode::OK);
    let m = v.as_array().unwrap().iter().find(|m| m["id"] == id.as_str()).unwrap();
    assert_eq!(m["unused_actions"], json!(["lonely"]));
    assert!(m["empty_sections"].as_array().unwrap().iter().any(|x| x == "prs"), "{m}");
    assert!(m["issues"].as_array().unwrap().iter().any(|i| i["path"] == "sections[odd].widget"), "{m}");
    assert_eq!(m["idle"], false);
}

#[tokio::test]
async fn archive_fork_and_summaries() {
    let t = T::new();
    let id = t.mission("Old").await;
    t.post(&format!("/api/missions/{id}/config"), Some(ADMIN), json!({"patch": {"vars": {"x": 1}}})).await;
    t.ops(
        &id,
        json!([
            {"op": "upsert", "collection": "needs", "key": "N-1", "state": "open", "patch": {}},
            {"op": "upsert", "collection": "run", "key": "run", "state": "at_risk", "patch": {}},
        ]),
    )
    .await;
    let (_, listed) = t.get("/api/missions?summary=true").await;
    assert_eq!(listed[0]["summary"]["open_needs"], 1);
    assert_eq!(listed[0]["summary"]["health"], "at_risk");
    let (status, f) = t.post(&format!("/api/missions/{id}/fork"), Some(ADMIN), json!({"title": "New"})).await;
    assert_eq!(status, StatusCode::OK);
    let fid = f["mission"]["id"].as_str().unwrap();
    let (_, fv) = t.get(&format!("/api/missions/{fid}")).await;
    assert_eq!(
        (fv["config"]["vars"]["x"].as_i64(), fv["records"].as_array().unwrap().len()),
        (Some(1), 2),
        "config and records copied"
    );
    assert!(fv["events"][0]["text"].as_str().unwrap().starts_with("forked from Old"));
    let (_, nr) =
        t.post(&format!("/api/missions/{id}/fork"), Some(ADMIN), json!({"title": "Bare", "records": false})).await;
    let (_, nv) = t.get(&format!("/api/missions/{}", nr["mission"]["id"].as_str().unwrap())).await;
    assert_eq!(nv["records"], json!([]));
    assert_eq!(t.post(&format!("/api/missions/{id}/archive"), None, json!({})).await.0, StatusCode::UNAUTHORIZED);
    let (_, archived) = t.post(&format!("/api/missions/{id}/archive"), Some(ADMIN), json!({})).await;
    assert_eq!(archived["archived"], true);
    let (_, live) = t.get("/api/missions").await;
    assert!(live.as_array().unwrap().iter().all(|m| m["id"] != id.as_str()), "archived missions leave the list");
    let (_, all) = t.get("/api/missions?archived=true").await;
    assert!(all.as_array().unwrap().iter().any(|m| m["id"] == id.as_str()));
    let (_, restored) = t.post(&format!("/api/missions/{id}/archive"), Some(ADMIN), json!({"archived": false})).await;
    assert_eq!(restored["archived"], false);
}
