//! The config's shape, defined once: `mission-control schema` prints it as JSON Schema (schemars), and validation
//! deserializes the resolved config into it (serde), so the two can't drift. Widget props are open
//! (`props`), since script widgets bring their own.

use super::validate::{Issue, Severity};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A resolved mission-control config. Every layer is a partial one of these.
#[derive(Deserialize, JsonSchema)]
pub struct Config {
    /// Config format version (1).
    pub schema: Option<u32>,
    pub mission: Option<MissionCfg>,
    /// Free values for templates: `{vars.repo}`.
    pub vars: Option<Map<String, Value>>,
    /// Named link templates: `{value}`, `{row.<field>}`, `{vars.<name>}`.
    pub links: Option<BTreeMap<String, String>>,
    /// State → color tone.
    pub states: Option<BTreeMap<String, Tone>>,
    /// Token overrides (`accent = "#2f6fdb"`), and `dark.*` for dark mode.
    pub theme: Option<Map<String, Value>>,
    /// Optional per-collection hints: key field, field types, default sort.
    pub collections: Option<BTreeMap<String, Map<String, Value>>>,
    /// Pages, shown as tabs after Overview (which always exists). Merged by `id`.
    pub pages: Option<Vec<Page>>,
    /// The sections, top to bottom; each names its page (`page`, default `overview`). Merged by `id`.
    pub sections: Option<Vec<Section>>,
    /// Named widgets: a built-in with preset props, a script (user/project only) or sandboxed HTML.
    pub widgets: Option<BTreeMap<String, WidgetDef>>,
    /// Buttons, by id.
    pub actions: Option<BTreeMap<String, Action>>,
    pub toolbar: Option<Toolbar>,
    /// The status strip under the top bar: health, cells, needs.
    pub strip: Option<Strip>,
    /// Page behaviour: keyboard shortcuts, notifications.
    pub ui: Option<Ui>,
    pub permissions: Option<Permissions>,
    pub retention: Option<Retention>,
    pub brief: Option<Brief>,
    pub ingest: Option<Ingest>,
    /// Release checks (read from the user's `config.toml` only; `mission-control update off|on|skip` edits it).
    pub update: Option<UpdateCfg>,
    /// Anything else: kept, but warned about (a typo, usually).
    #[serde(flatten)]
    #[schemars(skip)]
    pub unknown: Map<String, Value>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MissionCfg {
    pub title: Option<String>,
    /// Template name (`templates/<name>.toml` in builtin, user or project).
    pub template: Option<String>,
    /// `live` (server-sent events) or `poll:<seconds>`.
    #[schemars(regex(pattern = r"^(live|poll:[0-9]+)$"))]
    pub refresh: Option<String>,
    /// Load redline's overlay on the page (when a redline daemon answers).
    pub review: Option<bool>,
}

#[derive(Deserialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Todo,
    Prog,
    Rev,
    Block,
    Done,
    Warn,
}

/// A page: a tab with its own URL (`/m/<id>/<page>`).
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub id: String,
    /// The tab's text (default the id).
    pub title: Option<String>,
    /// A count shown on the tab, e.g. `{ collection = "prs", state = "open" }`.
    pub count: Option<super::widgets::CountSpec>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SectionColumn {
    Main,
    Rail,
}

