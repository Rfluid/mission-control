//! Pages (`[[pages]]`, shown as tabs) and which sections each one holds (`page = "<id>"` on a section).
//! Overview always exists and always comes first; a section naming an unknown page shows on Overview (and
//! validation warns), so nothing configured goes missing.

use serde_json::Value;
use std::collections::BTreeSet;

/// The page every mission has, and where sections go by default.
pub const OVERVIEW: &str = "overview";

/// Page ids in tab order: Overview, then `[[pages]]` in config order.
pub fn ids(config: &Value) -> Vec<String> {
    let mut out = vec![OVERVIEW.to_string()];
    for p in config.get("pages").and_then(Value::as_array).into_iter().flatten() {
        if let Some(id) = p.get("id").and_then(Value::as_str)
            && !out.iter().any(|x| x == id)
        {
            out.push(id.to_string());
        }
    }
    out
}

/// The page a section shows on: its `page` when that page exists, else Overview.
pub fn page_of(section: &Value, known: &[String]) -> String {
    match section.get("page").and_then(Value::as_str) {
        Some(p) if known.iter().any(|k| k == p) => p.to_string(),
        _ => OVERVIEW.to_string(),
    }
}

/// The sections on `page`, in config order.
pub fn sections_on(config: &Value, page: &str) -> Vec<Value> {
    let known = ids(config);
    let all = config.get("sections").and_then(Value::as_array).into_iter().flatten();
    all.filter(|s| page_of(s, &known) == page).cloned().collect()
}

/// Every collection a section reads: the value of any `collection` key inside it (`collection`,
/// `record.collection`, `nodes.collection`, `items[].count.collection`, …), so new widgets need no list here.
pub fn collections(section: &Value, out: &mut BTreeSet<String>) {
    match section {
        Value::Object(o) => {
            for (k, v) in o {
                match (k.as_str(), v) {
                    ("collection", Value::String(c)) => {
                        out.insert(c.clone());
                    }
                    _ => collections(v, out),
                }
            }
        }
        Value::Array(a) => {
            for v in a {
                collections(v, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn overview_is_first_and_unknown_pages_fall_back_to_it() {
        let c = json!({
            "pages": [{"id": "plan"}, {"id": "overview", "title": "Home"}, {"id": "arch"}],
            "sections": [{"id": "a", "widget": "feed"}, {"id": "b", "widget": "feed", "page": "plan"},
                         {"id": "c", "widget": "feed", "page": "ghost"}],
        });
        assert_eq!(ids(&c), ["overview", "plan", "arch"]);
        let on = |p: &str| sections_on(&c, p).iter().map(|s| s["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(on("overview"), ["a", "c"]);
        assert_eq!(on("plan"), ["b"]);
        assert_eq!(on("arch"), Vec::<String>::new());
    }

    #[test]
    fn collections_are_found_anywhere_in_a_section() {
        let s = json!({"id": "p", "widget": "plan", "nodes": {"collection": "packages"}, "gates": {"collection": "gates"},
                       "items": [{"count": {"collection": "needs"}}], "record": {"collection": "run", "key": "run"}});
        let mut out = BTreeSet::new();
        collections(&s, &mut out);
        assert_eq!(out.into_iter().collect::<Vec<_>>(), ["gates", "needs", "packages", "run"]);
    }
}
