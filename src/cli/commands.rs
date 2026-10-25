//! What each subcommand does. Output is JSON on stdout, for agents and scripts.

use super::{Cmd, ConfigCmd, more};
use crate::api::{self, Api};
use crate::lifecycle;
use crate::model::{NewMission, Op};
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::io::Read;

pub fn dispatch(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Serve { port } => tokio::runtime::Runtime::new()?.block_on(crate::daemon::serve(port)),
        Cmd::Status => {
            let s = lifecycle::running().map(|(port, h)| {
                json!({"running": true, "port": port, "pid": h.pid, "version": h.version, "started": h.started})
            });
            print(&s.unwrap_or_else(|| json!({"running": false})))
        }
        Cmd::Stop => print(&json!({"stopped": lifecycle::stop()?})),
        Cmd::New { title, template, session, project, force, review } => {
            let project_dir = Some(api::project_dir(project.as_deref())?.display().to_string());
            print(&more::new_mission(NewMission { title, session, project_dir, template, force }, review)?)
        }
        Cmd::Update { action } => more::update_cmd(action),
        Cmd::Review { target, off, redline } => print(&more::review(&target.mission, &redline, off)?),
        Cmd::Attach { id, session } => print(&serde_json::to_value(Api::connect()?.attach(&id, session.as_deref())?)?),
        Cmd::List { all } => print(&if all { Api::connect()?.list_all(true)? } else { Api::connect()?.list()? }),
        Cmd::Archive { target, restore } => print(&Api::connect()?.archive(&target.mission, !restore)?),
        Cmd::Fork { target, title, no_records } => {
            print(&serde_json::to_value(Api::connect()?.fork(&target.mission, &title, !no_records)?)?)
        }
        Cmd::View { target, collections, events, no_config, page } => {
            print(&Api::connect()?.view_of(&target.mission, &collections, events, !no_config, page.as_deref())?)
        }
        Cmd::Upsert { target, collection, key, state, fields } => {
            let patch = fields_to_patch(&fields)?;
            apply(&target.mission, vec![Op::Upsert { collection, key, state, patch }])
        }
        Cmd::Rm { target, collection, key } => apply(&target.mission, vec![Op::Remove { collection, key }]),
        Cmd::Log { target, text, kind } => apply(&target.mission, vec![Op::Log { at: None, kind, text, data: None }]),
        Cmd::Need { target, key, question, context, options, recommendation, why, blocks } => {
            let blocks = match blocks.len() {
                0 => Value::Null,
                1 => Value::String(blocks[0].clone()),
                _ => json!(blocks),
            };
            let patch = json!({"question": question, "context": context, "options": options,
                               "recommendation": recommendation, "why": why, "blocks": blocks});
            let op =
                Op::Upsert { collection: "needs".into(), key, state: Some("open".into()), patch: drop_nulls(patch) };
            apply(&target.mission, vec![op])
        }
        Cmd::Batch { target, file } => {
            // No file reads stdin; `-` does too, except through Windows PowerShell's `-File`, which rejects a bare `-`.
            let text = match file.as_deref() {
                None | Some("-") => {
                    let mut s = String::new();
                    std::io::stdin().read_to_string(&mut s)?;
                    s
                }
                Some(file) => std::fs::read_to_string(file).with_context(|| format!("reading {file}"))?,
            };
            apply(&target.mission, serde_json::from_str(&text).context("ops must be a JSON array of {op, ...}")?)
        }
        Cmd::Config(c) => config(c),
        Cmd::Wait { target, session, since, timeout } => {
            let session = session.unwrap_or_else(|| "default".into());
            print(&serde_json::to_value(Api::connect()?.wait_through_restarts(
                &target.mission,
                &session,
                since,
                timeout,
            )?)?)
        }
        Cmd::Brief { target, session, max_chars } => {
            let b = Api::connect()?.brief(&target.mission, session.as_deref().unwrap_or("default"), max_chars)?;
            println!("{}", b["text"].as_str().unwrap_or_default());
            Ok(())
        }
        Cmd::Hook { print_config: true } => print(&more::hook_config()),
        Cmd::Hook { print_config: false } => {
            if let Err(e) = more::hook() {
                eprintln!("mission-control hook: {e:#}");
            }
            Ok(())
        }
        Cmd::Mcp { channel } => crate::mcp::serve(channel),
        Cmd::Schema => print(&crate::config::schema::json_schema()),
        Cmd::Install(a) => crate::install::install(&a.into_opts()),
        Cmd::Uninstall(a) => crate::install::uninstall(&a.into_opts()),
        Cmd::Doctor(a) => {
            crate::install::doctor(&a.into_opts())?;
            more::missions_doctor()
        }
        Cmd::Import { file, title, template, project, session, write_config } => {
            more::import(&file, title, template, project.as_deref(), session, write_config)
        }
        Cmd::Export { target, output } => {
            let html = Api::connect()?.export(&target.mission)?;
            if let Some(p) = output {
                std::fs::write(&p, html).with_context(|| format!("writing {}", p.display()))?;
                print(&json!({"written": p, "note": "static snapshot: no token, buttons inert"}))
            } else {
                print!("{html}");
                Ok(())
            }
        }
    }
}

