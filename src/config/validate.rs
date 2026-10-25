//! Checks on a resolved config and its layers. Errors block a write; warnings show on the page and in `doctor`.

use super::{Layer, LayerKind};
use serde::Serialize;
use serde_json::Value;

/// Widgets the page renders (props typed in `widgets.rs`).
pub const BUILTIN_WIDGETS: [&str; 16] = [
    "kpis",
    "health",
    "needs",
    "table",
    "feed",
    "markdown",
    "kanban",
    "flow",
    "plan",
    "track",
    "checklist",
    "chart",
    "spend",
    "html",
    "graph",
    "doc",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Serialize)]
pub struct Issue {
    pub severity: Severity,
    pub path: String,
    pub message: String,
}

fn issue(severity: Severity, path: impl Into<String>, message: impl Into<String>) -> Issue {
    Issue { severity, path: path.into(), message: message.into() }
}

pub fn validate(config: &Value, layers: &[Layer]) -> Vec<Issue> {
    let mut out = super::schema::check(config);
    for l in layers.iter().filter(|l| l.kind == LayerKind::Mission) {
        untrusted(&l.doc, &mut out);
    }
    let actions = config.get("actions").and_then(Value::as_object);
    let widgets = config.get("widgets").and_then(Value::as_object);
    let links = config.get("links").and_then(Value::as_object);

    plan_refs(config, &mut out);
    for (id, a) in actions.into_iter().flatten() {
        let kind = a.get("kind").and_then(Value::as_str).unwrap_or("");
        let run_ok = config.pointer("/permissions/run_actions").and_then(Value::as_bool) == Some(true);
        if kind == "run" && !run_ok {
            out.push(issue(
                Severity::Warning,
                format!("actions.{id}"),
                "`run` action disabled: permissions.run_actions is false",
            ));
        }
    }
    let pages = super::pages::ids(config);
    for s in config.get("sections").and_then(Value::as_array).into_iter().flatten() {
        let id = s.get("id").and_then(Value::as_str).unwrap_or("?");
        if let Some(p) = s.get("page").and_then(Value::as_str)
            && !pages.iter().any(|k| k == p)
        {
            out.push(issue(
                Severity::Warning,
                format!("sections[{id}].page"),
                format!("unknown page `{p}` (have: {}); shown on overview", pages.join(", ")),
            ));
        }
        if let Some(p) = s.pointer("/go/page").and_then(Value::as_str)
            && !pages.iter().any(|k| k == p)
        {
            out.push(issue(Severity::Warning, format!("sections[{id}].go.page"), format!("unknown page `{p}`")));
        }
        let w = s.get("widget").and_then(Value::as_str).unwrap_or("");
        if !BUILTIN_WIDGETS.contains(&w) && !widgets.is_some_and(|m| m.contains_key(w)) {
            out.push(issue(Severity::Warning, format!("sections[{id}].widget"), format!("unknown widget `{w}`")));
        }
        for key in ["actions", "row_actions"] {
            for a in s.get(key).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                if !actions.is_some_and(|m| m.contains_key(a)) {
                    out.push(issue(
                        Severity::Warning,
                        format!("sections[{id}].{key}"),
                        format!("unknown action `{a}`"),
                    ));
                }
            }
        }
        for c in s.get("columns").and_then(Value::as_array).into_iter().flatten() {
            if let Some(l) = c.get("link").and_then(Value::as_str)
                && !links.is_some_and(|m| m.contains_key(l))
            {
                let col = c.get("id").and_then(Value::as_str).unwrap_or("?");
                out.push(issue(
                    Severity::Warning,
                    format!("sections[{id}].columns[{col}].link"),
                    format!("unknown link `{l}`"),
                ));
            }
        }
    }
    out
}

