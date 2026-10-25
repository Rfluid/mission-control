//! In-process HTTP tests: `router` + an in-memory store, driven with `tower::ServiceExt::oneshot`.

use super::*;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use serde_json::{Value, json};
use std::path::Path;
use tower::ServiceExt;

mod brief;
mod config;
mod page_events;
mod pages;
mod run;
mod widgets;

const ADMIN: &str = "admin-token";

struct T {
    router: Router,
    /// The user config layer: empty unless a test writes to it. Never the real one.
    user: tempfile::TempDir,
    /// State dir (file-layer history) and project dirs.
    dir: tempfile::TempDir,
}

impl T {
    fn new() -> Self {
        let user = tempfile::tempdir().unwrap();
        let store = Store::open(Path::new(":memory:")).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let app = App::new(store, ADMIN.into(), 7466, Some(user.path().to_path_buf()), dir.path().join("state"));
        Self { router: router(app), user, dir }
    }

    async fn raw(&self, req: Request<Body>) -> (StatusCode, String) {
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    async fn call(&self, method: Method, path: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(path);
        if let Some(t) = token {
            req = req.header("authorization", format!("Bearer {t}"));
        }
        let body = match body {
            Some(b) => {
                req = req.header("content-type", "application/json");
                Body::from(b.to_string())
            }
            None => Body::empty(),
        };
        let (status, text) = self.raw(req.body(body).unwrap()).await;
        (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
    }

    async fn get(&self, path: &str) -> (StatusCode, Value) {
        self.call(Method::GET, path, None, None).await
    }

    async fn post(&self, path: &str, token: Option<&str>, body: Value) -> (StatusCode, Value) {
        self.call(Method::POST, path, token, Some(body)).await
    }

    /// The mission token, read from the page like the browser gets it.
    async fn token(&self, id: &str) -> String {
        let req = Request::builder().uri(format!("/m/{id}")).body(Body::empty()).unwrap();
        let (_, html) = self.raw(req).await;
        html.split("token: \"").nth(1).unwrap().split('"').next().unwrap().to_string()
    }

    /// A page event with `token` in `X-Mission-Token`.
    async fn page(&self, id: &str, token: Option<&str>, body: Value) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/missions/{id}/events"))
            .header("content-type", "application/json");
        if let Some(t) = token {
            req = req.header("x-mission-token", t);
        }
        let (status, text) = self.raw(req.body(Body::from(body.to_string())).unwrap()).await;
        (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
    }

    async fn ops(&self, id: &str, ops: Value) {
        let (s, v) = self.post(&format!("/api/missions/{id}/ops"), Some(ADMIN), ops).await;
        assert_eq!(s, StatusCode::OK, "{v}");
    }

    async fn mission(&self, title: &str) -> String {
        let (s, v) = self.post("/api/missions", Some(ADMIN), json!({"title": title, "template": "minimal"})).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        v["mission"]["id"].as_str().unwrap().to_string()
    }
}

#[tokio::test]
async fn health_names_the_daemon_and_its_start() {
    let t = T::new();
    let (s, v) = t.get("/health").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["name"], "mission-control");
    assert!(v["started"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn every_write_needs_the_admin_token() {
    let t = T::new();
    let id = t.mission("m").await;
    let writes = [
        ("/api/missions".to_string(), json!({"title": "x"})),
        (format!("/api/missions/{id}/ops"), json!([])),
        (format!("/api/missions/{id}/config"), json!({"patch": {}})),
        (format!("/api/missions/{id}/attach"), json!({"session": "s"})),
    ];
    for (path, body) in &writes {
        for token in [None, Some("wrong"), Some("")] {
            let (s, _) = t.post(path, token, body.clone()).await;
            assert_eq!(s, StatusCode::UNAUTHORIZED, "{path} with {token:?}");
        }
        let (s, v) = t.post(path, Some(ADMIN), body.clone()).await;
        assert_eq!(s, StatusCode::OK, "{path}: {v}");
    }
    // Reads are open: without CORS headers other origins can't read the responses.
    for path in ["/api/missions".to_string(), format!("/api/missions/{id}"), format!("/api/missions/{id}/config")] {
        assert_eq!(t.get(&path).await.0, StatusCode::OK, "{path}");
    }
}

#[tokio::test]
async fn unknown_missions_are_404_everywhere() {
    let t = T::new();
    for path in ["/api/missions/msn_nope", "/api/missions/msn_nope/config", "/m/msn_nope"] {
        assert_eq!(t.get(path).await.0, StatusCode::NOT_FOUND, "{path}");
    }
    for (path, body) in [
        ("/api/missions/msn_nope/ops", json!([])),
        ("/api/missions/msn_nope/config", json!({"patch": {}})),
        ("/api/missions/msn_nope/attach", json!({})),
    ] {
        assert_eq!(t.post(path, Some(ADMIN), body).await.0, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn config_patches_with_errors_are_refused_and_not_stored() {
    let t = T::new();
    let id = t.mission("m").await;
    let path = format!("/api/missions/{id}/config");
    let bad = json!({"patch": {"actions": {"x": {"kind": "run", "command": ["ls"]}}}});
    let (s, v) = t.post(&path, Some(ADMIN), bad).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v[0]["path"], "actions.x", "the refusal lists the issues: {v}");
    let (s, v) = t.post(&path, Some(ADMIN), json!({"patch": {"vars": {"a": 1}}, "note": "ok"})).await;
    assert_eq!((s, v["version"].as_i64()), (StatusCode::OK, Some(2)), "the refused patch took no version");
    let (_, v) = t.get(&path).await;
    assert_eq!(v["provenance"]["vars.a"], "mission v2");
}

#[tokio::test]
async fn a_batch_is_all_or_nothing() {
    let t = T::new();
    let id = t.mission("m").await;
    let ops = json!([
        {"op": "upsert", "collection": "prs", "key": "1", "patch": {"t": 1}},
        {"op": "log", "text": "x"},
        {"op": "upsert", "collection": "prs", "key": "", "patch": {}},
    ]);
    let (s, v) = t.post(&format!("/api/missions/{id}/ops"), Some(ADMIN), ops).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert!(v.as_str().unwrap().contains("key must be"), "{v}");
    let (_, view) = t.get(&format!("/api/missions/{id}")).await;
    assert_eq!(view["records"], json!([]));
    assert!(view["events"].as_array().unwrap().iter().all(|e| e["kind"] != "log"), "the log op rolled back too");
    // A malformed op is rejected before anything runs.
    let (s, _) = t.post(&format!("/api/missions/{id}/ops"), Some(ADMIN), json!([{"op": "exec", "code": "x"}])).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn sessions_refuse_duplicate_titles_and_attach_rebinds() {
    let t = T::new();
    let body = json!({"title": "Run", "session": "ses_a"});
    let (s, v) = t.post("/api/missions", Some(ADMIN), body.clone()).await;
    assert_eq!(s, StatusCode::OK);
    let id = v["mission"]["id"].as_str().unwrap().to_string();
    let (s, v) = t.post("/api/missions", Some(ADMIN), body.clone()).await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert!(v.as_str().unwrap().contains(&id), "names the existing mission: {v}");
    let forced = json!({"title": "Run", "session": "ses_a", "force": true});
    assert_eq!(t.post("/api/missions", Some(ADMIN), forced).await.0, StatusCode::OK);
    let (s, v) = t.post(&format!("/api/missions/{id}/attach"), Some(ADMIN), json!({"session": "ses_b"})).await;
    assert_eq!((s, v["mission"]["session"].as_str()), (StatusCode::OK, Some("ses_b")));
    assert_eq!(v["url"], format!("http://127.0.0.1:7466/m/{id}"));
}

#[tokio::test]
async fn unknown_templates_are_refused_and_user_templates_count() {
    let t = T::new();
    let (s, v) = t.post("/api/missions", Some(ADMIN), json!({"title": "x", "template": "nope"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(v.as_str().unwrap().contains("have: minimal"), "{v}");
    std::fs::create_dir_all(t.user.path().join("templates")).unwrap();
    std::fs::write(t.user.path().join("templates/mine.toml"), "extends = \"minimal\"\n").unwrap();
    let (s, _) = t.post("/api/missions", Some(ADMIN), json!({"title": "x", "template": "mine"})).await;
    assert_eq!(s, StatusCode::OK, "the injected user dir is read");
}

#[tokio::test]
async fn the_page_escapes_the_title_and_carries_only_its_token() {
    let t = T::new();
    let id = t.mission("<script>alert(1)</script>").await;
    let other = t.mission("other").await;
    let req = Request::builder().uri(format!("/m/{id}")).body(Body::empty()).unwrap();
    let (s, html) = t.raw(req).await;
    assert_eq!(s, StatusCode::OK);
    assert!(!html.contains("<script>alert(1)"), "title escaped");
    assert!(html.contains("&lt;script&gt;alert(1)"));
    let req = Request::builder().uri(format!("/m/{other}")).body(Body::empty()).unwrap();
    let (_, other_html) = t.raw(req).await;
    let token = |h: &str| h.split("token: \"").nth(1).unwrap().split('"').next().unwrap().to_string();
    assert_eq!(token(&html).len(), 32);
    assert_ne!(token(&html), token(&other_html));
}

#[tokio::test]
async fn no_cors_headers_are_ever_sent() {
    let t = T::new();
    let req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/missions")
        .header("origin", "http://evil.example")
        .header("access-control-request-method", "POST")
        .header("access-control-request-headers", "authorization, x-mission-token")
        .body(Body::empty())
        .unwrap();
    let resp = t.router.clone().oneshot(req).await.unwrap();
    assert!(resp.headers().keys().all(|k| !k.as_str().starts_with("access-control-")), "{:?}", resp.headers());
}

#[tokio::test]
async fn other_host_names_are_refused() {
    let t = T::new();
    let id = t.mission("m").await;
    let get = |host: &str, path: String| Request::builder().uri(path).header("host", host).body(Body::empty()).unwrap();
    for host in ["evil.example:7466", "127.0.0.1.nip.io:7466", "127.0.0.1:9999", "localhost"] {
        for path in ["/api/missions".to_string(), format!("/m/{id}")] {
            assert_eq!(t.raw(get(host, path.clone())).await.0, StatusCode::MISDIRECTED_REQUEST, "{host} {path}");
        }
    }
    for host in ["127.0.0.1:7466", "localhost:7466", "LOCALHOST:7466"] {
        assert_eq!(t.raw(get(host, format!("/m/{id}"))).await.0, StatusCode::OK, "{host}");
    }
}
