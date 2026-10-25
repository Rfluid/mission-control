//! Layer merging (see `docs/CONFIG.md`, "Merge rules"), recording which layer set each key.
//!
//! Paths look like `mission.template`, `sections[prs].columns[state].as`: arrays merged by id show the id in
//! brackets; other arrays are leaves.

use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Removes a key when used as its value.
pub const UNSET: &str = "!unset";
/// Keys on an id'd array item that steer the merge and are not stored.
const DIRECTIVES: [&str; 3] = ["remove", "before", "after"];

/// Resolved key path → label of the layer that set it last.
pub type Provenance = BTreeMap<String, String>;

/// Merges `patch` into `base`, attributing every key it sets to `src`.
pub fn merge(base: &mut Value, patch: &Value, src: &str, prov: &mut Provenance) {
    merge_at(base, patch, "", src, prov);
}

/// `{"+" = [...], "-" = [...]}`: adds items to (and removes items from) a plain list instead of replacing it.
pub fn list_edit(v: &Value) -> Option<(&[Value], &[Value])> {
    let o = v.as_object()?;
    if o.is_empty() || !o.keys().all(|k| k == "+" || k == "-") {
        return None;
    }
    let get = |k: &str| o.get(k).map_or(Some(&[][..]), |x| x.as_array().map(Vec::as_slice));
    Some((get("+")?, get("-")?))
}

fn merge_at(base: &mut Value, patch: &Value, at: &str, src: &str, prov: &mut Provenance) {
    if let Some((add, del)) = list_edit(patch)
        && (base.is_array() || base.is_null())
    {
        let list = base.as_array().cloned().unwrap_or_default();
        let mut list: Vec<Value> = list.into_iter().filter(|x| !del.contains(x)).collect();
        for a in add {
            if !list.contains(a) {
                list.push(a.clone());
            }
        }
        *base = Value::Array(list);
        prov.insert(at.to_string(), src.to_string());
        return;
    }
    match patch {
        Value::Object(p) => {
            if !base.is_object() {
                *base = Value::Object(Map::new());
                forget(prov, at);
            }
            let b = base.as_object_mut().expect("just made an object");
            for (k, v) in p {
                let sub = join(at, k);
                if v.as_str() == Some(UNSET) {
                    b.remove(k);
                    forget(prov, &sub);
                } else {
                    merge_at(b.entry(k.clone()).or_insert(Value::Null), v, &sub, src, prov);
                }
            }
        }
        Value::Array(items) if is_id_list(items) && (base.is_array() || base.is_null()) => {
            if base.is_null() {
                *base = Value::Array(Vec::new());
            }
            merge_by_id(base.as_array_mut().expect("checked"), items, at, src, prov);
        }
        _ => {
            *base = patch.clone();
            forget(prov, at);
            prov.insert(at.to_string(), src.to_string());
        }
    }
}

fn merge_by_id(base: &mut Vec<Value>, items: &[Value], path: &str, src: &str, prov: &mut Provenance) {
    for item in items {
        let id = item_id(item).expect("checked by is_id_list").to_string();
        let sub = format!("{path}[{id}]");
        let at = base.iter().position(|b| item_id(b) == Some(id.as_str()));
        if item.get("remove").and_then(Value::as_bool) == Some(true) {
            if let Some(i) = at {
                base.remove(i);
            }
            forget(prov, &sub);
            continue;
        }
        let mut body = item.clone();
        let fields = body.as_object_mut().expect("checked");
        let before = fields.remove("before").and_then(|v| v.as_str().map(String::from));
        let after = fields.remove("after").and_then(|v| v.as_str().map(String::from));
        for d in DIRECTIVES {
            fields.remove(d);
        }
        let mut merged = match at {
            Some(i) => base.remove(i),
            None => Value::Null,
        };
        merge_at(&mut merged, &body, &sub, src, prov);
        let slot = place(base, before.as_deref(), after.as_deref()).unwrap_or(at.unwrap_or(base.len()));
        base.insert(slot.min(base.len()), merged);
    }
}

