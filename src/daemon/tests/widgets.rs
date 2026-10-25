//! Script widgets: served only from user/project `widgets/`, only for a script key a file set.

use super::*;

async fn script(t: &T, id: &str, file: &str) -> (StatusCode, String) {
    t.raw(Request::builder().uri(format!("/m/{id}/widgets/{file}")).body(Body::empty()).unwrap()).await
}

#[tokio::test]
async fn user_scripts_are_served_and_nothing_else() {
    let t = T::new();
    std::fs::create_dir_all(t.user.path().join("widgets")).unwrap();
    std::fs::write(t.user.path().join("widgets/heat.js"), "MissionWidget.define('heat', () => 'hot');").unwrap();
    std::fs::write(t.user.path().join("secret.js"), "nope").unwrap();
    std::fs::write(
        t.user.path().join("config.toml"),
        "[widgets.heat]\nscript = \"widgets/heat.js\"\n[widgets.sneak]\nscript = \"widgets/../secret.js\"\n[widgets.plain]\nbase = \"table\"\n",
    )
    .unwrap();
    let id = t.mission("m").await;
    let (s, body) = script(&t, &id, "heat.js").await;
    assert_eq!((s, body.contains("define('heat'")), (StatusCode::OK, true));
    for bad in ["sneak.js", "plain.js", "nope.js", "heat"] {
        assert_eq!(script(&t, &id, bad).await.0, StatusCode::NOT_FOUND, "{bad}");
    }
    let (s, _) = t
        .post(
            &format!("/api/missions/{id}/config"),
            Some(ADMIN),
            json!({"patch": {"widgets": {"heat": {"script": "widgets/heat.js"}}}}),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "the mission layer can't point a widget at a script");
}
