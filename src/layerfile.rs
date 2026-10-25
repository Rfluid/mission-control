//! Writing the user and project config files: a JSON patch applied with `toml_edit` (comments, order and
//! formatting kept), a one-time backup (`<file>.mission-control.bak`), and a history of earlier contents (in the state
//! dir, not the repo) that `config undo` restores from.
//!
//! Patch rules match the merge rules: tables merge key by key; arrays of tables with `id` merge by id (an
//! item with `remove = true` replaces the file's item, so the removal also reaches lower layers); other
//! values replace; `null` deletes the key from the file; `"!unset"` is written as is (a directive).

use crate::ids::now_ms;
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, TableLike};

/// `text` (a TOML document, possibly empty) with `patch` applied.
pub fn patch_toml(text: &str, patch: &Value) -> Result<String> {
    let mut doc: DocumentMut = text.parse().context("parsing the config file")?;
    let Value::Object(p) = patch else { bail!("config patch must be a table") };
    for (k, v) in p {
        set(doc.as_table_mut(), k, v, true)?;
    }
    Ok(doc.to_string())
}

/// Sets `key` in `t` to the patch value `v`. `top`: `t` is a standard table, where new tables are written as
/// `[sections]` blocks rather than inline.
fn set(t: &mut dyn TableLike, key: &str, v: &Value, top: bool) -> Result<()> {
    match v {
        Value::Null => {
            t.remove(key);
        }
        // `{"+" = [...], "-" = [...]}` on a list the file already has: edit that list in place.
        v if crate::config::list_edit(v).is_some() && t.get(key).and_then(Item::as_array).is_some() => {
            let (add, del) = crate::config::list_edit(v).context("checked")?;
            let arr = t.get_mut(key).and_then(Item::as_array_mut).context("checked")?;
            let mut keep = Array::new();
            for x in arr.iter() {
                if !del.iter().any(|d| to_value(d).is_ok_and(|d| d.to_string().trim() == x.to_string().trim())) {
                    keep.push_formatted(x.clone());
                }
            }
            for a in add {
                let a = to_value(a)?;
                if !keep.iter().any(|x| x.to_string().trim() == a.to_string().trim()) {
                    keep.push(a);
                }
            }
            *arr = keep;
        }
        Value::Object(o) => {
            if !t.get(key).is_some_and(Item::is_table_like) {
                let fresh = if top {
                    let mut table = Table::new();
                    table.set_implicit(true); // no empty `[actions]` header above `[actions.x]`
                    Item::Table(table)
                } else {
                    Item::Value(InlineTable::new().into())
                };
                t.insert(key, fresh);
            }
            // Children of a `[block]` table can be blocks too; an inline table's stay inline.
            let block = matches!(t.get(key), Some(Item::Table(_)));
            let sub = t.get_mut(key).and_then(Item::as_table_like_mut).context("just made a table")?;
            for (k, x) in o {
                set(sub, k, x, block)?;
            }
        }
        Value::Array(items) if !items.is_empty() && items.iter().all(|i| id_of(i).is_some()) => {
            by_id(t, key, items, top)?;
        }
        other => {
            let mut new = to_value(other)?;
            // Keep the comment and spacing around the old value (`repo = "a/b" # the main repo`).
            if let Some(old) = t.get(key).and_then(Item::as_value) {
                *new.decor_mut() = old.decor().clone();
            }
            t.insert(key, Item::Value(new));
        }
    }
    Ok(())
}

fn id_of(v: &Value) -> Option<&str> {
    v.get("id")?.as_str()
}

/// Merges id'd items into an array of tables (`[[sections]]`) or an inline array of tables (`columns = [...]`).
fn by_id(t: &mut dyn TableLike, key: &str, items: &[Value], top: bool) -> Result<()> {
    if t.get(key).is_none() {
        let fresh = if top { Item::ArrayOfTables(ArrayOfTables::new()) } else { Item::Value(Array::new().into()) };
        t.insert(key, fresh);
    }
    let slot = t.get_mut(key).context("just inserted")?;
    match slot {
        Item::ArrayOfTables(aot) => {
            for item in items {
                let id = id_of(item).context("checked")?;
                let found = aot.iter_mut().position(|x| x.get("id").and_then(|v| v.as_str()) == Some(id));
                let removing = item.get("remove").and_then(Value::as_bool) == Some(true);
                let i = match found {
                    Some(i) if !removing => i,
                    Some(i) => {
                        *aot.get_mut(i).context("index")? = Table::new();
                        i
                    }
                    None => {
                        aot.push(Table::new());
                        aot.len() - 1
                    }
                };
                let table = aot.get_mut(i).context("index")?;
                for (k, v) in item.as_object().context("checked")? {
                    set(table, k, v, false)?;
                }
            }
        }
        Item::Value(toml_edit::Value::Array(arr)) => {
            for item in items {
                let id = id_of(item).context("checked")?;
                let found = arr
                    .iter()
                    .position(|x| x.as_inline_table().and_then(|t| t.get("id")).and_then(|v| v.as_str()) == Some(id));
                let removing = item.get("remove").and_then(Value::as_bool) == Some(true);
                let mut table = match found {
                    Some(i) if !removing => arr.get(i).and_then(|v| v.as_inline_table()).cloned().unwrap_or_default(),
                    _ => InlineTable::new(),
                };
                for (k, v) in item.as_object().context("checked")? {
                    set(&mut table, k, v, false)?;
                }
                match found {
                    Some(i) => {
                        arr.replace(i, table);
                    }
                    None => arr.push(table),
                }
            }
            arr.fmt();
        }
        _ => bail!("`{key}` in the file isn't a list of tables; can't patch it by id"),
    }
    Ok(())
}

