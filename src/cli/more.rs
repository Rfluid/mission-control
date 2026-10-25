//! CLI commands that do more than map arguments to one call: import, hooks, the missions part of doctor.

use crate::api::{self, Api};
use crate::model::{NewMission, Op};
use crate::paths;
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::io::Read;
use std::path::PathBuf;

use super::commands::print;

pub(super) fn import(
    file: &std::path::Path,
    title: Option<String>,
    template: String,
    project: Option<&std::path::Path>,
    session: Option<String>,
    write_config: bool,
) -> Result<()> {
    let text = std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let doc: Value = serde_json::from_str(&text).with_context(|| format!("parsing {}", file.display()))?;
    let imported = crate::import::convert(&doc)?;
    let title = title.or(imported.title.clone()).unwrap_or_else(|| "Imported mission".into());
    let api = Api::connect()?;
    let project_dir = Some(api::project_dir(project)?.display().to_string());
    let created = api.create(&NewMission { title, session, project_dir, template: Some(template), force: true })?;
    let id = created.mission.id.clone();
    for chunk in imported.ops.chunks(crate::import::BATCH) {
        api.apply(&id, chunk)?;
    }
    let states = api.config_at(&id, Some("states")).unwrap_or(Value::Null);
    let suggested = json!({"states": crate::import::suggested_states(&imported.states, &states)});
    if write_config && suggested["states"].as_object().is_some_and(|m| !m.is_empty()) {
        api.patch_config(&id, "project", &suggested, Some("states from import"), false)?;
    }
    let records = imported.ops.iter().filter(|o| matches!(o, Op::Upsert { .. })).count();
    print(&json!({"mission": created.mission, "url": created.url, "imported": {"records": records,
        "events": imported.ops.len() - records}, "suggested_config": suggested, "config_written": write_config}))
}

/// The missions part of `mission-control doctor`: one line per finding, nothing for a healthy mission.
pub(super) fn missions_doctor() -> Result<()> {
    let report = Api::connect()?.doctor()?;
    let all = report.as_array().cloned().unwrap_or_default();
    println!("missions {} live", all.len());
    for m in &all {
        let mut notes: Vec<String> = Vec::new();
        for i in m["issues"].as_array().into_iter().flatten() {
            notes.push(format!(
                "{} {}: {}",
                i["severity"].as_str().unwrap_or(""),
                i["path"].as_str().unwrap_or(""),
                i["message"].as_str().unwrap_or("")
            ));
        }
        for b in m["big_collections"].as_array().into_iter().flatten() {
            notes.push(format!(
                "collection `{}` has {} records: filter its table, or keep fewer",
                b["collection"].as_str().unwrap_or(""),
                b["records"]
            ));
        }
        let list =
            |k: &str| m[k].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(", ");
        if !list("empty_sections").is_empty() {
            notes.push(format!("sections with no records yet: {}", list("empty_sections")));
        }
        if !list("unused_actions").is_empty() {
            notes.push(format!("actions no section or toolbar uses: {}", list("unused_actions")));
        }
        if m["idle"].as_bool() == Some(true) {
            notes.push("idle: no activity for a week (archive it, or attach a session)".into());
        }
        let head = format!("  {} {}", m["id"].as_str().unwrap_or(""), m["title"].as_str().unwrap_or(""));
        if notes.is_empty() {
            println!("{head}: ok");
        } else {
            println!("{head}:");
            for n in notes {
                println!("    - {n}");
            }
        }
    }
    Ok(())
}

/// The `hooks` block for Claude Code's `settings.json`, running this clone's entry point.
pub(super) fn hook_config() -> Value {
    let cmd = paths::entry_point(&paths::repo_dir()).display().to_string() + " hook";
    let h = |matcher: Option<&str>| {
        let mut v = json!({"hooks": [{"type": "command", "command": cmd, "timeout": 10}]});
        if let Some(m) = matcher {
            v["matcher"] = json!(m);
        }
        v
    };
    json!({"hooks": {"PreToolUse": [h(Some("Task|Agent"))], "PostToolUse": [h(Some("Task|Agent|Bash"))], "Stop": [h(None)]}})
}

/// One hook event: the mission is `$MISSION_CONTROL_ID`, else this session's, else the newest in this project.
pub(super) fn hook() -> Result<()> {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text)?;
    let payload: Value = serde_json::from_str(&text).context("hook payload")?;
    let api = Api::connect()?;
    let Some(mission) = hook_mission(&api, &payload)? else { return Ok(()) };
    let ingest = api.config_at(&mission, Some("ingest")).unwrap_or(Value::Null);
    let transcript = payload["transcript_path"].as_str().and_then(|p| std::fs::read_to_string(p).ok());
    let ops = crate::hooks::ops(&payload, &ingest, transcript.as_deref());
    if !ops.is_empty() {
        api.apply(&mission, &ops)?;
    }
    Ok(())
}

