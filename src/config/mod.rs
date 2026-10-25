//! Layered configuration: builtin → template → user → project → mission (see `docs/CONFIG.md`).

mod builtin;
mod merge;
pub mod pages;
pub mod path;
pub mod schema;
mod validate;
pub mod widgets;

pub use merge::{Provenance, UNSET, compose, list_edit, merge};
pub use validate::{Issue, Severity, validate};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Template file keys that describe the template instead of configuring the page.
const TEMPLATE_META: [&str; 4] = ["name", "description", "extends", "replace"];
/// Longest `extends` chain followed before giving up (also catches cycles).
const MAX_EXTENDS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerKind {
    Builtin,
    Template,
    User,
    Project,
    Mission,
}

/// One config document and where it came from.
#[derive(Clone, Debug)]
pub struct Layer {
    pub kind: LayerKind,
    /// Shown in provenance: `user (~/.config/mission-control/config.toml)`.
    pub label: String,
    pub doc: Value,
}

impl Layer {
    pub fn new(kind: LayerKind, label: impl Into<String>, doc: Value) -> Self {
        Self { kind, label: label.into(), doc }
    }
}

/// A template file, from one of the layers that can hold templates.
#[derive(Clone, Debug)]
pub struct TemplateFile {
    pub from: LayerKind,
    pub label: String,
    pub doc: Value,
}

/// Everything but the mission layer: what the daemon loads per project.
#[derive(Clone, Debug, Default)]
pub struct Stack {
    pub layers: Vec<Layer>,
    /// By template name, lowest layer first.
    pub templates: BTreeMap<String, Vec<TemplateFile>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Resolved {
    pub config: Value,
    pub provenance: Provenance,
    pub template: String,
    pub issues: Vec<Issue>,
}

impl Stack {
    /// Built-in layer and templates, plus the user's and (if `project` is set) the project's.
    pub fn load(user_dir: Option<&Path>, project: Option<&Path>) -> Result<Self> {
        let mut s = Self::default();
        s.layers.push(Layer::new(LayerKind::Builtin, "builtin", parse(builtin::CONFIG, "builtin config.toml")?));
        for (name, text) in builtin::TEMPLATES {
            s.add_template(name, LayerKind::Builtin, format!("builtin templates/{name}.toml"), parse(text, name)?);
        }
        if let Some(dir) = user_dir {
            s.load_dir(LayerKind::User, dir)?;
        }
        if let Some(root) = project {
            s.load_dir(LayerKind::Project, &root.join(".mission-control"))?;
        }
        Ok(s)
    }

    fn load_dir(&mut self, kind: LayerKind, dir: &Path) -> Result<()> {
        let who = if kind == LayerKind::User { "user" } else { "project" };
        let file = dir.join("config.toml");
        if file.is_file() {
            let doc = read(&file)?;
            self.layers.push(Layer::new(kind, format!("{who} ({})", file.display()), doc));
        }
        for path in toml_files(&dir.join("templates")) {
            let name = path.file_stem().and_then(|s| s.to_str()).context("template file name")?.to_string();
            let doc = read(&path)?;
            self.add_template(&name, kind, format!("{who} template ({})", path.display()), doc);
        }
        Ok(())
    }

    fn add_template(&mut self, name: &str, from: LayerKind, label: String, doc: Value) {
        self.templates.entry(name.to_string()).or_default().push(TemplateFile { from, label, doc });
    }

    /// Replaces the layer of `l.kind` (or adds it, in layer order): previews a file edit before writing it.
    pub fn set_layer(&mut self, l: Layer) {
        if let Some(i) = self.layers.iter().position(|x| x.kind == l.kind) {
            self.layers[i] = l;
        } else {
            let at = self.layers.iter().position(|x| x.kind > l.kind).unwrap_or(self.layers.len());
            self.layers.insert(at, l);
        }
    }

    /// Replaces template `name`'s file from layer `from` (or adds it, in layer order): checks a template before
    /// writing it.
    pub fn set_template(&mut self, name: &str, from: LayerKind, label: String, doc: Value) {
        let files = self.templates.entry(name.to_string()).or_default();
        let file = TemplateFile { from, label, doc };
        if let Some(i) = files.iter().position(|f| f.from == from) {
            files[i] = file;
        } else {
            let at = files.iter().position(|f| f.from > from).unwrap_or(files.len());
            files.insert(at, file);
        }
    }

    pub fn template_names(&self) -> Vec<&str> {
        self.templates.keys().map(String::as_str).collect()
    }