fn to_value(v: &Value) -> Result<toml_edit::Value> {
    Ok(match v {
        Value::Bool(b) => (*b).into(),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.into(),
            (None, Some(f)) => f.into(),
            _ => bail!("number out of range: {n}"),
        },
        Value::String(s) => s.as_str().into(),
        Value::Array(a) => {
            let mut arr = Array::new();
            for x in a {
                arr.push(to_value(x)?);
            }
            arr.into()
        }
        Value::Object(o) => {
            let mut t = InlineTable::new();
            for (k, x) in o {
                if !x.is_null() {
                    t.insert(k, to_value(x)?);
                }
            }
            t.into()
        }
        Value::Null => bail!("null can't be stored in TOML"),
    })
}

/// A config file and where its history lives.
pub struct LayerFile {
    pub path: PathBuf,
    history: PathBuf,
}

/// One saved earlier version of a file.
#[derive(Debug, serde::Serialize)]
pub struct Snapshot {
    pub at: i64,
    pub note: Option<String>,
}

impl LayerFile {
    /// `path` is the config file; history goes under `state_dir/history/` in a folder named after the path.
    pub fn new(path: PathBuf, state_dir: &Path) -> Self {
        let name: String =
            path.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        Self { history: state_dir.join("history").join(name), path }
    }

    pub fn read(&self) -> Result<String> {
        match fs::read_to_string(&self.path) {
            Ok(t) => Ok(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(e).with_context(|| format!("reading {}", self.path.display())),
        }
    }

    /// Writes `text` if it differs: backs the original up once, and keeps the old content for undo.
    /// Returns whether anything was written.
    pub fn write(&self, text: &str, note: Option<&str>) -> Result<bool> {
        let old = self.read()?;
        if old == text {
            return Ok(false);
        }
        let bak = PathBuf::from(format!("{}.mission-control.bak", self.path.display()));
        // Only the user's original: before mission's first edit (no history yet), and only if there was a file.
        if self.path.exists() && !bak.exists() && self.history().is_empty() {
            fs::copy(&self.path, &bak).with_context(|| format!("backing up {}", self.path.display()))?;
        }
        fs::create_dir_all(&self.history)?;
        // Unique and ordered even for two writes in one millisecond.
        let mut stamp = now_ms();
        while self.history.join(format!("{stamp}.toml")).exists() {
            stamp += 1;
        }
        fs::write(self.history.join(format!("{stamp}.toml")), &old)?;
        fs::write(self.history.join(format!("{stamp}.note")), note.unwrap_or_default())?;
        fs::create_dir_all(self.path.parent().context("config file has no parent dir")?)?;
        fs::write(&self.path, text).with_context(|| format!("writing {}", self.path.display()))?;
        Ok(true)
    }

    /// Earlier versions, newest first.
    pub fn history(&self) -> Vec<Snapshot> {
        let mut out: Vec<Snapshot> = fs::read_dir(&self.history)
            .into_iter()
            .flatten()
            .filter_map(|e| {
                let p = e.ok()?.path();
                (p.extension()? == "toml").then_some(())?;
                let at: i64 = p.file_stem()?.to_str()?.parse().ok()?;
                let note = fs::read_to_string(p.with_extension("note")).ok().filter(|n| !n.is_empty());
                Some(Snapshot { at, note })
            })
            .collect();
        out.sort_by_key(|s| std::cmp::Reverse(s.at));
        out
    }

    /// Restores the newest earlier version and drops it from the history. `None` if there's nothing to undo.
    pub fn undo(&self) -> Result<Option<Snapshot>> {
        let Some(last) = self.history().into_iter().next() else { return Ok(None) };
        let snap = self.history.join(format!("{}.toml", last.at));
        let text = fs::read_to_string(&snap)?;
        if text.is_empty() && !self.path.exists() {
            // nothing to restore to
        } else if text.is_empty() {
            fs::remove_file(&self.path)?;
        } else {
            fs::write(&self.path, text)?;
        }
        fs::remove_file(snap)?;
        let _ = fs::remove_file(self.history.join(format!("{}.note", last.at)));
        Ok(Some(last))
    }
}

#[cfg(test)]
mod tests;
