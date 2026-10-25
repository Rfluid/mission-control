//! Patching TOML files in place, backups and the undo history.

use super::*;
use serde_json::json;

const USER: &str = r#"# my defaults
[vars]
repo = "a/b" # the main repo

[[sections]]
id = "prs"
title = "PRs"
columns = [{ id = "key" }, { id = "state", as = "pill" }]

[[sections]]
id = "spend"
widget = "spend"
"#;

#[test]
fn patches_keep_comments_and_merge_by_id() {
    let patch = json!({
        "vars": {"repo": "c/d", "linear": "x"},
        "sections": [
            {"id": "prs", "row_actions": ["rerun"], "columns": [{"id": "state", "label": "State"}, {"id": "owner"}]},
            {"id": "spend", "remove": true},
            {"id": "burn", "widget": "chart", "after": "prs"},
        ],
        "actions": {"rerun": {"label": "Re-run", "kind": "ask_agent"}},
    });
    let out = patch_toml(USER, &patch).unwrap();
    assert!(out.starts_with("# my defaults\n"), "{out}");
    assert!(out.contains(r#"repo = "c/d" # the main repo"#), "{out}");
    let v: Value = crate::config::parse(&out, "out").unwrap();
    assert_eq!(v["vars"], json!({"repo": "c/d", "linear": "x"}));
    let secs = v["sections"].as_array().unwrap();
    assert_eq!(secs[0]["row_actions"], json!(["rerun"]));
    assert_eq!(secs[0]["title"], "PRs", "untouched keys stay");
    assert_eq!(
        secs[0]["columns"],
        json!([{"id": "key"}, {"id": "state", "as": "pill", "label": "State"}, {"id": "owner"}])
    );
    assert_eq!(secs[1], json!({"id": "spend", "remove": true}), "a removal replaces the file's item");
    assert_eq!(secs[2], json!({"id": "burn", "widget": "chart", "after": "prs"}));
    assert!(out.contains("[actions.rerun]") && !out.contains("[actions]\n"), "{out}");
}

#[test]
fn null_deletes_and_unset_is_kept_as_a_directive() {
    let out = patch_toml(
        "[brief]\nmax_chars = 10\ncollections = [\"a\"]\n",
        &json!({"brief": {"max_chars": null, "collections": "!unset"}}),
    )
    .unwrap();
    let v: Value = crate::config::parse(&out, "out").unwrap();
    assert_eq!(v, json!({"brief": {"collections": "!unset"}}));
}

#[test]
fn patching_an_empty_file_and_a_non_table_list() {
    let out = patch_toml("", &json!({"sections": [{"id": "a", "widget": "feed"}]})).unwrap();
    assert_eq!(out, "[[sections]]\nid = \"a\"\nwidget = \"feed\"\n");
    assert!(patch_toml("sections = 3\n", &json!({"sections": [{"id": "a"}]})).is_err());
    assert!(patch_toml("", &json!([1])).is_err());
}

#[test]
fn the_backup_is_the_file_before_the_first_edit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "# mine\n").unwrap();
    let file = LayerFile::new(path.clone(), &dir.path().join("state"));
    file.write("x = 1\n", None).unwrap();
    file.write("x = 2\n", None).unwrap();
    assert_eq!(std::fs::read_to_string(dir.path().join("config.toml.mission-control.bak")).unwrap(), "# mine\n");
}

#[test]
fn writes_back_up_once_and_undo_walks_back() {
    let dir = tempfile::tempdir().unwrap();
    let file = LayerFile::new(dir.path().join("cfg/config.toml"), &dir.path().join("state"));
    assert!(file.write("a = 1\n", Some("first")).unwrap());
    assert!(!file.write("a = 1\n", None).unwrap(), "unchanged: no write");
    assert!(!dir.path().join("cfg/config.toml.mission-control.bak").exists(), "nothing to back up the first time");
    assert!(file.write("a = 2\n", Some("second")).unwrap());
    assert!(
        !dir.path().join("cfg/config.toml.mission-control.bak").exists(),
        "the backup is only ever the user's original"
    );
    assert_eq!(
        file.history().iter().map(|s| s.note.clone()).collect::<Vec<_>>(),
        [Some("second".into()), Some("first".into())]
    );
    assert_eq!(file.undo().unwrap().unwrap().note.as_deref(), Some("second"));
    assert_eq!(file.read().unwrap(), "a = 1\n");
    file.undo().unwrap();
    assert!(!file.path.exists(), "undoing the first write removes the file it created");
    assert!(file.undo().unwrap().is_none());
}

#[test]
fn list_edits_change_the_files_own_list() {
    let out = patch_toml(
        "[toolbar]\nactions = [\"ask\", \"customize\"]\n",
        &json!({"toolbar": {"actions": {"+": ["burn"], "-": ["ask"]}}}),
    )
    .unwrap();
    let v: Value = crate::config::parse(&out, "out").unwrap();
    assert_eq!(v["toolbar"]["actions"], json!(["customize", "burn"]));
    let out = patch_toml("", &json!({"toolbar": {"actions": {"+": ["burn"]}}})).unwrap();
    let v: Value = crate::config::parse(&out, "out").unwrap();
    assert_eq!(v["toolbar"]["actions"], json!({"+": ["burn"]}), "no list here: the edit is kept for lower layers");
}
