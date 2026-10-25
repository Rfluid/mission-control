//! `mission-control mcp`: MCP server over stdio, built on rmcp (ported from redline's `src/mcp/`).
//!
//! One server per agent session. Tools (in `tools`) wrap the blocking agent-side client ([`crate::api`]); each
//! has a CLI twin. The session binds one mission (`mission_new` / `mission_attach`, or `$MISSION_CONTROL_ID`), so tools
//! default to it. With `--channel` (Claude Code), page events are also pushed into the session as channel
//! notifications (ported from redline's channel push).

mod tools;

use crate::api::Api;
use crate::ids::new_id;
use crate::model::Event;
use rmcp::model::{
    CustomNotification, Implementation, JsonObject, ProtocolVersion, ServerCapabilities, ServerConfig,
    ServerNotification,
};
use rmcp::{Peer, RoleServer, ServerHandler, ServiceExt, tool_handler};
use serde_json::json;
use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const INSTRUCTIONS: &str = "mission-control gives this session a live mission control page that the user watches. \
Start with mission_new (or mission_attach with the old id after a restart) and give the user the url. \
Keep the page current with batch: upsert records into the collections the page shows (view lists the sections), \
log milestones, and put questions for the user in the `needs` collection (state `open`; question, options, \
recommendation, why). Update on state changes, not narration; batch several writes into one call. \
Change the page itself with config_patch. When the user acts on the page, `wait` returns it. \
Follow the mission-control skill for details.";

const CHANNEL_INSTRUCTIONS: &str = " This server is also a Claude Code channel: what the user does on the page of \
the mission bound to this session arrives by itself as <channel source=\"mission-control\" kind=... mission_id=...> \
messages, so you don't need to call wait. Handle each one like a wait result: an answer to a need, a button click \
asking you to do something, or a customize request (answer it with config_patch).";

/// Claude Code channels (<https://code.claude.com/docs/en/channels-reference>, as in redline).
const CHANNEL_CAPABILITY: &str = "claude/channel";
const CHANNEL_METHOD: &str = "notifications/claude/channel";
/// Claude Code doesn't register a channel on protocol revision 2026-07-28 (rmcp's default), so a channel
/// server offers only the revisions before it.
const CHANNEL_PROTOCOLS: &[ProtocolVersion] = &[
    ProtocolVersion::V_2024_11_05,
    ProtocolVersion::V_2025_03_26,
    ProtocolVersion::V_2025_06_18,
    ProtocolVersion::V_2025_11_25,
];
/// Long-poll length for the push loop; the back-off after an error or while no mission is bound.
const PUSH_WAIT_SECS: u64 = 300;
const PUSH_RETRY: Duration = Duration::from_secs(5);
const PUSH_IDLE: Duration = Duration::from_secs(1);

/// One per agent session (process).
#[derive(Clone)]
pub struct Server {
    /// Missions created here are stamped with it; `mission_attach` re-binds to it.
    session: Arc<str>,
    /// The mission tools act on when they aren't given one.
    bound: Arc<Mutex<Option<String>>>,
    /// Running as a Claude Code channel (`--channel`).
    channel: bool,
}

impl Server {
    #[must_use]
    pub fn new(session: &str, mission: Option<String>, channel: bool) -> Self {
        Self { session: session.into(), bound: Arc::new(Mutex::new(mission)), channel }
    }

    fn bound(&self) -> Option<String> {
        self.bound.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    fn bind(&self, id: &str) {
        *self.bound.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(id.to_string());
    }

    /// `given`, else the bound mission, else an error that says what to call first.
    fn target(&self, given: Option<String>) -> Result<String, String> {
        given
            .or_else(|| self.bound())
            .ok_or_else(|| "no mission bound to this session: call mission_new, or mission_attach with its id".into())
    }
}

// rmcp's `#[tool_handler]` expands to `async fn`s without `.await`; the lint (clippy ≥ 1.99) fires on that
// generated code, which we can't change.
#[allow(clippy::unused_async_trait_impl)]
#[tool_handler]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        let mut caps = ServerCapabilities::builder().enable_tools().build();
        let mut instructions = INSTRUCTIONS.to_string();
        if self.channel {
            caps.experimental = Some([(CHANNEL_CAPABILITY.to_string(), JsonObject::new())].into());
            instructions.push_str(CHANNEL_INSTRUCTIONS);
        }
        ServerConfig::new(caps)
            .with_server_info(Implementation::new("mission-control", env!("CARGO_PKG_VERSION")))
            .with_instructions(instructions)
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        if self.channel { Cow::Borrowed(CHANNEL_PROTOCOLS) } else { Cow::Borrowed(ProtocolVersion::KNOWN_VERSIONS) }
    }
}

