//! Key paths into a resolved config, in the same syntax provenance uses: `mission.template`,
//! `sections[prs].columns[state].as` (brackets pick an item of an id'd array by its id).

use serde_json::Value;

/// The value at `path` (empty: the whole config), or `None` if any step is missing.
pub fn at<'a>(config: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = config;
    for part in path.split('.').filter(|p| !p.is_empty()) {
        let (key, mut ids) = match part.find('[') {
            Some(i) => (&part[..i], &part[i..]),
            None => (part, ""),
        };
        if !key.is_empty() {
            cur = cur.get(key)?;
        }
        while let Some(rest) = ids.strip_prefix('[') {
            let end = rest.find(']')?;
            let id = &rest[..end];
            cur = cur.as_array()?.iter().find(|v| v.get("id").and_then(Value::as_str) == Some(id))?;
            ids = &rest[end + 1..];
        }
        if !ids.is_empty() {
            return None;
        }
    }
    Some(cur)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paths_follow_keys_and_ids() {
        let c = json!({"mission": {"template": "x"}, "sections": [{"id": "prs", "columns": [{"id": "state", "as": "pill"}]}]});
        assert_eq!(at(&c, "mission.template"), Some(&json!("x")));
        assert_eq!(at(&c, "sections[prs].columns[state].as"), Some(&json!("pill")));
        assert_eq!(at(&c, ""), Some(&c));
        assert_eq!(at(&c, "sections[nope]"), None);
        assert_eq!(at(&c, "sections[prs"), None);
        assert_eq!(at(&c, "mission.template.deeper"), None);
    }
}