fn config(c: ConfigCmd) -> Result<()> {
    match c {
        ConfigCmd::Resolve { template, project } => {
            let stack = api::local_stack(project.as_deref())?;
            let layer = template.map(|t| {
                crate::config::Layer::new(
                    crate::config::LayerKind::Mission,
                    "--template",
                    json!({"mission": {"template": t}}),
                )
            });
            print(&serde_json::to_value(stack.resolve(layer.as_slice())?)?)
        }
        ConfigCmd::Get { target, path } => print(&Api::connect()?.config_at(&target.mission, path.as_deref())?),
        ConfigCmd::Explain { target, prefix } => {
            let r = Api::connect()?.explain_under(&target.mission, prefix.as_deref())?;
            for (k, v) in r["provenance"].as_object().into_iter().flatten() {
                println!("{k}  ←  {}", v.as_str().unwrap_or_default());
            }
            for i in r["issues"].as_array().into_iter().flatten() {
                println!(
                    "! {} {}: {}",
                    i["severity"].as_str().unwrap_or(""),
                    i["path"].as_str().unwrap_or(""),
                    i["message"].as_str().unwrap_or("")
                );
            }
            Ok(())
        }
        ConfigCmd::Patch { target, patch, note, layer, dry_run } => {
            let value: Value = serde_json::from_str(&patch).or_else(|_| crate::config::parse(&patch, "patch"))?;
            print(&Api::connect()?.patch_config(&target.mission, &layer, &value, note.as_deref(), dry_run)?)
        }
        ConfigCmd::TemplateSave { target, name, layer, description, replace } => {
            print(&Api::connect()?.save_template(&target.mission, &name, &layer, description.as_deref(), replace)?)
        }
        ConfigCmd::Widget { target, name, base, props, html, description, layer } => {
            let props: Option<Value> =
                props.map(|p| serde_json::from_str(&p)).transpose().context("--props must be a JSON object")?;
            let def = drop_nulls(json!({"base": base, "props": props, "html": html, "description": description}));
            print(&Api::connect()?.define_widget(&target.mission, &name, &def, &layer)?)
        }
        ConfigCmd::Undo { target, layer } => print(&Api::connect()?.undo_config(&target.mission, &layer)?),
        ConfigCmd::History { target, layer } => print(&Api::connect()?.config_history(&target.mission, &layer)?),
        ConfigCmd::Templates { project } => print(&api::local_stack(project.as_deref())?.summary()),
    }
}

fn apply(mission: &str, ops: Vec<Op>) -> Result<()> {
    print(&Api::connect()?.apply(mission, &ops)?)
}

/// `a=1 b=x c={"d":2}` → `{"a":1,"b":"x","c":{"d":2}}`. `k=null` deletes the field.
fn fields_to_patch(fields: &[String]) -> Result<Value> {
    let mut m = Map::new();
    for f in fields {
        let Some((k, v)) = f.split_once('=') else { bail!("expected field=value, got `{f}`") };
        m.insert(k.to_string(), serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.to_string())));
    }
    Ok(Value::Object(m))
}

fn drop_nulls(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.retain(|_, x| !x.is_null());
    }
    v
}

pub(super) fn print(v: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_parse_json_or_fall_back_to_strings() {
        let p =
            fields_to_patch(&["n=3".into(), "t=hello world".into(), "o={\"a\":1}".into(), "x=null".into()]).unwrap();
        assert_eq!(p, json!({"n": 3, "t": "hello world", "o": {"a": 1}, "x": null}));
        assert!(fields_to_patch(&["oops".into()]).is_err());
    }
}
