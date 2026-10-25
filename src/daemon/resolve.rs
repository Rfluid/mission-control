//! Resolving a mission's config (cached), with an optional overlay: an unsaved change to check, or a dry-run
//! draft the page previews (`?draft=`).

use super::App;
use super::cache::signature;
use super::config::Target;
use super::handlers::{Res, internal};
use crate::config::{Layer, LayerKind, Resolved, Stack};
use crate::ids::new_id;
use crate::model::Mission;
use crate::store::Applied;
use axum::http::StatusCode;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// How long a dry-run draft stays previewable.
const DRAFT_TTL: Duration = Duration::from_secs(3600);

/// A config change applied on top of what's stored, without storing it (checks and previews).
#[derive(Clone, Debug)]
pub enum Overlay {
    None,
    /// One more mission-layer patch.
    Mission(Value),
    /// A user or project file's new contents.
    File(LayerKind, String, Value),
}

pub(super) struct Draft {
    mission: String,
    overlay: Overlay,
    until: std::time::Instant,
}

impl App {
    /// A live draft of `mission-control`, as an overlay.
    pub(super) fn draft(&self, mission: &str, id: &str) -> Option<Overlay> {
        let mut d = self.drafts.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = std::time::Instant::now();
        d.retain(|_, x| x.until > now);
        d.get(id).filter(|x| x.mission == mission).map(|x| x.overlay.clone())
    }

    pub(super) fn save_draft(&self, mission: &str, overlay: Overlay) -> String {
        let id = new_id("drf");
        let until = std::time::Instant::now() + DRAFT_TTL;
        let mut d = self.drafts.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        d.insert(id.clone(), Draft { mission: mission.into(), overlay, until });
        id
    }

    /// Tells every open page whose config may have changed: all missions for the user layer, the project's
    /// for a project file.
    pub(super) fn config_changed(&self, target: Target, mission: &Mission) {
        let all = self.store().missions(true).unwrap_or_default();
        for m in all {
            let hit = match target {
                Target::Mission => m.id == mission.id,
                Target::User => true,
                Target::Project => m.project_dir.is_some() && m.project_dir == mission.project_dir,
            };
            if hit && m.id != mission.id {
                self.publish(&m.id, Applied::default(), true);
            }
        }
    }
}

/// The config stack (builtin, user, project) for `project_dir`, cached until a file changes.
pub(super) fn stack(app: &App, project_dir: Option<&str>) -> anyhow::Result<Arc<Stack>> {
    let sig = signature(app.user_dir.as_deref(), project_dir);
    app.cache
        .stack(project_dir, &sig, || Stack::load(app.user_dir.as_deref(), project_dir.map(PathBuf::from).as_deref()))
}

/// The mission's resolved config, with `over` applied on top (to check or preview a change). Without an
/// overlay it comes from the cache when nothing changed.
pub(super) fn resolve(app: &App, m: &Mission, over: &Overlay) -> Res<Arc<Resolved>> {
    let sig = signature(app.user_dir.as_deref(), m.project_dir.as_deref());
    if matches!(over, Overlay::None)
        && let Some(r) = app.cache.resolved(&m.id, &sig)
    {
        return Ok(r);
    }
    let fail = |e: anyhow::Error| (StatusCode::BAD_REQUEST, format!("{e:#}"));
    let mut layers: Vec<Layer> = (app.store().layers(&m.id).map_err(internal)?.into_iter())
        .map(|(v, doc)| Layer::new(LayerKind::Mission, format!("mission v{v}"), doc))
        .collect();
    let base = stack(app, m.project_dir.as_deref()).map_err(fail)?;
    let r = match over {
        Overlay::None => {
            let r = Arc::new(base.resolve(&layers).map_err(fail)?);
            app.cache.keep(&m.id, sig, r.clone());
            return Ok(r);
        }
        Overlay::Mission(p) => {
            layers.push(Layer::new(LayerKind::Mission, "mission (pending)", p.clone()));
            base.resolve(&layers)
        }
        Overlay::File(kind, label, doc) => {
            let mut s = (*base).clone();
            s.set_layer(Layer::new(*kind, label.clone(), doc.clone()));
            s.resolve(&layers)
        }
    };
    Ok(Arc::new(r.map_err(fail)?))
}

/// Defaults when `[retention]` doesn't say.
const EVENTS_KEEP: i64 = 2000;
const CONFIG_KEEP: usize = 50;

impl App {
    /// Applies `[retention]` to a mission after a write: old events past `events_keep` go, and mission-control config
    /// versions past `config_keep` fold into one. Best effort: retention never fails a write.
    pub(super) fn retain(&self, m: &Mission) {
        let Ok(r) = resolve(self, m, &Overlay::None) else { return };
        let get = |k: &str| r.config.pointer(&format!("/retention/{k}")).and_then(Value::as_i64);
        let keep_events = get("events_keep").unwrap_or(EVENTS_KEEP).max(1);
        let keep_versions = get("config_keep").and_then(|n| usize::try_from(n).ok()).unwrap_or(CONFIG_KEEP);
        let mut s = self.store();
        if let Err(e) = s.prune_events(&m.id, keep_events) {
            tracing::warn!(mission = %m.id, error = %e, "pruning events");
        }
        match s.compact_layers(&m.id, keep_versions) {
            Ok(n) if n > 0 => {
                drop(s);
                self.cache.forget(&m.id);
            }
            Err(e) => tracing::warn!(mission = %m.id, error = %e, "compacting config versions"),
            _ => {}
        }
    }
}
