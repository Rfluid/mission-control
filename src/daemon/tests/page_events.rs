//! Page events (mission token, limits, what each action does), `wait` cursors and the SSE stream.

use super::*;
use std::time::Duration;

async fn need(t: &T, id: &str) {
    t.ops(
        id,
        json!([{"op": "upsert", "collection": "needs", "key": "N-1", "state": "open", "patch": {"question": "Ship?"}}]),
    )
    .await;
}

#[tokio::test]
async fn page_events_need_this_missions_token() {
    let t = T::new();
    let (one, two) = (t.mission("a").await, t.mission("b").await);
    need(&t, &one).await;
    let (tok_one, tok_two) = (t.token(&one).await, t.token(&two).await);
    let ev = json!({"kind": "answer", "need": "N-1", "answer": "yes"});
    for token in [None, Some("wrong"), Some(tok_two.as_str()), Some(ADMIN)] {
        assert_eq!(t.page(&one, token, ev.clone()).await.0, StatusCode::UNAUTHORIZED, "{token:?}");
    }
    let (s, v) = t.page(&one, Some(&tok_one), ev).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    // The page token can't write through the agent API.
    let (s, _) = t.post(&format!("/api/missions/{one}/ops"), Some(&tok_one), json!([])).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn answering_sets_the_need_answered_and_logs_it() {
    let t = T::new();
    let id = t.mission("m").await;
    need(&t, &id).await;
    let tok = t.token(&id).await;
    let (s, _) = t.page(&id, Some(&tok), json!({"kind": "answer", "need": "N-9", "answer": "x"})).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = t.page(&id, Some(&tok), json!({"kind": "answer", "need": "N-1", "answer": "  "})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = t.page(&id, Some(&tok), json!({"kind": "answer", "need": "N-1", "answer": "later"})).await;
    assert_eq!(s, StatusCode::OK);
    let (_, view) = t.get(&format!("/api/missions/{id}")).await;
    let n = &view["records"][0];
    assert_eq!((n["state"].as_str(), n["body"]["answer"].as_str()), (Some("answered"), Some("later")));
    let last = view["events"].as_array().unwrap().last().unwrap().clone();
    assert_eq!((last["kind"].as_str(), last["data"]["question"].as_str()), (Some("answer"), Some("Ship?")));
}

#[tokio::test]
async fn actions_do_what_the_config_says() {
    let t = T::new();
    let id = t.mission("m").await;
    let patch = json!({"patch": {"vars": {"repo": "a/b"}, "actions": {
        "rerun": {"label": "Re-run", "kind": "ask_agent", "message": "Re-run {row.key} in {vars.repo}"},
        "pass": {"label": "Pass", "kind": "set", "state": "passed"},
        "flag": {"label": "Flag", "kind": "set", "field": "flagged", "value": true},
        "web": {"label": "Web", "kind": "open", "url": "https://x"},
    }}});
    assert_eq!(t.post(&format!("/api/missions/{id}/config"), Some(ADMIN), patch).await.0, StatusCode::OK);
    t.ops(&id, json!([{"op": "upsert", "collection": "prs", "key": "17", "state": "open", "patch": {}}])).await;
    let tok = t.token(&id).await;
    let act = |a: &str| json!({"kind": "action", "action": a, "collection": "prs", "key": "17", "text": "please"});
    assert_eq!(t.page(&id, Some(&tok), act("rerun")).await.0, StatusCode::OK);
    assert_eq!(t.page(&id, Some(&tok), act("pass")).await.0, StatusCode::OK);
    assert_eq!(t.page(&id, Some(&tok), act("flag")).await.0, StatusCode::OK);
    for bad in ["web", "nope"] {
        assert_eq!(t.page(&id, Some(&tok), act(bad)).await.0, StatusCode::BAD_REQUEST, "{bad}");
    }
    let (s, _) = t.page(&id, Some(&tok), json!({"kind": "action", "action": "customize"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "customize needs text");
    let (s, _) = t.page(&id, Some(&tok), json!({"kind": "action", "action": "customize", "text": "hide spend"})).await;
    assert_eq!(s, StatusCode::OK);
    let (_, view) = t.get(&format!("/api/missions/{id}")).await;
    let pr = &view["records"][0];
    assert_eq!((pr["state"].as_str(), &pr["body"]["flagged"]), (Some("passed"), &json!(true)));
    let texts: Vec<_> =
        view["events"].as_array().unwrap().iter().map(|e| (e["kind"].clone(), e["text"].clone())).collect();
    assert!(texts.contains(&(json!("action"), json!("Re-run 17 in a/b\nplease"))), "{texts:?}");
    assert!(texts.contains(&(json!("customize"), json!("hide spend"))), "{texts:?}");
}

#[tokio::test]
async fn page_events_are_limited_in_size_and_rate() {
    let t = T::new();
    let id = t.mission("m").await;
    let tok = t.token(&id).await;
    let huge = "x".repeat(page_body_limit() + 1);
    let (s, _) = t.page(&id, Some(&tok), json!({"kind": "action", "action": "customize", "text": huge})).await;
    assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE);
    let ev = json!({"kind": "action", "action": "customize", "text": "x"});
    let mut last = StatusCode::OK;
    for _ in 0..70 {
        last = t.page(&id, Some(&tok), ev.clone()).await.0;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
}

fn page_body_limit() -> usize {
    super::super::page::MAX_BODY_BYTES
}

#[tokio::test]
async fn wait_needs_the_admin_token() {
    let t = T::new();
    let id = t.mission("m").await;
    let tok = t.token(&id).await;
    let path = format!("/api/missions/{id}/wait?timeout=0");
    for token in [None, Some("nope"), Some(tok.as_str())] {
        assert_eq!(t.call(Method::GET, &path, token, None).await.0, StatusCode::UNAUTHORIZED, "{token:?}");
    }
    assert_eq!(t.call(Method::GET, &path, Some(ADMIN), None).await.0, StatusCode::OK);
}

#[tokio::test]
async fn wait_cursor_advances_and_new_sessions_continue() {
    let t = T::new();
    let id = t.mission("m").await;
    need(&t, &id).await;
    let tok = t.token(&id).await;
    t.ops(&id, json!([{"op": "log", "text": "agent noise"}])).await;
    t.page(&id, Some(&tok), json!({"kind": "answer", "need": "N-1", "answer": "go"})).await;
    let wait = |s: &str| format!("/api/missions/{id}/wait?session={s}&timeout=0");
    let (_, v) = t.call(Method::GET, &wait("a"), Some(ADMIN), None).await;
    let evs = v["events"].as_array().unwrap();
    assert_eq!((evs.len(), evs[0]["kind"].as_str(), v["timed_out"].as_bool()), (1, Some("answer"), Some(false)), "{v}");
    let (_, v) = t.call(Method::GET, &wait("a"), Some(ADMIN), None).await;
    assert_eq!((v["events"].as_array().unwrap().len(), v["timed_out"].as_bool()), (0, Some(true)), "delivered once");
    let (_, v) = t.call(Method::GET, &wait("restarted"), Some(ADMIN), None).await;
    assert!(v["events"].as_array().unwrap().is_empty(), "a new session continues where the last one stopped");
    let (_, v) = t.call(Method::GET, &format!("{}&since=0", wait("a")), Some(ADMIN), None).await;
    assert_eq!(v["events"].as_array().unwrap().len(), 1, "an explicit since replays");
}

#[tokio::test]
async fn a_waiting_agent_wakes_on_a_page_event() {
    let t = T::new();
    let id = t.mission("m").await;
    need(&t, &id).await;
    let tok = t.token(&id).await;
    let router = t.router.clone();
    let path = format!("/api/missions/{id}/wait?session=s&timeout=10");
    let waiter = tokio::spawn(async move {
        let req = Request::builder().uri(path).header("authorization", format!("Bearer {ADMIN}")).body(Body::empty());
        let resp = router.oneshot(req.unwrap()).await.unwrap();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice::<Value>(&bytes).unwrap()
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    let started = std::time::Instant::now();
    t.page(&id, Some(&tok), json!({"kind": "answer", "need": "N-1", "answer": "now"})).await;
    let v = tokio::time::timeout(Duration::from_secs(2), waiter).await.unwrap().unwrap();
    assert_eq!(v["events"][0]["data"]["answer"], "now");
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[tokio::test]
async fn the_stream_carries_record_changes() {
    use http_body_util::BodyExt;
    let t = T::new();
    let id = t.mission("m").await;
    let req = Request::builder().uri(format!("/api/missions/{id}/stream")).body(Body::empty()).unwrap();
    let resp = t.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.headers()["content-type"], "text/event-stream");
    let mut body = resp.into_body();
    t.ops(&id, json!([{"op": "upsert", "collection": "prs", "key": "1", "state": "open", "patch": {"t": 1}}])).await;
    let frame = tokio::time::timeout(Duration::from_secs(2), body.frame()).await.unwrap().unwrap().unwrap();
    let text = String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap();
    assert!(text.starts_with("event: change\ndata: "), "{text}");
    let data: Value = serde_json::from_str(text.lines().nth(1).unwrap().trim_start_matches("data: ")).unwrap();
    assert_eq!((data["records"][0]["key"].as_str(), data["config"].as_bool()), (Some("1"), Some(false)));
    let req = Request::builder().uri("/api/missions/msn_nope/stream").body(Body::empty()).unwrap();
    assert_eq!(t.router.clone().oneshot(req).await.unwrap().status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn only_needs_collections_can_be_answered_from_the_page() {
    let t = T::new();
    let id = t.mission("m").await;
    t.ops(&id, json!([{"op": "upsert", "collection": "prs", "key": "1", "state": "open", "patch": {}}])).await;
    let tok = t.token(&id).await;
    let (s, _) =
        t.page(&id, Some(&tok), json!({"kind": "answer", "need": "1", "collection": "prs", "answer": "x"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let patch = json!({"patch": {"sections": [{"id": "asks", "widget": "needs", "collection": "asks"}]}});
    t.post(&format!("/api/missions/{id}/config"), Some(ADMIN), patch).await;
    t.ops(&id, json!([{"op": "upsert", "collection": "asks", "key": "A", "state": "open", "patch": {}}])).await;
    let (s, _) =
        t.page(&id, Some(&tok), json!({"kind": "answer", "need": "A", "collection": "asks", "answer": "y"})).await;
    assert_eq!(s, StatusCode::OK, "a collection a needs section shows");
}