pub(super) fn hook_mission(api: &Api, payload: &Value) -> Result<Option<String>> {
    if let Ok(id) = std::env::var("MISSION_CONTROL_ID")
        && !id.is_empty()
    {
        return Ok(Some(id));
    }
    let list = api.list()?;
    let missions = list.as_array().cloned().unwrap_or_default();
    let session = payload["session_id"].as_str().unwrap_or_default();
    if let Some(m) = missions.iter().find(|m| !session.is_empty() && m["session"].as_str() == Some(session)) {
        return Ok(m["id"].as_str().map(String::from));
    }
    let cwd = payload["cwd"].as_str().map_or_else(std::env::current_dir, |c| Ok(PathBuf::from(c)))?;
    let root = paths::project_root(&cwd).display().to_string();
    // Newest first (the list is ordered by creation).
    Ok(missions
        .iter()
        .find(|m| m["project_dir"].as_str() == Some(root.as_str()))
        .and_then(|m| m["id"].as_str().map(String::from)))
}

/// `mission-control review`: registers the mission page with redline and turns on its overlay (or `off`).
pub(super) fn review(mission: &str, redline: &str, off: bool) -> Result<Value> {
    let api = Api::connect()?;
    if off {
        return api.patch_config(mission, "mission", &json!({"mission": {"review": false}}), Some("review off"), false);
    }
    let view = api.view_of(mission, &[], Some(0), false, None)?;
    let title = view["mission"]["title"].as_str().unwrap_or("mission");
    let url = format!("{}/m/{mission}", api.base());
    let out = std::process::Command::new(redline)
        .args(["register", title, "--uri", &url, "--kind", "mission"])
        .output()
        .with_context(|| format!("running `{redline} register` (is redline installed? set MISSION_CONTROL_REDLINE)"))?;
    if !out.status.success() {
        anyhow::bail!("redline register failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let reg: Value = serde_json::from_slice(&out.stdout).context("redline register output")?;
    let embed = reg["embed"].as_str().unwrap_or_default();
    let attr =
        |name: &str| embed.split(&format!("{name}=\"")).nth(1).and_then(|r| r.split('"').next()).map(String::from);
    let base = attr("src").and_then(|s| s.strip_suffix("/client.js").map(String::from));
    let (Some(base), Some(artifact), Some(token)) = (base, attr("data-redline-artifact"), attr("data-redline-token"))
    else {
        anyhow::bail!("couldn't read redline's embed snippet: {embed}");
    };
    let op = Op::Upsert {
        collection: "_review".into(),
        key: "redline".into(),
        state: None,
        patch: json!({"base": base, "artifact": artifact, "token": token}),
    };
    api.apply(mission, &[op])?;
    api.patch_config(mission, "mission", &json!({"mission": {"review": true}}), Some("review on (redline)"), false)?;
    Ok(json!({"review": true, "artifact_id": artifact, "url": url,
        "next": format!("Reload the page to comment on it. Comments reach you through redline (wait_for_comments, or `redline wait --artifact {artifact}`).")}))
}

/// `mission-control update [check|skip|off|on]` (ported from redline).
pub(super) fn update_cmd(action: Option<super::UpdateCmd>) -> Result<()> {
    use super::UpdateCmd;
    use crate::update;
    let files = update::Files::default();
    match action {
        None => print(&update::apply::apply(&files)?),
        Some(UpdateCmd::Check) => {
            update::config::load(&files.config)?; // surface a broken config.toml here
            let c = update::check(&files);
            let mut v = serde_json::to_value(update::info(&files))?;
            if let Some(e) = c.error {
                v["error"] = json!(e);
            }
            v["config"] = json!(files.config);
            print(&v)
        }
        Some(UpdateCmd::Skip { version }) => print(&serde_json::to_value(update::skip(&files, version.as_deref())?)?),
        Some(UpdateCmd::Off) => print(&serde_json::to_value(update::set_check(&files, false)?)?),
        Some(UpdateCmd::On) => print(&serde_json::to_value(update::set_check(&files, true)?)?),
    }
}

/// `mission-control new`: the created mission, an update notice when there is one, and review set up if asked.
pub(super) fn new_mission(req: NewMission, review: bool) -> Result<Value> {
    let created = Api::connect()?.create(&req)?;
    let mut out = serde_json::to_value(&created)?;
    if let (Some(Value::Object(u)), Some(o)) = (crate::update::agent_notice(), out.as_object_mut()) {
        o.extend(u);
    }
    if review {
        let redline = std::env::var("MISSION_CONTROL_REDLINE").unwrap_or_else(|_| "redline".into());
        out["review"] = self::review(&created.mission.id, &redline, false)?;
    }
    Ok(out)
}
