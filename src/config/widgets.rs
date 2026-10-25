//! Props of each built-in widget, typed once: validation checks every section against its widget's props
//! (named widgets against their `base`, with their preset props), and `mission-control schema` publishes them under
//! `x-widget-props` so agents don't guess.

use super::schema::Column;
use super::validate::{Issue, Severity};
use schemars::JsonSchema;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

/// A string, or a list of them (`state = "open"` or `state = ["open", "pending"]`).
#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

/// One record, by collection and key; `field` picks a body field.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordRef {
    pub collection: String,
    pub key: String,
    pub field: Option<String>,
}

/// What a KPI tile counts: records of a collection (optionally in `state`, or with `field` == `value`), or
/// all events.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CountSpec {
    pub collection: Option<String>,
    pub state: Option<OneOrMany>,
    pub field: Option<String>,
    pub value: Option<Value>,
    pub events: Option<bool>,
}

/// A sum of one or more number fields over a collection's records (optionally only some states).
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SumSpec {
    pub collection: String,
    /// One field, or several added together (`["input_tokens", "output_tokens"]`).
    pub field: OneOrMany,
    pub state: Option<OneOrMany>,
}

#[derive(Deserialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    /// `1234` → "1,234".
    Number,
    /// `14.2` → "$14.20".
    Usd,
    /// `3100000` → "3.1M".
    Tokens,
}

/// A number shown somewhere small: a count, a sum or a fixed value, with an optional denominator.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    pub count: Option<CountSpec>,
    pub sum: Option<SumSpec>,
    pub value: Option<Value>,
    /// The total it's out of ("3 / 8").
    pub of: Option<CountSpec>,
    pub format: Option<Format>,
    /// Text before or after the number ("of", "merged", "tokens").
    pub prefix: Option<String>,
    pub suffix: Option<String>,
}

