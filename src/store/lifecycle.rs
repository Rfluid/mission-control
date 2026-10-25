//! Mission lifecycle: archive, fork, and the per-mission summary the index page shows.

use super::Store;
use super::ops::log;
use super::rows::{ConfigRow, RecordRow};
use super::schema::{events, mission_config, missions, records};
use crate::model::{Mission, NewMission};
use anyhow::{Context, Result};
use diesel::prelude::*;
use serde_json::{Value, json};

impl Store {
    /// Archives (or restores) a mission. Archived missions leave the list but keep everything.
    pub fn set_archived(&mut self, id: &str, archived: bool) -> Result<Option<Mission>> {
        diesel::update(missions::table.find(id)).set(missions::archived.eq(archived)).execute(&mut self.conn)?;
        Ok(self.mission(id)?.map(|(m, _)| m))
    }

    /// A new mission from `id`: same project and config versions, its records too unless `records` is false.
    /// History (events) starts fresh with a line saying where it came from.
    pub fn fork(&mut self, id: &str, title: &str, records: bool) -> Result<Mission> {
        let (src, _) = self.mission(id)?.context("no such mission")?;
        let req = NewMission { title: title.into(), project_dir: src.project_dir.clone(), ..NewMission::default() };
        let new = self.create(&req)?;
        self.conn.transaction(|c| {
            let layers: Vec<ConfigRow> = mission_config::table
                .filter(mission_config::mission.eq(id))
                .order(mission_config::version.asc())
                .select(ConfigRow::as_select())
                .load(c)?;
            for (i, mut row) in layers.into_iter().enumerate() {
                row.mission.clone_from(&new.id);
                row.version = i64::try_from(i).unwrap_or(i64::MAX) + 1;
                diesel::insert_into(mission_config::table).values(&row).execute(c)?;
            }
            if records {
                let rows: Vec<RecordRow> =
                    records::table.filter(records::mission.eq(id)).select(RecordRow::as_select()).load(c)?;
                for mut r in rows {
                    r.mission.clone_from(&new.id);
                    diesel::insert_into(records::table).values(&r).execute(c)?;
                }
            }
            let text = format!("forked from {} ({})", src.title, src.id);
            log(c, &new.id, "log", &text, Some(&json!({"from": src.id})))?;
            anyhow::Ok(())
        })?;
        Ok(new)
    }

    /// What the index shows for a mission: open needs, records, the last event's time, and the `run/run` state.
    pub fn summary(&mut self, id: &str) -> Result<Value> {
        let open: i64 = records::table
            .filter(records::mission.eq(id).and(records::collection.eq("needs")))
            .filter(records::state.ne("answered").or(records::state.is_null()))
            .count()
            .get_result(&mut self.conn)?;
        let total: i64 = records::table.filter(records::mission.eq(id)).count().get_result(&mut self.conn)?;
        let last: Option<i64> =
            events::table.filter(events::mission.eq(id)).select(diesel::dsl::max(events::at)).first(&mut self.conn)?;
        let health: Option<Option<String>> =
            records::table.find((id, "run", "run")).select(records::state).first(&mut self.conn).optional()?;
        Ok(json!({"open_needs": open, "records": total, "last_activity": last, "health": health.flatten()}))
    }
}
