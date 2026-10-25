//! Pages: a mission-layer patch adds one and moves a section onto it; `?page=` reads one page's worth.

use super::*;

#[tokio::test]
async fn a_mission_patch_adds_a_page_and_moves_a_section_onto_it() {
    let t = T::new();
    let id = t.mission("m").await;
    t.ops(
        &id,
        json!([
            {"op": "upsert", "collection": "notes", "key": "status", "patch": {"text": "hi"}},
            {"op": "upsert", "collection": "needs", "key": "N-1", "state": "open", "patch": {"question": "?"}},
            {"op": "upsert", "collection": "other", "key": "x", "patch": {}},
        ]),
    )
    .await;
    // minimal: needs, status and the rail on Overview; the full feed on its own page.
    let (_, v) = t.get(&format!("/api/missions/{id}?page=overview")).await;
    assert_eq!(v["config"]["sections"].as_array().unwrap().len(), 4, "{v}");
    let (_, v) = t.get(&format!("/api/missions/{id}?page=activity")).await;
    assert_eq!(v["config"]["sections"][0]["id"], "activity");
    let patch = json!({"pages": [{"id": "architecture", "title": "Architecture"}], "sections": [{"id": "status", "page": "architecture"}]});
    let (s, v) = t.post(&format!("/api/missions/{id}/config"), Some(ADMIN), json!({"patch": patch})).await;
    assert_eq!(s, StatusCode::OK, "{v}");

    let (_, v) = t.get(&format!("/api/missions/{id}?page=architecture")).await;
    let ids: Vec<_> = v["config"]["sections"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["status"]);
    let colls: Vec<_> = v["records"].as_array().unwrap().iter().map(|r| r["collection"].as_str().unwrap()).collect();
    assert_eq!(colls, ["needs", "notes"], "the page's collections, plus needs for the strip");
    let (_, v) = t.get(&format!("/api/missions/{id}?page=overview")).await;
    assert!(v["config"]["sections"].as_array().unwrap().iter().all(|s| s["id"] != "status"));

    let (s, v) = t.get(&format!("/api/missions/{id}?page=ghost")).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert!(v.as_str().unwrap().contains("overview, activity, architecture"), "{v}");

    let (_, v) = t.get(&format!("/api/missions/{id}/config")).await;
    assert_eq!(v["provenance"]["pages[architecture].title"], "mission v2");
    assert_eq!(v["provenance"]["pages[overview].title"], "builtin");
    assert_eq!(v["provenance"]["sections[status].page"], "mission v2");

    let req = Request::builder().uri(format!("/m/{id}/architecture")).body(Body::empty()).unwrap();
    let (s, html) = t.raw(req).await;
    assert_eq!(s, StatusCode::OK);
    assert!(html.contains(&format!("id: \"{id}\"")));
}