/// A KPI tile's sub-line: fixed text, or a metric ("3 merged", "of 5 spawned").
#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Note {
    Text(String),
    Metric(Box<Metric>),
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KpiItem {
    pub label: String,
    /// A count from records or events …
    pub count: Option<CountSpec>,
    /// … or a sum of fields …
    pub sum: Option<SumSpec>,
    /// … or a fixed value.
    pub value: Option<Value>,
    /// The total the count is out of: shows "3 / 8" and a bar.
    pub of: Option<CountSpec>,
    pub format: Option<Format>,
    /// A muted line under the value.
    pub note: Option<Note>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Kpis {
    pub items: Option<Vec<KpiItem>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Health {
    /// The record whose state and `reason` show; default `vars.health` / `vars.health_reason`.
    pub record: Option<RecordRef>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Needs {
    /// Default `needs`.
    pub collection: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Table {
    pub collection: Option<String>,
    pub columns: Option<Vec<Column>>,
    /// A field; `-field` for descending. Default `key` (numbers inside text compare as numbers).
    pub sort: Option<String>,
    /// Rows to show: `{ field = value }` or `{ field = [values] }`; `state` and `key` work too.
    pub filter: Option<Map<String, Value>>,
    /// Text when there are no rows.
    pub empty: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Feed {
    /// Only these event kinds.
    pub kinds: Option<OneOrMany>,
    /// Newest events shown (default 50).
    pub limit: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Markdown {
    /// Fixed text …
    pub text: Option<String>,
    /// … or a record's field (default field `text`).
    pub record: Option<RecordRef>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Kanban {
    pub collection: Option<String>,
    /// Lane states in order; default the states present, ordered by tone.
    pub lanes: Option<Vec<String>>,
    /// Body fields shown under each card's title.
    pub fields: Option<Vec<String>>,
    /// The field used as card title (default `title`, then `name`, then the key).
    pub card_title: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Checklist {
    /// Gates: records with requirements (strings or `{text, done}`) and an optional note.
    pub collection: Option<String>,
    pub card_title: Option<String>,
    /// The requirements field (default `requirements`, else `items`, else `requires`).
    pub requirements: Option<String>,
    /// The note field (default `note`, else `answer`), shown as markdown.
    pub note: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlowLanes {
    pub collection: String,
    /// Lane title field (default `title`, else the key).
    pub label: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlowNodes {
    pub collection: String,
    /// The field naming a node's lane (default `lane`).
    pub lane: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlowGates {
    pub collection: String,
    /// The field naming the lane a gate follows (default `after`).
    pub after: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Flow {
    pub lanes: Option<FlowLanes>,
    pub nodes: Option<FlowNodes>,
    pub gates: Option<FlowGates>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanPhases {
    /// Phase records, one per column in key order; several names: the first with records. Without any,
    /// the columns are the distinct values of the nodes' phase field.
    pub collection: OneOrMany,
    /// Phase title field (default `title`, else `name`, else the key).
    pub label: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanNodes {
    pub collection: String,
    /// The field naming a node's phase; a list: the first present (default `["phase", "wave"]`).
    pub phase: Option<OneOrMany>,
    /// The field listing the keys a node depends on (default `depends_on`).
    pub deps: Option<String>,
    /// The node title field(s) (default `["title", "name"]`).
    pub title: Option<OneOrMany>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanGates {
    pub collection: String,
    /// The field naming the phase a gate follows (default `after`).
    pub after: Option<String>,
    /// The requirements field (default `requirements`, else `items`, else `requires`).
    pub requirements: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Plan {
    pub phases: Option<PlanPhases>,
    pub nodes: Option<PlanNodes>,
    pub gates: Option<PlanGates>,
    /// Draw dependency edges (default true; `flow` sections draw none).
    pub edges: Option<bool>,
    /// Columns of the Table view, as in `table`.
    pub columns: Option<Vec<Column>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphNodes {
    pub collection: String,
    /// The field naming a node's group (its column; default `group`).
    pub group: Option<String>,
    /// Title field(s) (default `title`, else `name`).
    pub title: Option<OneOrMany>,
    /// The field naming a task record whose state the node shows (default `task`).
    pub task: Option<String>,
    /// A muted line when there's no task (default `note`).
    pub note: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphEdges {
    pub collection: String,
    /// Fields holding the two node keys and the label (defaults `from`, `to`, `label`).
    pub from: Option<String>,
    pub to: Option<String>,
    pub label: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Graph {
    pub nodes: Option<GraphNodes>,
    pub edges: Option<GraphEdges>,
    /// Group order, left to right (default: as they first appear, nodes by key).
    pub groups: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Doc {
    /// Fixed markdown …
    pub text: Option<String>,
    /// … or a record's field (default field `body`).
    pub record: Option<RecordRef>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Track {
    /// The `plan` section whose phases, nodes and gates the track shows.
    pub plan: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    Line,
    Bar,
    Sparkline,
}

#[derive(Deserialize, JsonSchema)]
pub struct Chart {
    pub collection: Option<String>,
    /// Default `line`.
    pub kind: Option<ChartKind>,
    /// The x field (default `key`); numbers and Unix-ms times plot to scale.
    pub x: Option<String>,
    /// The y field, or several fields (one series each). Default `value`.
    pub y: Option<OneOrMany>,
    /// A field that splits rows into series (instead of several `y` fields).
    pub series: Option<String>,
    /// The series name when there is one series.
    pub label: Option<String>,
    pub x_label: Option<String>,
    /// Pixels (default 200).
    pub height: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Spend {
    /// Default `spend`: records with `usd` (and `input_tokens`, `output_tokens`).
    pub collection: Option<String>,
    /// The amount field (default `usd`).
    pub field: Option<String>,
    /// The row label field (default the key).
    pub label: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Html {
    /// HTML shown in a sandboxed iframe …
    pub html: Option<String>,
    /// … or a record's field (default field `html`).
    pub record: Option<RecordRef>,
    /// Pixels (default 240).
    pub height: Option<u32>,
}

/// Checks `props` against `widget`'s props; `None` for a widget this table doesn't know.
fn check_as<T: DeserializeOwned>(props: &Value) -> Result<(), String> {
    serde_path_to_error::deserialize::<_, T>(props).map(|_| ()).map_err(|e| {
        let p = e.path().to_string();
        if p == "." { e.inner().to_string() } else { format!("{p}: {}", e.inner()) }
    })
}

fn check(widget: &str, props: &Value) -> Option<Result<(), String>> {
    Some(match widget {
        "kpis" => check_as::<Kpis>(props),
        "health" => check_as::<Health>(props),
        "needs" => check_as::<Needs>(props),
        "table" => check_as::<Table>(props),
        "feed" => check_as::<Feed>(props),
        "markdown" => check_as::<Markdown>(props),
        "kanban" => check_as::<Kanban>(props),
        "checklist" => check_as::<Checklist>(props),
        "flow" => check_as::<Flow>(props),
        "plan" => check_as::<Plan>(props),
        "track" => check_as::<Track>(props),
        "graph" => check_as::<Graph>(props),
        "doc" => check_as::<Doc>(props),
        "chart" => check_as::<Chart>(props),
        "spend" => check_as::<Spend>(props),
        "html" => check_as::<Html>(props),
        _ => return None,
    })
}

/// Every section's props against its widget (a named widget's `base`, with its preset props underneath).
pub fn check_sections(config: &Value) -> Vec<Issue> {
    let widgets = config.get("widgets").and_then(Value::as_object);
    let mut out = Vec::new();
    for s in config.get("sections").and_then(Value::as_array).into_iter().flatten() {
        let Some(name) = s.get("widget").and_then(Value::as_str) else { continue };
        let named = widgets.and_then(|w| w.get(name));
        let base = named.and_then(|n| n.get("base")).and_then(Value::as_str).unwrap_or(name);
        let mut props = named.and_then(|n| n.get("props")).cloned().unwrap_or(Value::Object(Map::new()));
        if let (Some(p), Some(own)) = (props.as_object_mut(), s.as_object()) {
            p.extend(own.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        if let Some(Err(msg)) = check(base, &props) {
            let id = s.get("id").and_then(Value::as_str).unwrap_or("?");
            out.push(Issue {
                severity: Severity::Error,
                path: format!("sections[{id}]"),
                message: format!("`{base}` props: {msg}"),
            });
        }
    }
    out
}

/// The props schema of every built-in widget, by name.
pub fn schemas() -> Value {
    let one = |s: schemars::Schema| serde_json::to_value(s).unwrap_or(Value::Null);
    serde_json::json!({
        "kpis": one(schemars::schema_for!(Kpis)), "health": one(schemars::schema_for!(Health)),
        "needs": one(schemars::schema_for!(Needs)), "table": one(schemars::schema_for!(Table)),
        "feed": one(schemars::schema_for!(Feed)), "markdown": one(schemars::schema_for!(Markdown)),
        "kanban": one(schemars::schema_for!(Kanban)), "checklist": one(schemars::schema_for!(Checklist)),
        "flow": one(schemars::schema_for!(Flow)), "chart": one(schemars::schema_for!(Chart)),
        "plan": one(schemars::schema_for!(Plan)), "track": one(schemars::schema_for!(Track)),
        "graph": one(schemars::schema_for!(Graph)), "doc": one(schemars::schema_for!(Doc)),
        "spend": one(schemars::schema_for!(Spend)), "html": one(schemars::schema_for!(Html)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn props_are_checked_through_named_widgets() {
        let c = json!({
            "widgets": {"burn": {"base": "chart", "props": {"kind": "pie"}}},
            "sections": [
                {"id": "a", "widget": "burn", "y": ["open"]},
                {"id": "b", "widget": "feed", "limit": "many"},
                {"id": "c", "widget": "kpis", "items": [{"label": "x", "count": {"collection": "c", "state": ["a", "b"]}}]},
                {"id": "d", "widget": "mine", "anything": true},
            ],
        });
        let issues = check_sections(&c);
        let paths: Vec<_> = issues.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(paths, ["sections[a]", "sections[b]"], "{issues:?}");
        assert!(issues[0].message.contains("pie") && issues[1].message.contains("limit"), "{issues:?}");
    }

    #[test]
    fn every_builtin_has_a_props_schema() {
        let s = schemas();
        for w in super::super::validate::BUILTIN_WIDGETS {
            assert!(s.get(w).is_some_and(Value::is_object), "{w}");
        }
    }
}
