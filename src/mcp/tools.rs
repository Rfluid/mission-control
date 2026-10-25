//! MCP tool definitions. Every tool has a CLI twin (`src/cli/`); both go through [`Api`].

use super::Server;
use crate::api::{self, Api};
use crate::model::{NewMission, Op};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct NewArgs {
    /// What the run is, e.g. "Billing export build".
    title: String,
    /// Template name (see `template_list`); default from config, usually `minimal`.
    template: Option<String>,
    /// Repo root whose `.mission-control/` is the project layer; default: found from the working directory.
    project_dir: Option<String>,
    /// Create even if this session already has a mission with this title.
    #[serde(default)]
    force: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AttachArgs {
    /// The mission id (`msn_…`) from before the restart.
    id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct BatchArgs {
    /// Ops applied in order, in one transaction: all or nothing.
    ops: Vec<Op>,
    /// Mission id; default: the one bound to this session.
    mission: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ViewArgs {
    /// Only records of these collections; default all.
    #[serde(default)]
    collections: Vec<String>,
    /// Newest events to include (default 20).
    events: Option<i64>,
    /// Include the resolved config (sections, actions…); default false: ask for it when you change the page.
    #[serde(default)]
    config: bool,
    /// Only this page (a `[[pages]]` id, e.g. `plan`): its sections, and records of the collections they read.
    page: Option<String>,
    mission: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct GetArgs {
    /// Key path, e.g. `sections[prs].columns` or `actions`; default the whole resolved config.
    path: Option<String>,
    mission: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ExplainArgs {
    /// Only keys starting with this path, e.g. `sections[prs]`.
    prefix: Option<String>,
    mission: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct PatchArgs {
    /// A config table (JSON object) or TOML text. Arrays of tables merge by `id`: patch one item by id,
    /// `remove = true` drops it, `before`/`after` move it; `"!unset"` removes a key.
    patch: Value,
    /// Why, shown in the history and the feed.
    note: Option<String>,
    /// `mission-control` (default: this page only), `user` (every mission: "always", "by default") or `project` (this
    /// repo's missions, shared with the team: "for this repo").
    layer: Option<String>,
    /// Validate and preview only: returns the resolved config, issues and a `preview_url` the user can open.
    #[serde(default)]
    dry_run: bool,
    mission: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SaveArgs {
    /// File name: `templates/<name>.toml` (letters, digits, `-`, `_`).
    name: String,
    /// `user` (default: the user's own templates) or `project` (shared with the repo).
    layer: Option<String>,
    description: Option<String>,
    /// Overwrite an existing template of that name.
    #[serde(default)]
    replace: bool,
    mission: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct WidgetArgs {
    /// Widget name; sections use it as `widget = "<name>"`.
    name: String,
    /// A built-in widget to preset (`table`, `kpis`, `chart`, …).
    base: Option<String>,
    /// The built-in's props.
    props: Option<Value>,
    /// Or: HTML, shown in a sandboxed iframe with no access to the page or its token.
    html: Option<String>,
    description: Option<String>,
    /// `mission-control` (default), `user` or `project`.
    layer: Option<String>,
    mission: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct BriefArgs {
    /// Character budget; default the mission's `[brief] max_chars`.
    max_chars: Option<usize>,
    mission: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LayerArgs {
    /// `mission-control` (default), `user` or `project`.
    layer: Option<String>,
    mission: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct WaitArgs {
    /// Seconds to block (default 120, max 540).
    timeout_s: Option<u64>,
    /// Event seq to start after; default: where this session's last wait stopped.
    since: Option<i64>,
    mission: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TemplatesArgs {
    /// Repo root whose `.mission-control/templates/` to include; default: found from the working directory.
    project_dir: Option<String>,
}

const DEFAULT_WAIT_SECS: u64 = 120;
/// Events an MCP `view` returns unless asked: agents rarely need the whole feed.
const MCP_VIEW_EVENTS: i64 = 20;
/// Below common MCP client tool timeouts.
const MAX_WAIT_SECS: u64 = 540;

/// Tool output: pretty JSON on success, an `isError` text result on failure.
type ToolResult = Result<String, String>;

/// Runs a blocking call off the async runtime and renders it as pretty JSON.
async fn blocking(f: impl FnOnce() -> anyhow::Result<Value> + Send + 'static) -> ToolResult {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
        .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
        .map_err(|e| format!("{e:#}"))
}

#[tool_router(vis = "pub(crate)")]
impl Server {
    #[tool(
        description = "Start a mission control page for this run and bind it to this session. Give the user the returned url. After a session restart, use mission_attach instead."
    )]
    async fn mission_new(&self, Parameters(a): Parameters<NewArgs>) -> ToolResult {
        let session = self.session.to_string();
        let me = self.clone();
        blocking(move || {
            let project_dir =
                Some(api::project_dir(a.project_dir.map(PathBuf::from).as_deref())?.display().to_string());
            let req = NewMission {
                title: a.title,
                template: a.template,
                project_dir,
                session: Some(session),
                force: a.force,
            };
            let c = Api::connect()?.create(&req)?;
            me.bind(&c.mission.id);
            let mut v = json!({
                "mission_id": c.mission.id, "url": c.url,
                "next": "Give the user the url. Write state with batch; ask the user with an upsert into `needs`.",
            });
            if let (Some(Value::Object(u)), Some(o)) = (crate::update::agent_notice(), v.as_object_mut()) {
                o.extend(u);
            }
            Ok(v)
        })
        .await
    }

    #[tool(
        description = "Re-bind an existing mission (by id) to this session, e.g. after a restart. Records and history are kept."
    )]
    async fn mission_attach(&self, Parameters(a): Parameters<AttachArgs>) -> ToolResult {
        let session = self.session.to_string();
        let me = self.clone();
        blocking(move || {
            let c = Api::connect()?.attach(&a.id, Some(&session))?;
            me.bind(&c.mission.id);
            Ok(json!({"mission_id": c.mission.id, "title": c.mission.title, "url": c.url}))
        })
        .await
    }

    #[tool(description = "Missions on this machine (not archived), newest first: find an id to attach to.")]
    async fn mission_list(&self) -> ToolResult {
        blocking(|| Api::connect()?.list()).await
    }

    #[tool(
        description = "Write to the mission in one transaction. Ops: {op:\"upsert\", collection, key, state?, patch} (patch merges into the record body; null deletes a field), {op:\"remove\", collection, key}, {op:\"log\", text, kind?}. Questions for the user: upsert into `needs` with state \"open\" and patch {question, context?, options?, recommendation?, why?}."
    )]
    async fn batch(&self, Parameters(a): Parameters<BatchArgs>) -> ToolResult {
        let m = self.target(a.mission)?;
        blocking(move || Api::connect()?.apply(&m, &a.ops)).await
    }

    #[tool(
        description = "Records and recent events of the mission (and, with config=true, the resolved config: sections, actions). To pick a mission back up, prefer brief; pass `collections` to read less."
    )]
    async fn view(&self, Parameters(a): Parameters<ViewArgs>) -> ToolResult {
        let m = self.target(a.mission)?;
        blocking(move || {
            let events = Some(a.events.unwrap_or(MCP_VIEW_EVENTS));
            Api::connect()?.view_of(&m, &a.collections, events, a.config, a.page.as_deref())
        })
        .await
    }

    #[tool(description = "The mission's resolved config, or the part at a key path like `sections[prs].columns`.")]
    async fn config_get(&self, Parameters(a): Parameters<GetArgs>) -> ToolResult {
        let m = self.target(a.mission)?;
        blocking(move || Api::connect()?.config_at(&m, a.path.as_deref())).await
    }

    #[tool(
        description = "Which layer (builtin, template, user file, project file, mission version) set each config key, plus config issues. Use before changing config, to patch the right thing."
    )]
    async fn config_explain(&self, Parameters(a): Parameters<ExplainArgs>) -> ToolResult {
        let m = self.target(a.mission)?;
        blocking(move || Api::connect()?.explain_under(&m, a.prefix.as_deref())).await
    }

    #[tool(
        description = "Change the page: add, change, move or remove sections, columns, buttons (actions), links, states, theme, named widgets. Pick the layer from what the user said: `mission-control` (\"here\", \"this page\"; default), `user` (\"always\", \"by default\", every mission), `project` (\"for this repo\", the team). Validated (refused on errors) and undoable (config_undo). `dry_run` previews. The mission layer can't define `run` actions, permissions or script widgets."
    )]
    async fn config_patch(&self, Parameters(a): Parameters<PatchArgs>) -> ToolResult {
        let m = self.target(a.mission)?;
        blocking(move || {
            let patch = match a.patch {
                Value::String(t) => crate::config::parse(&t, "patch")?,
                v => v,
            };
            let layer = a.layer.as_deref().unwrap_or("mission");
            Api::connect()?.patch_config(&m, layer, &patch, a.note.as_deref(), a.dry_run)
        })
        .await
    }

    #[tool(
        description = "Block until the user acts on the page (answers a need, clicks a button, asks for a change), or until timeout. Returns page events: kind `answer` (data.need, data.answer), `action` (data.action, data.row, data.message, data.text) or `customize` (text: what to change; answer with config_patch). Call again to continue."
    )]
    async fn wait(&self, Parameters(a): Parameters<WaitArgs>) -> ToolResult {
        let m = self.target(a.mission)?;
        let session = self.session.to_string();
        let timeout = a.timeout_s.unwrap_or(DEFAULT_WAIT_SECS).min(MAX_WAIT_SECS);
        blocking(move || {
            Ok(serde_json::to_value(Api::connect()?.wait_through_restarts(&m, &session, a.since, timeout)?)?)
        })
        .await
    }

    #[tool(
        description = "Save this mission's customizations as a template (built on its current template with `extends`), so new missions can start from it (mission_new template=<name>). `user` layer by default; `project` to share with the repo."
    )]
    async fn template_save(&self, Parameters(a): Parameters<SaveArgs>) -> ToolResult {
        let m = self.target(a.mission)?;
        blocking(move || {
            let layer = a.layer.as_deref().unwrap_or("user");
            Api::connect()?.save_template(&m, &a.name, layer, a.description.as_deref(), a.replace)
        })
        .await
    }

    #[tool(
        description = "Define a named widget: a built-in with preset props (base + props), or your own HTML (shown sandboxed). Then use it in a section: config_patch {sections: [{id, widget: <name>}]}."
    )]
    async fn widget_define(&self, Parameters(a): Parameters<WidgetArgs>) -> ToolResult {
        let m = self.target(a.mission)?;
        if a.base.is_some() == a.html.is_some() {
            return Err("give either `base` (with optional `props`) or `html`".into());
        }
        blocking(move || {
            let mut def = serde_json::Map::new();
            for (k, v) in [
                ("base", a.base.map(Value::String)),
                ("props", a.props),
                ("html", a.html.map(Value::String)),
                ("description", a.description.map(Value::String)),
            ] {
                if let Some(v) = v {
                    def.insert(k.into(), v);
                }
            }
            Api::connect()?.define_widget(&m, &a.name, &Value::Object(def), a.layer.as_deref().unwrap_or("mission"))
        })
        .await
    }

    #[tool(
        description = "A short text digest of the mission within a character budget: open needs, page events you haven't received yet, and the configured collections by state. Use it to pick a mission back up instead of view."
    )]
    async fn brief(&self, Parameters(a): Parameters<BriefArgs>) -> ToolResult {
        let m = self.target(a.mission)?;
        let session = self.session.to_string();
        let r = tokio::task::spawn_blocking(move || Api::connect()?.brief(&m, &session, a.max_chars))
            .await
            .map_err(|e| e.to_string())?;
        r.map(|v| v["text"].as_str().unwrap_or_default().to_string()).map_err(|e| format!("{e:#}"))
    }

    #[tool(description = "Undo the last config change to a layer (`mission-control` default, `user`, `project`).")]
    async fn config_undo(&self, Parameters(a): Parameters<LayerArgs>) -> ToolResult {
        let m = self.target(a.mission)?;
        blocking(move || Api::connect()?.undo_config(&m, a.layer.as_deref().unwrap_or("mission"))).await
    }

    #[tool(
        description = "A layer's config changes, newest first, with notes (`mission-control` default, `user`, `project`)."
    )]
    async fn config_history(&self, Parameters(a): Parameters<LayerArgs>) -> ToolResult {
        let m = self.target(a.mission)?;
        blocking(move || Api::connect()?.config_history(&m, a.layer.as_deref().unwrap_or("mission"))).await
    }

    #[tool(
        description = "The JSON Schema of mission-control config: every key, its type and what it does. Read it before writing an unfamiliar key."
    )]
    async fn config_schema(&self) -> ToolResult {
        blocking(|| Ok(crate::config::schema::json_schema())).await
    }

    #[tool(
        description = "Templates available here (built-in, the user's, the project's), with descriptions and the files that define each."
    )]
    async fn template_list(&self, Parameters(a): Parameters<TemplatesArgs>) -> ToolResult {
        blocking(move || Ok(api::local_stack(a.project_dir.map(PathBuf::from).as_deref())?.summary())).await
    }
}
