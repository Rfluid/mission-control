//! The redline embed (`[mission] review = true`): when the mission has a redline artifact (record
//! `_review/redline`, written by `mission-control review`), the page loads redline's overlay so the user can comment
//! anywhere on it. The script base must be a loopback redline daemon; anything else is ignored.

use crate::model::Record;
use serde_json::Value;

/// The record `mission-control review` writes.
pub const COLLECTION: &str = "_review";
pub const KEY: &str = "redline";

/// `http://127.0.0.1:<port>` and nothing else.
fn loopback(base: &str) -> bool {
    base.strip_prefix("http://127.0.0.1:")
        .is_some_and(|p| !p.is_empty() && p.len() <= 5 && p.chars().all(|c| c.is_ascii_digit()))
}

fn plain(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The redline snippet for the page, if review is on and the stored artifact checks out.
pub fn snippet(config: &Value, records: &[Record]) -> Option<String> {
    if config.pointer("/mission/review").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let r = records.iter().find(|r| r.collection == COLLECTION && r.key == KEY)?;
    let get = |k: &str| r.body.get(k).and_then(Value::as_str);
    let (base, artifact, token) = (get("base")?, get("artifact")?, get("token")?);
    if !loopback(base) || !plain(artifact) || !plain(token) {
        return None;
    }
    Some(format!(
        "<script src=\"{base}/client.js\" data-redline-artifact=\"{artifact}\" data-redline-token=\"{token}\"></script>\n<script src=\"{base}/overlay.js\"></script>\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rec(body: Value) -> Record {
        Record { collection: COLLECTION.into(), key: KEY.into(), state: None, body, updated: 0 }
    }

    #[test]
    fn only_a_loopback_redline_is_embedded_and_only_when_review_is_on() {
        let on = json!({"mission": {"review": true}});
        let good = rec(json!({"base": "http://127.0.0.1:7465", "artifact": "art_1", "token": "abc123"}));
        assert!(
            snippet(&on, std::slice::from_ref(&good)).unwrap().contains(r#"src="http://127.0.0.1:7465/overlay.js""#)
        );
        assert!(snippet(&json!({}), &[good]).is_none(), "review off");
        for bad in [
            json!({"base": "https://evil.example", "artifact": "a", "token": "t"}),
            json!({"base": "http://127.0.0.1:7465/x", "artifact": "a", "token": "t"}),
            json!({"base": "http://127.0.0.1:7465", "artifact": "a\" onload=\"x", "token": "t"}),
        ] {
            assert!(snippet(&on, &[rec(bad.clone())]).is_none(), "{bad}");
        }
    }
}
