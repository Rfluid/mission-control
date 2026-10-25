//! Resolved configs, cached: a config is re-resolved only when one of its files changes (size or mtime of
//! `config.toml` and `templates/*.toml` in the user and project dirs) or its mission layer is written.

use crate::config::{Resolved, Stack};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// One config file's identity for change detection: path, mtime, size. A missing file counts too, so
/// creating it is a change.
type Stamp = (PathBuf, Option<SystemTime>, u64);
/// A cached value and the file stamps it was built from.
type Entry<T> = (Vec<Stamp>, Arc<T>);

#[derive(Default)]
pub(super) struct Cache {
    /// By project dir.
    stacks: Mutex<HashMap<Option<String>, Entry<Stack>>>,
    /// By mission id.
    resolved: Mutex<HashMap<String, Entry<Resolved>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What the user and project layers are read from, as it is on disk now.
pub(super) fn signature(user: Option<&Path>, project: Option<&str>) -> Vec<Stamp> {
    let dirs = [user.map(Path::to_path_buf), project.map(|p| Path::new(p).join(".mission-control"))];
    let mut out = Vec::new();
    for dir in dirs.into_iter().flatten() {
        let mut files = vec![dir.join("config.toml")];
        if let Ok(rd) = std::fs::read_dir(dir.join("templates")) {
            let mut t: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
            t.sort();
            files.extend(t);
        }
        for f in files {
            let meta = std::fs::metadata(&f).ok();
            out.push((f, meta.as_ref().and_then(|m| m.modified().ok()), meta.map_or(0, |m| m.len())));
        }
    }
    out
}

impl Cache {
    pub(super) fn stack(
        &self,
        project: Option<&str>,
        sig: &[Stamp],
        load: impl FnOnce() -> anyhow::Result<Stack>,
    ) -> anyhow::Result<Arc<Stack>> {
        let key = project.map(String::from);
        if let Some((s, st)) = lock(&self.stacks).get(&key)
            && s == sig
        {
            return Ok(st.clone());
        }
        let st = Arc::new(load()?);
        lock(&self.stacks).insert(key, (sig.to_vec(), st.clone()));
        Ok(st)
    }

    pub(super) fn resolved(&self, mission: &str, sig: &[Stamp]) -> Option<Arc<Resolved>> {
        lock(&self.resolved).get(mission).filter(|(s, _)| s == sig).map(|(_, r)| r.clone())
    }

    pub(super) fn keep(&self, mission: &str, sig: Vec<Stamp>, r: Arc<Resolved>) {
        lock(&self.resolved).insert(mission.into(), (sig, r));
    }

    /// The mission layer of `mission-control` changed.
    pub(super) fn forget(&self, mission: &str) {
        lock(&self.resolved).remove(mission);
    }
}
