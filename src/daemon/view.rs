//! The mission page and what it reads: the page shell (`/m/{id}`, `/m/{id}/{page}`), the view
//! (`GET /api/missions/{id}`, optionally one page's worth) and the static export.

use super::App;
use super::assets::{ASSETS, MISSION_HTML};
use super::config::{Overlay, resolve};
use super::handlers::{Res, found, internal};
use crate::config::pages;
use crate::model::View;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::Arc;

/// The collection the `needs` widget and the status strip read by default.
const NEEDS: &str = "needs";
/// Events sent with a view: the newest page.
const VIEW_EVENTS: i64 = 100;
pub(super) const VIEW_EVENTS_MAX: i64 = 500;

/// `GET /m/{id}/{page}`: the same page shell; the page script reads which page from the URL (an unknown one
/// shows Overview).
pub async fn page_at(State(app): State<Arc<App>>, Path((id, _page)): Path<(String, String)>) -> Res<Html<String>> {
    page(State(app), Path(id)).await
}

pub async fn page(State(app): State<Arc<App>>, Path(id): Path<String>) -> Res<Html<String>> {
    let (m, token) = found(&app, &id)?;
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    let mut html =
        MISSION_HTML.replace("{{ID}}", &esc(&m.id)).replace("{{TOKEN}}", &token).replace("{{TITLE}}", &esc(&m.title));
    // The redline overlay, when review is on (resolution errors just mean no overlay).
    if let Ok(r) = resolve(&app, &m, &Overlay::None) {
        let records = app.store().records(&id).map_err(internal)?;
        if let Some(snip) = super::review::snippet(&r.config, &records) {
            html = html.replace("</body>", &format!("{snip}</body>"));
        }
    }
    Ok(Html(html))
}

/// `GET /api/missions/{id}/export`: the page as one static HTML file: assets inlined, the view embedded, no
/// token (its buttons are inert), so it can be shared or published anywhere.
pub async fn export(State(app): State<Arc<App>>, Path(id): Path<String>) -> Res<Response> {
    let (mission, _) = found(&app, &id)?;
    let r = resolve(&app, &mission, &Overlay::None)?;
    let (records, events, event_count) = {
        let mut s = app.store();
        (
            s.records(&id).map_err(internal)?,
            s.events(&id, 0, VIEW_EVENTS_MAX).map_err(internal)?,
            s.event_count(&id).map_err(internal)?,
        )
    };
    let view = View {
        mission: mission.clone(),
        template: r.template.clone(),
        config: r.config.clone(),
        issues: r.issues.clone(),
        records,
        events,
        event_count,
    };
    // Inside <script>, `</` would end the element: escape it (and the JS line separators).
    let data = serde_json::to_string(&view)
        .map_err(internal)?
        .replace("</", "<\\/")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    let asset = |name: &str| ASSETS.iter().find(|(n, ..)| *n == name).map_or("", |(_, _, body)| *body);
    let mut html = MISSION_HTML
        .replace("{{TITLE}}", &esc(&mission.title))
        .replace(r#"<link rel="icon" href="/assets/favicon.svg" type="image/svg+xml">"#, "")
        .replace(
            r#"<script>window.MISSION = { id: "{{ID}}", token: "{{TOKEN}}" };</script>"#,
            &format!(
                "<script>window.MISSION = {{ id: \"{}\", token: \"\", snapshot: {data} }};</script>",
                esc(&mission.id)
            ),
        );
    // Images inline as data: URLs (percent-encoded SVG).
    for (name, ..) in ASSETS.iter().filter(|(_, mime, _)| *mime == "image/svg+xml") {
        let data: String = asset(name)
            .chars()
            .map(|c| match c {
                '"' => "'".to_string(),
                '#' | '%' | '<' | '>' | '\n' => format!("%{:02X}", c as u32),
                c => c.to_string(),
            })
            .collect();
        html = html.replace(&format!(r#"src="/assets/{name}""#), &format!(r#"src="data:image/svg+xml,{data}""#));
    }
    // Stylesheets inline, minus the bundled fonts (a snapshot is one file; it falls back to the system stacks).
    for (name, ..) in ASSETS.iter().filter(|(_, mime, _)| *mime == "text/css") {
        let css: String = asset(name).lines().filter(|l| !l.starts_with("@font-face")).collect::<Vec<_>>().join("\n");
        html = html.replace(
            &format!(r#"<link rel="stylesheet" href="/assets/{name}">"#),
            &format!("<style>\n{css}\n</style>"),
        );
    }
    for (name, ..) in ASSETS.iter().filter(|(_, mime, _)| *mime == "text/javascript") {
        html = html.replace(
            &format!(r#"<script src="/assets/{name}"></script>"#),
            &format!("<script>\n{}</script>", asset(name)),
        );
    }
    Ok(([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response())
}

#[derive(Deserialize)]
pub struct ViewQuery {
    /// A draft config change to preview (`config patch --dry-run` returns its id).
    draft: Option<String>,
    /// Newest events to include (default 100, at most 500).
    events: Option<i64>,
    /// `false` leaves the resolved config out (agents re-reading state rarely need it).
    config: Option<bool>,
    /// One page's worth: its sections, and records of the collections they read (plus `needs`).
    page: Option<String>,
}

pub async fn view(State(app): State<Arc<App>>, Path(id): Path<String>, Query(q): Query<ViewQuery>) -> Res<Json<View>> {
    let (mission, _) = found(&app, &id)?;
    let over = match &q.draft {
        Some(d) => app.draft(&id, d).ok_or((StatusCode::NOT_FOUND, format!("no draft `{d}` (drafts last an hour)")))?,
        None => Overlay::None,
    };
    let r = resolve(&app, &mission, &over)?;
    let (records, events) = {
        let mut s = app.store();
        (
            s.records(&id).map_err(internal)?,
            s.events(&id, 0, q.events.unwrap_or(VIEW_EVENTS).clamp(0, VIEW_EVENTS_MAX)).map_err(internal)?,
        )
    };
    let event_count = app.store().event_count(&id).map_err(internal)?;
    let mut config = r.config.clone();
    let mut records = records;
    if let Some(p) = &q.page {
        let known = pages::ids(&config);
        if !known.contains(p) {
            return Err((StatusCode::NOT_FOUND, format!("no page `{p}` (have: {})", known.join(", "))));
        }
        let sections = pages::sections_on(&config, p);
        // The status strip shows open needs on every page.
        let mut read = BTreeSet::from([NEEDS.to_string()]);
        for s in &sections {
            pages::collections(s, &mut read);
        }
        records.retain(|r| read.contains(&r.collection));
        config["sections"] = Value::Array(sections);
    }
    Ok(Json(View {
        mission,
        template: r.template.clone(),
        config: if q.config == Some(false) { Value::Null } else { config },
        issues: r.issues.clone(),
        records,
        events,
        event_count,
    }))
}
