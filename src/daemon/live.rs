//! Live delivery: changes fan out to open pages (SSE) and to agents waiting for page events (long-poll).

use super::App;
use super::handlers::{Res, admin, found, internal};
use crate::model::Event;
use crate::store::Applied;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};

/// Event kinds the page sends; `wait` returns only these.
pub const PAGE_KINDS: [&str; 3] = ["answer", "action", "customize"];
/// Longest long-poll, below common client and MCP tool timeouts.
const MAX_WAIT_SECS: u64 = 540;
const DEFAULT_WAIT_SECS: u64 = 120;
/// Most page events one `wait` returns.
const WAIT_BATCH: i64 = 100;
const KEEP_ALIVE: Duration = Duration::from_secs(15);

/// One change to a mission, as an open page receives it.
#[derive(Clone, Debug, Serialize)]
pub struct Change {
    pub mission: String,
    #[serde(flatten)]
    pub applied: Applied,
    /// The config changed: the page re-reads the view.
    pub config: bool,
}

impl App {
    /// Sends a change to every open page and waiting agent. No receivers is fine.
    pub(super) fn publish(&self, mission: &str, applied: Applied, config: bool) {
        if applied.is_empty() && !config {
            return;
        }
        tracing::debug!(mission, records = applied.records.len(), events = applied.events.len(), config, "change");
        let _ = self.tx.send(Change { mission: mission.into(), applied, config });
    }
}

/// `GET /api/missions/{id}/stream`: server-sent events, one `change` per write. A page that falls behind
/// gets a `reset` and re-reads the view.
pub async fn stream(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Res<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>> {
    found(&app, &id)?;
    let rx = BroadcastStream::new(app.tx.subscribe());
    let s = rx.filter_map(move |msg| match msg {
        Ok(c) if c.mission == id => Some(Ok(SseEvent::default().event("change").data(json!(c).to_string()))),
        Ok(_) => None,
        Err(_) => Some(Ok(SseEvent::default().event("reset").data("{}"))),
    });
    Ok(Sse::new(s).keep_alive(KeepAlive::new().interval(KEEP_ALIVE)))
}

#[derive(Deserialize)]
pub struct WaitQuery {
    /// Agent session whose cursor to use and advance.
    session: Option<String>,
    /// Explicit cursor; default the session's.
    since: Option<i64>,
    timeout: Option<u64>,
}

/// `GET /api/missions/{id}/wait` (admin): page events after the cursor, as soon as there are any, or none
/// after `timeout` seconds. Advances the session's cursor past what it returns.
pub async fn wait(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Query(q): Query<WaitQuery>,
) -> Res<Json<Value>> {
    admin(&app, &headers)?;
    found(&app, &id)?;
    let session = q.session.unwrap_or_else(|| "default".into());
    // Subscribe before the first read, so an event landing in between still wakes us.
    let mut rx = app.tx.subscribe();
    let since = match q.since {
        Some(s) => s,
        None => app.store().cursor(&id, &session).map_err(internal)?,
    };
    let secs = q.timeout.unwrap_or(DEFAULT_WAIT_SECS).min(MAX_WAIT_SECS);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let evs: Vec<Event> = app.store().events_of(&id, since, &PAGE_KINDS, WAIT_BATCH).map_err(internal)?;
        if let Some(last) = evs.last() {
            let next = last.seq;
            app.store().set_cursor(&id, &session, next).map_err(internal)?;
            return Ok(Json(json!({"events": evs, "next": next, "timed_out": false})));
        }
        loop {
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Ok(c)) if c.mission != id => {}
                Ok(Ok(_) | Err(RecvError::Lagged(_))) => break,
                Ok(Err(RecvError::Closed)) | Err(_) => {
                    return Ok(Json(json!({"events": [], "next": since, "timed_out": true})));
                }
            }
        }
    }
}

#[derive(Deserialize)]
pub struct OlderQuery {
    before: Option<i64>,
    limit: Option<i64>,
    /// With `key`: only events about that record (its history).
    collection: Option<String>,
    key: Option<String>,
}

/// Most events one page returns.
const PAGE_MAX: i64 = 500;

/// `GET /api/missions/{id}/events?before=<seq>&limit=<n>`: older events for the feed, oldest first.
/// `?collection=&key=`: the events about one record instead (the drawer's history).
pub async fn older(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Query(q): Query<OlderQuery>,
) -> Res<Json<Vec<Event>>> {
    found(&app, &id)?;
    let limit = q.limit.unwrap_or(50).clamp(1, PAGE_MAX);
    if let (Some(c), Some(k)) = (&q.collection, &q.key) {
        return Ok(Json(app.store().record_events(&id, c, k, limit).map_err(internal)?));
    }
    Ok(Json(app.store().events_before(&id, q.before, limit).map_err(internal)?))
}