/// Combines two patches into one that does what applying `base` then `patch` does, keeping the directives
/// (`remove`, `before`, `after`, `"!unset"`) a later merge needs. For saving a mission's patches as a template.
pub fn compose(base: &mut Value, patch: &Value) {
    match (base.as_object_mut(), patch) {
        (Some(b), Value::Object(p)) => {
            for (k, v) in p {
                let slot = b.entry(k.clone()).or_insert(Value::Null);
                if let Some((add, del)) = list_edit(v) {
                    compose_list_edit(slot, add, del);
                    continue;
                }
                let id_lists =
                    slot.as_array().is_some_and(|i| is_id_list(i)) && v.as_array().is_some_and(|n| is_id_list(n));
                if id_lists {
                    compose_by_id(slot.as_array_mut().expect("checked"), v.as_array().expect("checked"));
                } else if v.is_object() && slot.is_object() {
                    compose(slot, v);
                } else {
                    *slot = v.clone();
                }
            }
        }
        _ => *base = patch.clone(),
    }
}

/// A list edit after an earlier patch: applied to a list it replaced, or folded into its own list edit.
fn compose_list_edit(slot: &mut Value, add: &[Value], del: &[Value]) {
    if let Value::Array(list) = slot {
        list.retain(|x| !del.contains(x));
        let new: Vec<Value> = add.iter().filter(|a| !list.contains(a)).cloned().collect();
        list.extend(new);
        return;
    }
    let (mut plus, mut minus) = match list_edit(slot) {
        Some((p, m)) => (p.to_vec(), m.to_vec()),
        None => (Vec::new(), Vec::new()),
    };
    plus.retain(|x| !del.contains(x));
    minus.retain(|x| !add.contains(x));
    for a in add {
        if !plus.contains(a) {
            plus.push(a.clone());
        }
    }
    for d in del {
        if !minus.contains(d) {
            minus.push(d.clone());
        }
    }
    *slot = serde_json::json!({"+": plus, "-": minus});
}

fn compose_by_id(items: &mut Vec<Value>, new: &[Value]) {
    let is_removal = |v: &Value| v.get("remove").and_then(Value::as_bool) == Some(true);
    for item in new {
        let id = item_id(item);
        if is_removal(item) {
            // Whatever earlier patches did to it, the item ends up gone.
            items.retain(|b| item_id(b) != id);
            items.push(item.clone());
            continue;
        }
        match items.iter().rposition(|b| item_id(b) == id) {
            // Removed earlier, added back now: keep both steps, so the merge removes and then appends.
            Some(i) if is_removal(&items[i]) => items.push(item.clone()),
            Some(i) => compose(&mut items[i], item),
            None => items.push(item.clone()),
        }
    }
}

/// Index to insert at for `before`/`after` an existing id; `None` keeps the item where it was.
fn place(base: &[Value], before: Option<&str>, after: Option<&str>) -> Option<usize> {
    let find = |id: &str| base.iter().position(|b| item_id(b) == Some(id));
    before.and_then(find).or_else(|| after.and_then(find).map(|i| i + 1))
}

fn is_id_list(items: &[Value]) -> bool {
    !items.is_empty() && items.iter().all(|i| item_id(i).is_some())
}

fn item_id(v: &Value) -> Option<&str> {
    v.as_object()?.get("id")?.as_str()
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() { key.to_string() } else { format!("{path}.{key}") }
}

