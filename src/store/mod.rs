//! `SQLite` persistence via Diesel: missions, records (every collection), events, mission-control config versions.

mod cursors;
mod lifecycle;
mod ops;
mod rows;
mod schema;

pub use ops::Applied;

use crate::ids::{new_id, new_token, now_ms};
use crate::model::{Event, Mission, NewMission, Record};
use anyhow::{Context, Result, anyhow, bail};
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use rows::{ConfigRow, EventRow, MissionRow, RecordRow};
use schema::{events, mission_config, missions, records};
use serde_json::{Value, json};
use std::path::Path;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub struct Store {
    conn: SqliteConnection,
}

impl Store {
    /// Opens (or creates) the database at `path` and runs pending migrations. `":memory:"` for tests.
    pub fn open(path: &Path) -> Result<Self> {
        let url = path.to_str().context("non-UTF-8 state path")?;
        let mut conn = SqliteConnection::establish(url)?;
        conn.batch_execute("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.run_pending_migrations(MIGRATIONS).map_err(|e| anyhow!("migrations: {e}"))?;
        Ok(Self { conn })
    }

    /// Creates a mission; a `template` becomes version 1 of its mission-control config layer.
    pub fn create(&mut self, req: &NewMission) -> Result<Mission> {
        let row = MissionRow {
            id: new_id("msn"),
            title: req.title.clone(),
            session: req.session.clone(),
            project_dir: req.project_dir.clone(),
            token: new_token(),
            created: now_ms(),
            archived: false,
        };
        diesel::insert_into(missions::table).values(&row).execute(&mut self.conn)?;
        if let Some(t) = &req.template {
            self.patch_layer(&row.id, &json!({"mission": {"template": t}}), Some("created"))?;
        }
        Ok(row.into())
    }

    pub fn missions(&mut self, include_archived: bool) -> Result<Vec<Mission>> {
        let mut q = missions::table.order(missions::created.desc()).into_boxed();
        if !include_archived {
            q = q.filter(missions::archived.eq(false));
        }
        Ok(q.select(MissionRow::as_select()).load(&mut self.conn)?.into_iter().map(Into::into).collect())
    }

    /// The mission and its browser token.
    pub fn mission(&mut self, id: &str) -> Result<Option<(Mission, String)>> {
        let row = missions::table.find(id).select(MissionRow::as_select()).first(&mut self.conn).optional()?;
        Ok(row.map(|r| {
            let token = r.token.clone();
            (r.into(), token)
        }))
    }

    /// A live (not archived) mission of `session` titled `title`, if any: `new` refuses a duplicate.
    pub fn find_titled(&mut self, session: &str, title: &str) -> Result<Option<Mission>> {
        let row = missions::table
            .filter(missions::session.eq(session).and(missions::title.eq(title)).and(missions::archived.eq(false)))
            .select(MissionRow::as_select())
            .first(&mut self.conn)
            .optional()?;
        Ok(row.map(Into::into))
    }

    /// Binds a mission to `session` (a restarted agent session re-attaching). Records and history stay.
    pub fn attach(&mut self, id: &str, session: Option<&str>) -> Result<Option<Mission>> {
        diesel::update(missions::table.find(id)).set(missions::session.eq(session)).execute(&mut self.conn)?;
        Ok(self.mission(id)?.map(|(m, _)| m))
    }

    pub fn records(&mut self, mission: &str) -> Result<Vec<Record>> {
        let rows = records::table
            .filter(records::mission.eq(mission))
            .order((records::collection, records::updated))
            .select(RecordRow::as_select())
            .load(&mut self.conn)?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// Newest `limit` events after `since`, oldest first.
    pub fn events(&mut self, mission: &str, since: i64, limit: i64) -> Result<Vec<Event>> {
        let mut rows = events::table
            .filter(events::mission.eq(mission).and(events::seq.gt(since)))
            .order(events::seq.desc())
            .limit(limit)
            .select(EventRow::as_select())
            .load(&mut self.conn)?;
        rows.reverse();
        rows.into_iter().map(TryInto::try_into).collect()
    }

    pub fn event_count(&mut self, mission: &str) -> Result<i64> {
        Ok(events::table.filter(events::mission.eq(mission)).count().get_result(&mut self.conn)?)
    }

    /// The mission-control config layer: every version's patch, oldest first. Resolution applies them in order, so a
    /// patch can `remove` or reposition items that lower layers define.
    pub fn layers(&mut self, mission: &str) -> Result<Vec<(i64, Value)>> {
        let rows = mission_config::table
            .filter(mission_config::mission.eq(mission))
            .order(mission_config::version.asc())
            .select(ConfigRow::as_select())
            .load(&mut self.conn)?;
        rows.into_iter().map(|r| Ok((r.version, serde_json::from_str(&r.doc)?))).collect()
    }

    /// Folds the oldest versions into one when there are more than `keep`: their patches are composed (directives
    /// kept), so the resolved config doesn't change; only undo can't step back between them any more.
    pub fn compact_layers(&mut self, mission: &str, keep: usize) -> Result<usize> {
        let layers = self.layers(mission)?;
        if keep < 1 || layers.len() <= keep {
            return Ok(0);
        }
        let fold = layers.len() - keep + 1;
        let mut doc = json!({});
        for (_, patch) in &layers[..fold] {
            crate::config::compose(&mut doc, patch);
        }
        let (first, last) = (layers[0].0, layers[fold - 1].0);
        self.conn.transaction(|c| {
            diesel::delete(
                mission_config::table.filter(
                    mission_config::mission
                        .eq(mission)
                        .and(mission_config::version.gt(first))
                        .and(mission_config::version.le(last)),
                ),
            )
            .execute(c)?;
            diesel::update(mission_config::table.find((mission, first)))
                .set((
                    mission_config::doc.eq(doc.to_string()),
                    mission_config::note.eq(format!("v{first}–v{last}, compacted")),
                ))
                .execute(c)?;
            anyhow::Ok(())
        })?;
        Ok(fold - 1)
    }

    /// Drops the newest mission-control config version. Returns `(version, patch, note)`, or `None` if there are none.
    pub fn undo_layer(&mut self, mission: &str) -> Result<Option<(i64, Value, Option<String>)>> {
        let last = mission_config::table
            .filter(mission_config::mission.eq(mission))
            .order(mission_config::version.desc())
            .select(ConfigRow::as_select())
            .first(&mut self.conn)
            .optional()?;
        let Some(row) = last else { return Ok(None) };
        diesel::delete(mission_config::table.find((mission, row.version))).execute(&mut self.conn)?;
        Ok(Some((row.version, serde_json::from_str(&row.doc)?, row.note)))
    }

    /// Every mission-control config version, oldest first: `{version, note, at, patch}`.
    pub fn layer_history(&mut self, mission: &str) -> Result<Vec<Value>> {
        let rows = mission_config::table
            .filter(mission_config::mission.eq(mission))
            .order(mission_config::version.asc())
            .select(ConfigRow::as_select())
            .load(&mut self.conn)?;
        rows.into_iter()
            .map(|r| {
                let patch: Value = serde_json::from_str(&r.doc)?;
                Ok(json!({"version": r.version, "note": r.note, "at": r.at, "patch": patch}))
            })
            .collect()
    }

    /// Adds `patch` to the mission layer as the next version, and logs it. Returns the version and the log event.
    pub fn patch_layer(&mut self, mission: &str, patch: &Value, note: Option<&str>) -> Result<(i64, Event)> {
        if !patch.is_object() {
            bail!("config patch must be a table");
        }
        let version = self.layers(mission)?.last().map_or(0, |(v, _)| *v);
        let row = ConfigRow {
            mission: mission.into(),
            version: version + 1,
            doc: patch.to_string(),
            note: note.map(String::from),
            at: now_ms(),
        };
        diesel::insert_into(mission_config::table).values(&row).execute(&mut self.conn)?;
        let text = format!("config v{}{}", row.version, note.map(|n| format!(": {n}")).unwrap_or_default());
        let ev = ops::log(&mut self.conn, mission, "config", &text, Some(patch))?;
        Ok((row.version, ev))
    }
}

#[cfg(test)]
mod tests;
