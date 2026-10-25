//! Blocking agent-side client of the daemon (CLI, MCP). Starts or replaces the daemon on demand
//! ([`crate::lifecycle`]).

use crate::config::Stack;
use crate::lifecycle;
use crate::model::{Attach, Created, NewMission, Op};
use crate::paths;
use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Timeout for ordinary calls (long-polls set their own).
const CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// Extra time a long-poll gets over its own timeout before the client gives up on the socket.
const WAIT_SLACK: Duration = Duration::from_secs(15);
/// Reconnect attempts while waiting, and the pause before each (a restart takes well under a second).
const RECONNECT_TRIES: u32 = 5;
const RECONNECT_DELAY: Duration = Duration::from_millis(500);

/// What `wait` returns: page events (answers, clicks, customize requests) and the cursor after them.
#[derive(Debug, serde::Deserialize, Serialize)]
pub struct Waited {
    pub events: Vec<crate::model::Event>,
    pub next: i64,
    pub timed_out: bool,
}

pub struct Api {
    pub port: u16,
    token: String,
}

/// Port for a new daemon: `$MISSION_CONTROL_PORT`, else 7466.
pub fn port() -> u16 {
    std::env::var("MISSION_CONTROL_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(paths::DEFAULT_PORT)
}

impl Api {
    pub fn connect() -> Result<Self> {
        Ok(Self { port: lifecycle::ensure()?, token: paths::admin_token()? })
    }

    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn get(&self, path: &str, timeout: Duration) -> Result<Value> {
        decode(ureq::get(&format!("{}{path}", self.base())).timeout(timeout).call())
    }

    pub fn post(&self, path: &str, body: &impl Serialize) -> Result<Value> {
        let req = ureq::post(&format!("{}{path}", self.base())).timeout(CALL_TIMEOUT);
        decode(req.set("Authorization", &format!("Bearer {}", self.token)).send_json(body))
    }

    pub fn create(&self, req: &NewMission) -> Result<Created> {
        Ok(serde_json::from_value(self.post("/api/missions", req)?)?)
    }

    pub fn attach(&self, mission: &str, session: Option<&str>) -> Result<Created> {
        let body = Attach { session: session.map(String::from) };
        Ok(serde_json::from_value(self.post(&format!("/api/missions/{}/attach", enc(mission)), &body)?)?)
    }

    /// Health of every live mission (`GET /api/doctor`).
    pub fn doctor(&self) -> Result<Value> {
        self.get("/api/doctor", CALL_TIMEOUT)
    }

    pub fn list(&self) -> Result<Value> {
        self.get("/api/missions", CALL_TIMEOUT)
    }

    /// Missions with their summaries; archived ones too when `archived`.
    pub fn list_all(&self, archived: bool) -> Result<Value> {
        self.get(&format!("/api/missions?summary=true&archived={archived}"), CALL_TIMEOUT)
    }

    /// The static HTML snapshot of a mission.
    pub fn export(&self, mission: &str) -> Result<String> {
        let url = format!("{}/api/missions/{}/export", self.base(), enc(mission));
        Ok(ureq::get(&url).timeout(CALL_TIMEOUT).call().map_err(call_error)?.into_string()?)
    }

    pub fn archive(&self, mission: &str, archived: bool) -> Result<Value> {
        self.post(&format!("/api/missions/{}/archive", enc(mission)), &json!({"archived": archived}))
    }

    pub fn fork(&self, mission: &str, title: &str, records: bool) -> Result<Created> {
        let v =
            self.post(&format!("/api/missions/{}/fork", enc(mission)), &json!({"title": title, "records": records}))?;
        Ok(serde_json::from_value(v)?)
    }

    pub fn view(&self, mission: &str) -> Result<Value> {
        self.get(&format!("/api/missions/{}", enc(mission)), CALL_TIMEOUT)
    }

    pub fn apply(&self, mission: &str, ops: &[Op]) -> Result<Value> {
        self.post(&format!("/api/missions/{}/ops", enc(mission)), &ops)
    }

    /// `{config, provenance, template, issues}` of a mission.
    pub fn explain(&self, mission: &str) -> Result<Value> {
        self.get(&format!("/api/missions/{}/config", enc(mission)), CALL_TIMEOUT)
    }

    /// The view, with records only from `collections` (all when empty), `events` newest events and, unless
    /// `config`, no resolved config: keeps agent reads small. `page` narrows it to one page's sections and the
    /// records they read.
    pub fn view_of(
        &self,
        mission: &str,
        collections: &[String],
        events: Option<i64>,
        config: bool,
        page: Option<&str>,
    ) -> Result<Value> {
        let mut q = format!("/api/missions/{}?config={config}", enc(mission));
        if let Some(n) = events {
            let _ = write!(q, "&events={n}");
        }
        if let Some(p) = page {
            let _ = write!(q, "&page={}", enc(p));
        }
        let mut v = self.get(&q, CALL_TIMEOUT)?;
        if !collections.is_empty()
            && let Some(rs) = v.get_mut("records").and_then(Value::as_array_mut)
        {
            rs.retain(|r| r["collection"].as_str().is_some_and(|c| collections.iter().any(|x| x == c)));
        }
        Ok(v)
    }

    /// The resolved config, or the part at `path` (`sections[prs].columns`).
    pub fn config_at(&self, mission: &str, path: Option<&str>) -> Result<Value> {
        let r = self.explain(mission)?;
        let p = path.unwrap_or_default();
        crate::config::path::at(&r["config"], p).cloned().with_context(|| format!("no config key `{p}`"))
    }

    /// Provenance of the keys under `prefix` (all when `None`), and every config issue.
    pub fn explain_under(&self, mission: &str, prefix: Option<&str>) -> Result<Value> {
        let r = self.explain(mission)?;
        let p = prefix.unwrap_or_default();
        let prov: serde_json::Map<String, Value> = (r["provenance"].as_object().into_iter().flatten())
            .filter(|(k, _)| k.starts_with(p))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(json!({"template": r["template"], "provenance": prov, "issues": r["issues"]}))
    }

    /// Long-polls for page events on `mission-control`. `since: None` continues `session`'s server-side cursor.
    pub fn wait(&self, mission: &str, session: &str, since: Option<i64>, timeout_s: u64) -> Result<Waited> {
        let mut q = format!("/api/missions/{}/wait?session={}&timeout={timeout_s}", enc(mission), enc(session));
        if let Some(s) = since {
            let _ = write!(q, "&since={s}");
        }
        let req = ureq::get(&format!("{}{q}", self.base())).timeout(Duration::from_secs(timeout_s) + WAIT_SLACK);
        Ok(serde_json::from_value(decode(req.set("Authorization", &format!("Bearer {}", self.token)).call())?)?)
    }

    /// [`Api::wait`] that rides through a daemon restart (a rebuild replaces the daemon mid-wait): it
    /// reconnects, starting a daemon if needed, until the deadline. The cursor means nothing is missed.
    pub fn wait_through_restarts(
        mut self,
        mission: &str,
        session: &str,
        since: Option<i64>,
        timeout_s: u64,
    ) -> Result<Waited> {
        let deadline = Instant::now() + Duration::from_secs(timeout_s);
        let mut tries = 0;
        loop {
            let left = deadline.saturating_duration_since(Instant::now()).as_secs();
            match self.wait(mission, session, since, left) {
                Err(e) if unreachable(&e) && tries < RECONNECT_TRIES && left > 0 => {
                    tries += 1;
                    std::thread::sleep(RECONNECT_DELAY);
                    self = Api::connect()?;
                }
                r => return r,
            }
        }
    }

    /// Patches `layer` (`mission-control`, `user` or `project`); `dry_run` validates and returns a preview instead.
    pub fn patch_config(
        &self,
        mission: &str,
        layer: &str,
        patch: &Value,
        note: Option<&str>,
        dry_run: bool,
    ) -> Result<Value> {
        let body = json!({"patch": patch, "note": note, "layer": layer, "dry_run": dry_run});
        self.post(&format!("/api/missions/{}/config", enc(mission)), &body)
    }

    /// Saves the mission's customizations as template `name` in the `user` or `project` config.
    pub fn save_template(
        &self,
        mission: &str,
        name: &str,
        layer: &str,
        description: Option<&str>,
        replace: bool,
    ) -> Result<Value> {
        let body = json!({"name": name, "layer": layer, "description": description, "replace": replace});
        self.post(&format!("/api/missions/{}/templates", enc(mission)), &body)
    }

    /// Defines (or redefines) a named widget on `layer`: a built-in with preset props, or sandboxed HTML.
    pub fn define_widget(&self, mission: &str, name: &str, def: &Value, layer: &str) -> Result<Value> {
        let patch = json!({"widgets": {name: def}});
        self.patch_config(mission, layer, &patch, Some(&format!("widget {name}")), false)
    }

    /// The budgeted text digest of a mission (`[brief]`), peeking `session`'s unread page events.
    pub fn brief(&self, mission: &str, session: &str, max_chars: Option<usize>) -> Result<Value> {
        let mut q = format!("/api/missions/{}/brief?session={}", enc(mission), enc(session));
        if let Some(n) = max_chars {
            let _ = write!(q, "&max_chars={n}");
        }
        self.get(&q, CALL_TIMEOUT)
    }

    pub fn undo_config(&self, mission: &str, layer: &str) -> Result<Value> {
        self.post(&format!("/api/missions/{}/config/undo", enc(mission)), &json!({"layer": layer}))
    }

    pub fn config_history(&self, mission: &str, layer: &str) -> Result<Value> {
        self.get(&format!("/api/missions/{}/config/history?layer={}", enc(mission), enc(layer)), CALL_TIMEOUT)
    }
}

/// Project root for a new mission or a local config read: `project`, else found from the working directory.
pub fn project_dir(project: Option<&Path>) -> Result<PathBuf> {
    match project {
        Some(p) => Ok(p.to_path_buf()),
        None => Ok(paths::project_root(&std::env::current_dir()?)),
    }
}

/// Config layers on this machine (no daemon): builtin, the user's and `project`'s (default: found from here).
pub fn local_stack(project: Option<&Path>) -> Result<Stack> {
    Stack::load(paths::user_config_dir().as_deref(), Some(&project_dir(project)?))
}

fn decode(r: Result<ureq::Response, ureq::Error>) -> Result<Value> {
    Ok(r.map_err(call_error)?.into_json()?)
}

/// The daemon went away (as opposed to answering with an error).
pub fn unreachable(e: &anyhow::Error) -> bool {
    matches!(e.downcast_ref::<ureq::Error>(), Some(ureq::Error::Transport(_)))
}

/// A failed call as an error the agent can act on: the daemon's message, or "unreachable".
fn call_error(e: ureq::Error) -> anyhow::Error {
    match e {
        ureq::Error::Status(code, resp) => anyhow!("{code}: {}", resp.into_string().unwrap_or_default()),
        e @ ureq::Error::Transport(_) => anyhow!(e).context("mission-control daemon unreachable"),
    }
}

/// Percent-encodes a path segment.
pub fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}