    /// Every template: its description and the files (layers) that define it, lowest first.
    pub fn summary(&self) -> Value {
        let out: serde_json::Map<String, Value> = (self.templates.iter())
            .map(|(name, files)| {
                let desc = files.iter().rev().find_map(|f| f.doc.get("description").cloned()).unwrap_or(Value::Null);
                let extends = files.iter().rev().find_map(|f| f.doc.get("extends").cloned()).unwrap_or(Value::Null);
                let from: Vec<&str> = files.iter().map(|f| f.label.as_str()).collect();
                (name.clone(), serde_json::json!({"description": desc, "extends": extends, "files": from}))
            })
            .collect();
        Value::Object(out)
    }

    /// Merges the stack with the `mission-control` patches on top, in order. Two passes: the first only finds
    /// `mission.template`.
    pub fn resolve(&self, mission: &[Layer]) -> Result<Resolved> {
        let mut plain: Vec<&Layer> = self.layers.iter().collect();
        plain.extend(mission);
        let (pre, _) = merge_all(plain.iter().copied().map(|l| (&l.doc, l.label.as_str())));
        let template = pre.pointer("/mission/template").and_then(Value::as_str).unwrap_or("minimal").to_string();
        let chain = self.template_chain(&template)?;

        let builtin = &self.layers[0];
        let mut docs: Vec<(&Value, &str)> = vec![(&builtin.doc, builtin.label.as_str())];
        docs.extend(chain.iter().map(|(doc, label)| (doc, label.as_str())));
        docs.extend(plain[1..].iter().map(|l| (&l.doc, l.label.as_str())));
        let (mut config, mut provenance) = merge_all(docs.into_iter());
        drop_unmatched_sections(&mut config, &mut provenance);

        let mut all: Vec<Layer> = plain.into_iter().cloned().collect();
        all.extend(chain.into_iter().map(|(doc, label)| Layer::new(LayerKind::Template, label, doc)));
        let issues = validate(&config, &all);
        Ok(Resolved { config, provenance, template, issues })
    }

    /// Template docs to apply, parent first, each with its metadata keys stripped.
    fn template_chain(&self, name: &str) -> Result<Vec<(Value, String)>> {
        let mut names = vec![name.to_string()];
        loop {
            if names.len() > MAX_EXTENDS {
                bail!("template `{name}`: extends chain longer than {MAX_EXTENDS} (cycle?)");
            }
            let files = self.files_of(names.last().expect("non-empty"))?;
            let parent = files.iter().rev().find_map(|f| f.doc.get("extends")?.as_str().map(String::from));
            match parent {
                Some(p) if !p.is_empty() => names.push(p),
                _ => break,
            }
        }
        let mut out = Vec::new();
        for n in names.iter().rev() {
            for f in self.files_of(n)? {
                let mut doc = f.doc.clone();
                if let Some(o) = doc.as_object_mut() {
                    for k in TEMPLATE_META {
                        o.remove(k);
                    }
                }
                out.push((doc, f.label.clone()));
            }
        }
        Ok(out)
    }

    /// The files of template `name` that apply: from the last one marked `replace = true`, up.
    fn files_of(&self, name: &str) -> Result<&[TemplateFile]> {
        let files = self
            .templates
            .get(name)
            .with_context(|| format!("unknown template `{name}` (have: {})", self.template_names().join(", ")))?;
        let start = files.iter().rposition(|f| f.doc.get("replace").and_then(Value::as_bool) == Some(true));
        Ok(&files[start.unwrap_or(0)..])
    }
}

/// A section with no `widget` after the merge is a patch for a section this page doesn't have (a user's
/// "buttons on the PR table" with a template that has none): it is dropped, not rendered broken.
fn drop_unmatched_sections(config: &mut Value, prov: &mut Provenance) {
    let Some(sections) = config.get_mut("sections").and_then(Value::as_array_mut) else { return };
    let gone: Vec<String> = sections
        .iter()
        .filter(|s| s.get("widget").is_none())
        .filter_map(|s| s.get("id").and_then(Value::as_str).map(String::from))
        .collect();
    sections.retain(|s| s.get("widget").is_some());
    for id in gone {
        let p = format!("sections[{id}]");
        prov.retain(|k, _| !(k == &p || k.starts_with(&format!("{p}."))));
    }
}

fn merge_all<'a>(docs: impl Iterator<Item = (&'a Value, &'a str)>) -> (Value, Provenance) {
    let (mut config, mut prov) = (Value::Object(serde_json::Map::new()), Provenance::new());
    for (doc, label) in docs {
        merge(&mut config, doc, label, &mut prov);
    }
    (config, prov)
}

pub fn parse(text: &str, what: &str) -> Result<Value> {
    toml::from_str(text).with_context(|| format!("parsing {what}"))
}

fn read(path: &Path) -> Result<Value> {
    parse(
        &fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
        &path.display().to_string(),
    )
}

fn toml_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests;