#[derive(Deserialize, JsonSchema)]
pub struct Section {
    pub id: String,
    /// A built-in widget or a name from `[widgets]`.
    pub widget: String,
    pub title: Option<String>,
    /// The page it shows on: a `[[pages]]` id (default `overview`).
    pub page: Option<String>,
    /// Which column on a two-column page (default `main`).
    pub column: Option<SectionColumn>,
    /// A link in the section header to another page ("Open plan", "All").
    pub go: Option<GoLink>,
    pub collection: Option<String>,
    /// Section buttons (action ids).
    pub actions: Option<Vec<String>>,
    /// Per-row buttons (action ids); the row goes with the click.
    pub row_actions: Option<Vec<String>>,
    /// Table columns, merged by `id`.
    pub columns: Option<Vec<Column>>,
    /// The widget's own props (`items`, `limit`, `record`, `sort`, …).
    #[serde(flatten)]
    pub props: Map<String, Value>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GoLink {
    /// A page id.
    pub page: String,
    /// Link text (default the page's title).
    pub label: Option<String>,
}

/// The status strip (`.design/components.md`): health first, then the cells, then *Needs you*.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Strip {
    /// The record whose state and `reason` are the run's health (default `run`/`run`); else `vars.health`.
    pub health: Option<super::widgets::RecordRef>,
    /// The collection of questions for the user (default `needs`).
    pub needs: Option<String>,
    /// Cells between health and needs, in order. Merged by `id`.
    pub cells: Option<Vec<StripCell>>,
}

