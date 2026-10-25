//! Types shared by the daemon, the agent-side client and the MCP server (the wire format).

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mission {
    pub id: String,
    pub title: String,
    pub session: Option<String>,
    pub project_dir: Option<String>,
    pub created: i64,
    pub archived: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NewMission {
    pub title: String,
    #[serde(default)]
    pub session: Option<String>,
    /// Repo root whose `.mission-control/` is the project layer.
    #[serde(default)]
    pub project_dir: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    /// Create even if this session already has a live mission with the same title.
    #[serde(default)]
    pub force: bool,
}

/// `POST /api/missions/{id}/attach`: bind a mission to a (new) agent session.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Attach {
    #[serde(default)]
    pub session: Option<String>,
}

/// What `POST /api/missions` returns: the agent keeps `id`; the user opens `url`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Created {
    pub mission: Mission,
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub collection: String,
    pub key: String,
    pub state: Option<String>,
    pub body: Value,
    pub updated: i64,
}

/// Which record: a collection and a key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordKey {
    pub collection: String,
    pub key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub seq: i64,
    pub mission: String,
    pub kind: String,
    pub text: String,
    pub data: Option<Value>,
    pub at: i64,
}

/// One typed write. A batch of them applies in one transaction.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Creates or updates a record: `patch` is a JSON merge patch (RFC 7386) on its body.
    Upsert {
        collection: String,
        key: String,
        /// Record state (drives pills and counts); kept when omitted.
        #[serde(default)]
        state: Option<String>,
        /// Fields to merge into the body; `null` deletes a field.
        #[serde(default)]
        patch: Value,
    },
    /// Deletes a record.
    Remove { collection: String, key: String },
    /// Appends to the activity log (the feed).
    Log {
        #[serde(default = "default_log_kind")]
        kind: String,
        text: String,
        #[serde(default)]
        data: Option<Value>,
        /// When it happened (Unix ms); default now. For imports and backfills.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<i64>,
    },
}

fn default_log_kind() -> String {
    "log".into()
}

/// What the page sends (`POST /api/missions/{id}/events`, mission token).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PageEvent {
    /// Answers a need: sets it `answered` with the answer in its body.
    Answer {
        need: String,
        /// Default `needs`.
        #[serde(default)]
        collection: Option<String>,
        answer: String,
    },
    /// A button: a configured action by id, optionally on a row, with optional typed text.
    Action {
        action: String,
        #[serde(default)]
        collection: Option<String>,
        #[serde(default)]
        key: Option<String>,
        #[serde(default)]
        text: Option<String>,
    },
}

/// Everything the page needs to render a mission.
#[derive(Debug, Serialize)]
pub struct View {
    pub mission: Mission,
    pub template: String,
    pub config: Value,
    pub issues: Vec<crate::config::Issue>,
    pub records: Vec<Record>,
    /// The newest events, oldest first.
    pub events: Vec<Event>,
    /// All events of the mission (`events` holds only the newest page).
    pub event_count: i64,
}

/// `GET /health`: unauthenticated, enough for a client to decide whether to restart the daemon.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Health {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub pid: u32,
    /// Unix ms when the daemon started; clients compare it with their binary's mtime (`crate::lifecycle`).
    #[serde(default)]
    pub started: i64,
}

/// `daemon.json` in the state dir: how clients find the daemon.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DaemonState {
    pub port: u16,
    pub pid: u32,
    #[serde(default)]
    pub version: String,
    /// Unix ms when the daemon started. Older `daemon.json` files lack it.
    #[serde(default)]
    pub started: i64,
}

/// What's known about a newer release (`mission-control update check`, the page's notice, agents).
#[allow(clippy::struct_excessive_bools)] // flags the notice logic reads directly
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateInfo {
    /// This binary's version.
    pub current: String,
    /// Newest release seen by the last check, if any.
    pub latest: Option<String>,
    /// `latest` is newer than `current`.
    pub available: bool,
    /// Show a notice: available, checks on, and `latest` not skipped.
    pub notify: bool,
    /// `[update] check` in the user's `config.toml`.
    pub check: bool,
    /// `latest` is in `[update] skip`.
    pub skipped: bool,
    /// Unix ms of the last check, if any.
    pub checked: Option<i64>,
    /// How to update.
    pub command: String,
    /// Release notes for `latest`.
    pub notes: Option<String>,
}