/// `plan = "<section id>"` (a `track` section, a strip cell) must name a `plan` section.
fn plan_refs(config: &Value, out: &mut Vec<Issue>) {
    let sections = config.get("sections").and_then(Value::as_array);
    let is_plan = |id: &str| {
        sections.into_iter().flatten().any(|s| {
            s.get("id").and_then(Value::as_str) == Some(id) && s.get("widget").and_then(Value::as_str) == Some("plan")
        })
    };
    let mut check = |path: String, v: Option<&Value>| {
        if let Some(id) = v.and_then(Value::as_str)
            && !is_plan(id)
        {
            out.push(issue(Severity::Warning, path, format!("`{id}` isn't a `plan` section")));
        }
    };
    for s in sections.into_iter().flatten().filter(|s| s.get("widget").and_then(Value::as_str) == Some("track")) {
        check(format!("sections[{}].plan", s.get("id").and_then(Value::as_str).unwrap_or("?")), s.get("plan"));
    }
    for c in config.pointer("/strip/cells").and_then(Value::as_array).into_iter().flatten() {
        check(format!("strip.cells[{}].plan", c.get("id").and_then(Value::as_str).unwrap_or("?")), c.get("plan"));
    }
}

/// Keys of an action that decide what a `run` button executes. The mission layer may not set them, even on
/// an action defined elsewhere (that would rewrite the user's command).
const RUN_KEYS: [&str; 4] = ["command", "cwd", "env", "timeout_s"];

/// The mission layer is written by the agent and the page: no shell, no unsandboxed scripts, no permissions.
fn untrusted(doc: &Value, out: &mut Vec<Issue>) {
    for (id, a) in doc.get("actions").and_then(Value::as_object).into_iter().flatten() {
        if a.get("kind").and_then(Value::as_str) == Some("run") {
            out.push(issue(Severity::Error, format!("actions.{id}"), "`run` actions only in user/project config"));
        }
        for k in RUN_KEYS.iter().filter(|k| a.get(**k).is_some()) {
            out.push(issue(
                Severity::Error,
                format!("actions.{id}.{k}"),
                "what a `run` action executes is set only in user/project config",
            ));
        }
    }
    if doc.get("permissions").is_some() {
        out.push(issue(Severity::Error, "permissions", "permissions are set only in user/project config"));
    }
    for (id, w) in doc.get("widgets").and_then(Value::as_object).into_iter().flatten() {
        if w.get("script").is_some() {
            out.push(issue(
                Severity::Error,
                format!("widgets.{id}.script"),
                "script widgets only in user/project config; use widget `html` (sandboxed) here",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mission_layer_cannot_define_shell_or_scripts() {
        let doc = json!({"actions": {"x": {"kind": "run", "command": ["rm"]}}, "widgets": {"w": {"script": "a.js"}}});
        let issues = validate(&doc, &[Layer::new(LayerKind::Mission, "mission", doc.clone())]);
        assert_eq!(issues.iter().filter(|i| i.severity == Severity::Error).count(), 3, "kind, command, script");
        let ok = validate(&doc, &[Layer::new(LayerKind::User, "user", doc.clone())]);
        assert!(ok.iter().all(|i| i.severity == Severity::Warning), "{ok:?}");
    }

    #[test]
    fn mission_layer_cannot_rewrite_a_run_command_or_grant_permissions() {
        let doc = json!({"actions": {"logs": {"command": ["rm", "-rf", "/"]}}, "permissions": {"run_actions": true}});
        let issues = validate(&json!({}), &[Layer::new(LayerKind::Mission, "mission", doc)]);
        let paths: Vec<_> = issues.iter().filter(|i| i.severity == Severity::Error).map(|i| i.path.as_str()).collect();
        assert_eq!(paths, ["actions.logs.command", "permissions"]);
    }

    #[test]
    fn a_section_on_an_unknown_page_warns() {
        let doc = json!({"pages": [{"id": "plan"}], "sections": [{"id": "a", "widget": "feed", "page": "plan"}, {"id": "b", "widget": "feed", "page": "arch"}]});
        let issues = validate(&doc, &[]);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].path, "sections[b].page");
    }

    #[test]
    fn dangling_references_warn() {
        let doc = json!({"sections": [{"id": "s", "widget": "nope", "actions": ["ghost"], "columns": [{"id": "c", "link": "pr"}]}]});
        let issues = validate(&doc, &[]);
        assert_eq!(issues.len(), 3, "{issues:?}");
    }
}