/// Serves MCP on stdio until the client disconnects. Session: `$MISSION_CONTROL_SESSION`, else a new id.
pub async fn run(channel: bool) -> anyhow::Result<()> {
    let session =
        std::env::var("MISSION_CONTROL_SESSION").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| new_id("ses"));
    let mission = std::env::var("MISSION_CONTROL_ID").ok().filter(|s| !s.is_empty());
    let server = Server::new(&session, mission, channel);
    let running = server.clone().serve(rmcp::transport::stdio()).await?;
    if channel {
        tokio::spawn(push_loop(running.peer().clone(), server));
    }
    running.waiting().await?;
    Ok(())
}

/// Claude Code channels: push each page event of the bound mission into the session as it arrives. It shares
/// the session's `wait` cursor, so an event is delivered once, by whichever asks first.
async fn push_loop(peer: Peer<RoleServer>, server: Server) {
    loop {
        let Some(mission) = server.bound() else {
            tokio::time::sleep(PUSH_IDLE).await;
            continue;
        };
        let session = server.session.to_string();
        let res = tokio::task::spawn_blocking(move || {
            Api::connect()?.wait_through_restarts(&mission, &session, None, PUSH_WAIT_SECS)
        })
        .await;
        let Ok(Ok(waited)) = res else {
            tokio::time::sleep(PUSH_RETRY).await;
            continue;
        };
        for ev in &waited.events {
            if peer.send_notification(channel_note(ev)).await.is_err() {
                return; // client went away
            }
        }
    }
}

/// One channel message per page event. `meta` values must be strings with identifier keys.
fn channel_note(ev: &Event) -> ServerNotification {
    let d = ev.data.clone().unwrap_or_default();
    let s = |k: &str| d.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let content = match ev.kind.as_str() {
        "answer" => {
            let q = s("question");
            let on = if q.is_empty() { String::new() } else { format!(" (\"{q}\")") };
            format!("The user answered need {}{on}: {}", s("need"), s("answer"))
        }
        "customize" => format!("The user asks to change the mission page: {}. Answer with config_patch.", ev.text),
        _ => format!("The user clicked \"{}\" on the mission page: {}", s("label"), ev.text),
    };
    let params = json!({
        "content": content,
        "meta": {"mission_id": ev.mission, "kind": ev.kind, "seq": ev.seq.to_string()},
    });
    ServerNotification::CustomNotification(CustomNotification::new(CHANNEL_METHOD, Some(params)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_default_to_the_bound_mission() {
        let s = Server::new("ses_1", None, false);
        assert!(s.target(None).unwrap_err().contains("mission_new"));
        assert_eq!(s.target(Some("msn_x".into())).unwrap(), "msn_x");
        s.bind("msn_a");
        assert_eq!(s.target(None).unwrap(), "msn_a");
        assert_eq!(s.clone().target(None).unwrap(), "msn_a", "clones share the binding");
    }

    #[test]
    fn channel_notes_are_string_valued_and_say_what_happened() {
        let ev = Event {
            seq: 7,
            mission: "msn_1".into(),
            kind: "answer".into(),
            text: "N-1: later".into(),
            data: Some(json!({"need": "N-1", "question": "Ship?", "answer": "later"})),
            at: 0,
        };
        let ServerNotification::CustomNotification(n) = channel_note(&ev) else { panic!("not custom") };
        assert_eq!(n.method, CHANNEL_METHOD);
        let p = n.params.unwrap();
        assert_eq!(p["content"], "The user answered need N-1 (\"Ship?\"): later");
        let meta = p["meta"].as_object().unwrap();
        assert!(meta.values().all(serde_json::Value::is_string), "{meta:?}");
    }

    #[test]
    fn channel_mode_skips_the_newest_protocol() {
        let r = Server::new("s", None, true);
        assert!(!r.supported_protocol_versions().contains(&ProtocolVersion::V_2026_07_28));
        assert!(Server::new("s", None, false).supported_protocol_versions().contains(&ProtocolVersion::V_2026_07_28));
    }

    #[test]
    fn every_tool_is_listed() {
        let names: Vec<String> = Server::tool_router().list_all().into_iter().map(|t| t.name.to_string()).collect();
        for want in ["mission_new", "mission_attach", "mission_list", "batch", "view", "config_get", "config_explain"] {
            assert!(names.iter().any(|n| n == want), "{want} missing from {names:?}");
        }
    }
}
