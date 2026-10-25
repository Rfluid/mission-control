//! Config tests across layers: template chains, user/project patches, mission patch order.

use super::*;
use serde_json::json;

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

#[test]
fn builtin_templates_all_resolve_cleanly() {
    let s = Stack::load(None, None).unwrap();
    for name in s.template_names() {
        let m = Layer::new(LayerKind::Mission, "mission", json!({"mission": {"template": name}}));
        let r = s.resolve(&[m]).unwrap();
        let errors: Vec<_> = r.issues.iter().filter(|i| i.severity == Severity::Error).collect();
        assert!(errors.is_empty(), "{name}: {errors:?}");
        assert!(r.config["sections"].as_array().is_some_and(|a| !a.is_empty()), "{name} has no sections");
    }
}

#[test]
fn user_patches_a_builtin_template_and_wins_over_it() {
    let user = tempfile::tempdir().unwrap();
    write(
        user.path(),
        "config.toml",
        "[mission]\ntemplate = \"minimal\"\n[[sections]]\nid = \"spend\"\nwidget = \"table\"\ncollection = \"spend\"\n",
    );
    write(user.path(), "templates/minimal.toml", "[[sections]]\nid = \"activity\"\ntitle = \"Log\"\n");
    let s = Stack::load(Some(user.path()), None).unwrap();
    let r = s.resolve(&[]).unwrap();
    let ids: Vec<_> = r.config["sections"].as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(ids.last(), Some(&"spend"));
    assert!(r.provenance["sections[activity].title"].starts_with("user template"));
    assert!(r.provenance["sections[spend].widget"].starts_with("user ("));
}

#[test]
fn mission_layer_picks_the_template_and_extends_is_followed() {
    let proj = tempfile::tempdir().unwrap();
    write(
        proj.path(),
        ".mission-control/templates/billing.toml",
        "extends = \"multi-agent-build\"\n[vars]\nrepo = \"acme/shop\"\n",
    );
    let s = Stack::load(None, Some(proj.path())).unwrap();
    let m = Layer::new(LayerKind::Mission, "mission", json!({"mission": {"template": "billing"}}));
    let r = s.resolve(&[m]).unwrap();
    assert_eq!(r.template, "billing");
    assert_eq!(r.config["vars"]["repo"], "acme/shop");
    assert!(r.config["sections"].as_array().unwrap().iter().any(|x| x["id"] == "plan"));
}

#[test]
fn mission_patches_apply_in_order_over_the_template() {
    let s = Stack::load(None, None).unwrap();
    let v1 = Layer::new(LayerKind::Mission, "mission v1", json!({"mission": {"template": "minimal"}}));
    let v2 = Layer::new(LayerKind::Mission, "mission v2", json!({"sections": [{"id": "activity", "remove": true}]}));
    let r = s.resolve(&[v1, v2]).unwrap();
    let ids: Vec<_> = r.config["sections"].as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["needs", "status", "overview", "recent"]);
}

#[test]
fn extends_cycles_are_reported() {
    let user = tempfile::tempdir().unwrap();
    write(user.path(), "templates/a.toml", "extends = \"b\"\n");
    write(user.path(), "templates/b.toml", "extends = \"a\"\n");
    let s = Stack::load(Some(user.path()), None).unwrap();
    let m = Layer::new(LayerKind::Mission, "mission", json!({"mission": {"template": "a"}}));
    assert!(s.resolve(&[m]).is_err());
}

#[test]
fn the_examples_resolve_cleanly_with_every_template() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let s = Stack::load(Some(&root.join("user-config")), Some(&root.join("project"))).unwrap();
    for name in s.template_names() {
        let m = Layer::new(LayerKind::Mission, "mission", json!({"mission": {"template": name}}));
        let r = s.resolve(&[m]).unwrap();
        let errors: Vec<_> = r.issues.iter().filter(|i| i.severity == Severity::Error).collect();
        assert!(errors.is_empty(), "{name}: {errors:?}");
    }
}

#[test]
fn a_patch_for_a_section_the_page_lacks_is_dropped() {
    let user = tempfile::tempdir().unwrap();
    write(user.path(), "config.toml", "[[sections]]\nid = \"prs\"\nrow_actions = [\"x\"]\n");
    let s = Stack::load(Some(user.path()), None).unwrap();
    let r = s.resolve(&[Layer::new(LayerKind::Mission, "m", json!({"mission": {"template": "minimal"}}))]).unwrap();
    assert!(r.config["sections"].as_array().unwrap().iter().all(|x| x["id"] != "prs"));
    assert!(!r.provenance.keys().any(|k| k.starts_with("sections[prs]")));
    let r = s.resolve(&[Layer::new(LayerKind::Mission, "m", json!({"mission": {"template": "pr-babysit"}}))]).unwrap();
    let prs = r.config["sections"].as_array().unwrap().iter().find(|x| x["id"] == "prs").unwrap().clone();
    assert_eq!(prs["row_actions"], json!(["x"]), "where the section exists, the patch applies");
}

#[test]
fn ui_shortcuts_default_on_and_a_user_file_turns_them_off() {
    let s = Stack::load(None, None).unwrap();
    let r = s.resolve(&[]).unwrap();
    assert_eq!(r.config["ui"]["shortcuts"], true);
    assert_eq!(r.provenance["ui.shortcuts"], "builtin");
    let user = tempfile::tempdir().unwrap();
    write(user.path(), "config.toml", "[ui]\nshortcuts = false\n");
    let r = Stack::load(Some(user.path()), None).unwrap().resolve(&[]).unwrap();
    assert_eq!(r.config["ui"]["shortcuts"], false);
    assert!(r.provenance["ui.shortcuts"].starts_with("user ("), "{:?}", r.provenance["ui.shortcuts"]);
    let bad = Layer::new(LayerKind::Mission, "mission", json!({"ui": {"shortcuts": "no"}}));
    assert!(
        s.resolve(&[bad]).unwrap().issues.iter().any(|i| i.severity == Severity::Error && i.path.starts_with("ui"))
    );
}