/// Drops provenance for `path` and everything under it.
fn forget(prov: &mut Provenance, path: &str) {
    if path.is_empty() {
        prov.clear();
        return;
    }
    prov.retain(|k, _| {
        k != path && !(k.starts_with(path) && matches!(k.as_bytes().get(path.len()), Some(b'.' | b'[')))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run(layers: &[(&str, Value)]) -> (Value, Provenance) {
        let (mut v, mut p) = (Value::Null, Provenance::new());
        for (src, doc) in layers {
            merge(&mut v, doc, src, &mut p);
        }
        (v, p)
    }

    #[test]
    fn tables_merge_and_record_the_last_writer() {
        let (v, p) = run(&[
            ("builtin", json!({"mission": {"refresh": "live", "template": "minimal"}})),
            ("user", json!({"mission": {"template": "research"}})),
        ]);
        assert_eq!(v, json!({"mission": {"refresh": "live", "template": "research"}}));
        assert_eq!(p["mission.refresh"], "builtin");
        assert_eq!(p["mission.template"], "user");
    }

    #[test]
    fn id_arrays_merge_by_id_and_position() {
        let (v, p) = run(&[
            ("builtin", json!({"sections": [{"id": "a", "widget": "kpis"}, {"id": "b", "widget": "feed"}]})),
            ("user", json!({"sections": [{"id": "b", "title": "Log"}, {"id": "c", "widget": "table", "after": "a"}]})),
            ("mission", json!({"sections": [{"id": "a", "remove": true}]})),
        ]);
        assert_eq!(
            v,
            json!({"sections": [{"id": "c", "widget": "table"}, {"id": "b", "widget": "feed", "title": "Log"}]})
        );
        assert_eq!(p["sections[b].title"], "user");
        assert_eq!(p["sections[b].widget"], "builtin");
        assert!(!p.keys().any(|k| k.starts_with("sections[a]")));
    }

    #[test]
    fn plain_arrays_replace_and_unset_removes() {
        let (v, p) = run(&[
            ("builtin", json!({"brief": {"collections": ["needs"], "max_chars": 4000}})),
            ("project", json!({"brief": {"collections": ["needs", "prs"], "max_chars": "!unset"}})),
        ]);
        assert_eq!(v, json!({"brief": {"collections": ["needs", "prs"]}}));
        assert_eq!(p["brief.collections"], "project");
        assert!(!p.contains_key("brief.max_chars"));
    }

    #[test]
    fn list_edits_add_and_remove_without_restating() {
        let (v, p) = run(&[
            ("builtin", json!({"toolbar": {"actions": ["ask", "customize"]}})),
            ("user", json!({"toolbar": {"actions": {"+": ["burn", "ask"]}}})),
            ("mission", json!({"toolbar": {"actions": {"-": ["customize"]}}})),
        ]);
        assert_eq!(v["toolbar"]["actions"], json!(["ask", "burn"]));
        assert_eq!(p["toolbar.actions"], "mission");
        let mut both = json!({"toolbar": {"actions": {"+": ["a"]}}});
        compose(&mut both, &json!({"toolbar": {"actions": {"+": ["b"], "-": ["x"]}}}));
        assert_eq!(both, json!({"toolbar": {"actions": {"+": ["a", "b"], "-": ["x"]}}}));
    }

    #[test]
    fn composed_patches_merge_like_the_patches_in_order() {
        for base in [
            json!({"sections": [{"id": "a", "widget": "kpis"}, {"id": "b", "widget": "feed"}], "brief": {"max_chars": 1}}),
            json!({"sections": [{"id": "b", "widget": "feed"}, {"id": "a", "widget": "kpis"}]}),
        ] {
            let p1 = json!({"sections": [{"id": "b", "remove": true}, {"id": "c", "widget": "table", "before": "a"}], "vars": {"x": 1}});
            let p2 = json!({"sections": [{"id": "c", "title": "C"}, {"id": "b", "widget": "feed", "title": "B again"}], "brief": {"max_chars": "!unset"}});
            let (step, _) = run(&[("base", base.clone()), ("p1", p1.clone()), ("p2", p2.clone())]);
            let mut both = p1;
            compose(&mut both, &p2);
            let (once, _) = run(&[("base", base), ("both", both)]);
            assert_eq!(once, step);
        }
    }

    #[test]
    fn replacing_a_table_with_a_scalar_forgets_its_children() {
        let (_, p) = run(&[("builtin", json!({"theme": {"accent": "#000"}})), ("user", json!({"theme": "dark"}))]);
        assert_eq!(p.get("theme").map(String::as_str), Some("user"));
        assert!(!p.contains_key("theme.accent"));
    }
}
