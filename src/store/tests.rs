//! Store tests: ops, merge patches, config versions, sessions (in-memory database).

use super::*;
use crate::model::Op;

fn store() -> (Store, String) {
    let mut s = Store::open(Path::new(":memory:")).unwrap();
    let m =
        s.create(&NewMission { title: "t".into(), template: Some("minimal".into()), ..Default::default() }).unwrap();
    (s, m.id)
}

fn upsert(collection: &str, key: &str, state: Option<&str>, patch: Value) -> Op {
    Op::Upsert { collection: collection.into(), key: key.into(), state: state.map(String::from), patch }
}

#[test]
fn upsert_merges_bodies_and_keeps_state() {
    let (mut s, id) = store();
    s.apply(&id, &[upsert("prs", "1", Some("open"), json!({"title": "a", "checks": "red"}))]).unwrap();
    s.apply(&id, &[upsert("prs", "1", None, json!({"checks": null, "review": "ok"}))]).unwrap();
    let r = &s.records(&id).unwrap()[0];
    assert_eq!(r.state.as_deref(), Some("open"));
    assert_eq!(r.body, json!({"title": "a", "review": "ok"}));
}

#[test]
fn batches_apply_in_order_and_unknown_missions_fail() {
    let (mut s, id) = store();
    let ops = [upsert("prs", "1", None, json!({})), Op::Remove { collection: "prs".into(), key: "1".into() }];
    s.apply(&id, &ops).unwrap();
    assert!(s.records(&id).unwrap().is_empty());
    assert!(s.apply("msn_missing", &ops[..1]).is_err());
}

#[test]
fn layer_patches_version_and_log() {
    let (mut s, id) = store();
    let v = s.patch_layer(&id, &json!({"vars": {"repo": "a/b"}}), Some("repo")).unwrap();
    assert_eq!(v.0, 2);
    assert_eq!(v.1.kind, "config");
    let layers = s.layers(&id).unwrap();
    assert_eq!(layers, vec![(1, json!({"mission": {"template": "minimal"}})), (2, json!({"vars": {"repo": "a/b"}}))]);
    assert_eq!(s.events(&id, 0, 10).unwrap().len(), 2);
}

#[test]
fn attach_rebinds_the_session_and_keeps_records() {
    let (mut s, id) = store();
    s.apply(&id, &[upsert("prs", "1", Some("open"), json!({}))]).unwrap();
    assert!(s.find_titled("ses_new", "t").unwrap().is_none());
    let m = s.attach(&id, Some("ses_new")).unwrap().unwrap();
    assert_eq!(m.session.as_deref(), Some("ses_new"));
    assert_eq!(s.find_titled("ses_new", "t").unwrap().unwrap().id, id);
    assert_eq!(s.records(&id).unwrap().len(), 1);
    assert!(s.attach("msn_missing", Some("x")).unwrap().is_none());
}

#[test]
fn prune_keeps_the_newest_events() {
    let (mut s, id) = store();
    let logs: Vec<Op> =
        (0..10).map(|i| Op::Log { kind: "log".into(), text: format!("e{i}"), data: None, at: None }).collect();
    s.apply(&id, &logs).unwrap();
    assert_eq!(s.prune_events(&id, 4).unwrap(), 7, "10 logs + the created config event, keep 4");
    let left: Vec<String> = s.events(&id, 0, 100).unwrap().into_iter().map(|e| e.text).collect();
    assert_eq!(left, ["e6", "e7", "e8", "e9"]);
    assert_eq!(s.prune_events(&id, 4).unwrap(), 0);
    let first = s.events_before(&id, None, 100).unwrap()[0].seq;
    let older = s.events_before(&id, Some(first + 2), 10).unwrap();
    assert_eq!(older.iter().map(|e| e.text.as_str()).collect::<Vec<_>>(), ["e6", "e7"]);
}

#[test]
fn compaction_folds_old_versions_without_changing_the_config() {
    let (mut s, id) = store();
    let patches = [
        json!({"sections": [{"id": "status", "remove": true}]}),
        json!({"sections": [{"id": "x", "widget": "feed", "before": "needs"}]}),
        json!({"sections": [{"id": "status", "widget": "markdown", "text": "back"}], "vars": {"a": 1}}),
        json!({"vars": {"a": "!unset", "b": 2}}),
    ];
    for p in &patches {
        s.patch_layer(&id, p, None).unwrap();
    }
    let stack = crate::config::Stack::load(None, None).unwrap();
    let resolve = |s: &mut Store| {
        let layers: Vec<crate::config::Layer> = s
            .layers(&id)
            .unwrap()
            .into_iter()
            .map(|(v, d)| crate::config::Layer::new(crate::config::LayerKind::Mission, format!("v{v}"), d))
            .collect();
        stack.resolve(&layers).unwrap().config
    };
    let before = resolve(&mut s);
    assert_eq!(s.compact_layers(&id, 2).unwrap(), 3, "5 versions, keep 2: fold 4 into 1");
    assert_eq!(s.layers(&id).unwrap().iter().map(|(v, _)| *v).collect::<Vec<_>>(), [1, 5]);
    assert_eq!(resolve(&mut s), before);
    assert_eq!(s.compact_layers(&id, 2).unwrap(), 0);
}

#[test]
fn state_changes_log_events_and_a_record_has_its_history() {
    let (mut s, id) = store();
    let a = s.apply(&id, &[upsert("tasks", "S1", Some("todo"), json!({"name": "Storage adapter"}))]).unwrap();
    assert!(a.events.is_empty(), "a new record doesn't log");
    let a = s.apply(&id, &[upsert("tasks", "S1", Some("in_progress"), json!({}))]).unwrap();
    assert_eq!(a.events[0].kind, "state");
    assert_eq!(a.events[0].text, "S1 Storage adapter → in progress");
    assert_eq!(
        a.events[0].data,
        Some(json!({"collection": "tasks", "key": "S1", "from": "todo", "to": "in_progress"}))
    );
    assert!(
        s.apply(&id, &[upsert("tasks", "S1", None, json!({"note": "x"}))]).unwrap().events.is_empty(),
        "same state"
    );
    let n = s.apply(&id, &[upsert("needs", "N-1", Some("open"), json!({"question": "Ship?"}))]).unwrap();
    assert_eq!((n.events[0].kind.as_str(), n.events[0].text.as_str()), ("need", "N-1 opened: Ship?"));
    assert!(s.apply(&id, &[upsert("needs", "N-1", Some("answered"), json!({}))]).unwrap().events.is_empty());
    s.apply(
        &id,
        &[Op::Log {
            kind: "action".into(),
            text: "Retry".into(),
            at: None,
            data: Some(json!({"action": "retry", "row": {"collection": "tasks", "key": "S1"}})),
        }],
    )
    .unwrap();
    s.apply(&id, &[upsert("tasks", "S2", Some("todo"), json!({})), upsert("tasks", "S2", Some("done"), json!({}))])
        .unwrap();
    let h = s.record_events(&id, "tasks", "S1", 50).unwrap();
    let kinds: Vec<_> = h.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(kinds, ["state", "action"]);
    assert_eq!(s.record_events(&id, "needs", "N-1", 50).unwrap().len(), 1);
}