/// One strip cell: a label over a number, or the phase of a `plan` section.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StripCell {
    pub id: String,
    pub label: String,
    pub count: Option<super::widgets::CountSpec>,
    pub sum: Option<super::widgets::SumSpec>,
    pub value: Option<Value>,
    /// The total it's out of ("3/8").
    pub of: Option<super::widgets::CountSpec>,
    pub format: Option<super::widgets::Format>,
    /// Text after the number ("done", "running").
    pub suffix: Option<String>,
    /// Color the number with a state tone (only for a count of that state, like blocked).
    pub tone: Option<Tone>,
    /// Hide the cell while the number is 0.
    pub hide_zero: Option<bool>,
    /// A `plan` section id: the cell shows its current phase ("2 of 4 · Build") and a phase bar.
    pub plan: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Column {
    /// The record field (`key`, `state`, `updated`, or a body field).
    pub id: String,
    pub label: Option<String>,
    /// A `[links]` name.
    pub link: Option<String>,
    #[serde(rename = "as")]
    pub render: Option<ColumnAs>,
    /// A field to show instead when this one is empty (`id = "reason", or = "note"`).
    pub or: Option<String>,
    #[serde(flatten)]
    pub props: Map<String, Value>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColumnAs {
    Pill,
    Code,
    Muted,
    Time,
    /// How long ago, short and mono ("38m", "2h", "3d").
    Age,
    Link,
    Markdown,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WidgetDef {
    /// A built-in widget to preset.
    pub base: Option<String>,
    pub props: Option<Map<String, Value>>,
    /// `widgets/<name>.js` in the user or project config dir (not in the mission layer).
    pub script: Option<String>,
    /// HTML shown in a sandboxed iframe with no token.
    pub html: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub kind: ActionKind,
    pub label: Option<String>,
    /// Ask before acting.
    pub confirm: Option<bool>,
    /// `ask_agent`: message template sent to the agent.
    pub message: Option<String>,
    /// `ask_agent`: ask the user for text first.
    pub prompt: Option<bool>,
    pub placeholder: Option<String>,
    /// `set`: the row's new state …
    pub state: Option<String>,
    /// … or a body field and its value.
    pub field: Option<String>,
    pub value: Option<Value>,
    /// `open`: a link template.
    pub url: Option<String>,
    /// `copy`: a text template.
    pub text: Option<String>,
    /// `run`: argv (no shell). User/project config only.
    pub command: Option<Vec<String>>,
    pub cwd: Option<String>,
    pub env: Option<BTreeMap<String, String>>,
    pub timeout_s: Option<u64>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    AskAgent,
    Customize,
    Answer,
    Set,
    Open,
    Copy,
    Run,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ui {
    /// Single-key shortcuts (`/`, `?`, `g` then a letter, `n`, `j`/`k`); ⌘K / Ctrl+K always works. Default true.
    pub shortcuts: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Toolbar {
    pub actions: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    /// Allow `run` buttons (user/project config only).
    pub run_actions: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Retention {
    /// Newest events kept per mission (default 2000).
    pub events_keep: Option<u64>,
    /// Events the feed loads per "older" page (default 50).
    pub page_size: Option<u64>,
    /// Mission config versions kept; older ones fold into one (default 50).
    pub config_keep: Option<u64>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Brief {
    pub max_chars: Option<u64>,
    pub collections: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateCfg {
    /// Check for a new release daily and show a notice (default true).
    pub check: Option<bool>,
    /// Releases not to be notified about.
    pub skip: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ingest {
    pub subagents: Option<bool>,
    pub prs: Option<bool>,
    pub spend: Option<bool>,
    /// USD per million tokens by model-id prefix (longest match wins), for spend estimates.
    pub prices: Option<BTreeMap<String, Price>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    /// Default 0.1 × input.
    pub cache_read: Option<f64>,
    /// Default 1.25 × input.
    pub cache_write: Option<f64>,
}

/// The JSON Schema of [`Config`], with each built-in widget's props under `x-widget-props`.
pub fn json_schema() -> Value {
    let mut s = serde_json::to_value(schemars::schema_for!(Config)).unwrap_or(Value::Null);
    if let Some(o) = s.as_object_mut() {
        o.insert("x-widget-props".into(), super::widgets::schemas());
    }
    s
}

/// Type errors (the first one serde meets) and unknown top-level keys of a resolved config.
pub fn check(config: &Value) -> Vec<Issue> {
    let mut out = Vec::new();
    match serde_path_to_error::deserialize::<_, Config>(config) {
        Ok(c) => {
            out.extend(super::widgets::check_sections(config));
            for k in c.unknown.keys() {
                out.push(Issue {
                    severity: Severity::Warning,
                    path: k.clone(),
                    message: format!("unknown key `{k}` (see `mission-control schema`)"),
                });
            }
        }
        Err(e) => {
            let path = by_id(config, &e.path().to_string());
            out.push(Issue { severity: Severity::Error, path, message: e.inner().to_string() });
        }
    }
    out
}

/// `sections[2].columns[0].as` → `sections[prs].columns[state].as`, the provenance spelling.
fn by_id(config: &Value, path: &str) -> String {
    let mut out = String::new();
    let mut cur = Some(config);
    for part in path.split('.') {
        let (key, rest) = part.split_once('[').map_or((part, ""), |(k, r)| (k, r));
        cur = cur.and_then(|c| c.get(key));
        if !out.is_empty() {
            out.push('.');
        }
        out.push_str(key);
        for idx in rest.split('[').filter(|s| !s.is_empty()) {
            let idx = idx.trim_end_matches(']');
            let item = idx.parse::<usize>().ok().and_then(|i| cur.and_then(|c| c.get(i)));
            let id = item.and_then(|i| i.get("id")).and_then(Value::as_str);
            out.push('[');
            out.push_str(id.unwrap_or(idx));
            out.push(']');
            cur = item;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn type_errors_name_the_item_by_id() {
        let c = json!({"sections": [{"id": "a", "widget": "feed"}, {"id": "prs", "widget": "table", "columns": [{"id": "state", "as": "glow"}]}]});
        let issues = check(&c);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].path, "sections[prs].columns[state].as");
        assert!(issues[0].message.contains("glow"));
    }

    #[test]
    fn unknown_top_level_keys_warn_and_bad_kinds_error() {
        let issues = check(&json!({"sectons": []}));
        assert_eq!((issues[0].severity, issues[0].path.as_str()), (Severity::Warning, "sectons"));
        let issues = check(&json!({"actions": {"x": {"kind": "explode"}}}));
        assert_eq!((issues[0].severity, issues[0].path.as_str()), (Severity::Error, "actions.x.kind"));
    }

    #[test]
    fn the_schema_names_every_top_level_key() {
        let s = json_schema();
        let props = s["properties"].as_object().unwrap();
        for k in ["mission", "sections", "actions", "widgets", "states", "permissions"] {
            assert!(props.contains_key(k), "{k}");
        }
    }
}
