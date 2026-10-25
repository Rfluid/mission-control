//! `wait` cursors: per mission and agent session, the last page event the session was handed.

use super::Store;
use super::rows::EventRow;
use super::schema::{cursors, events};
use crate::ids::now_ms;
use crate::model::{Event, Record};
use anyhow::Result;
use diesel::prelude::*;

impl Store {
    /// Where `session`'s wait on `mission-control` continues. A session new to the mission continues where the
    /// furthest other session stopped (a restarted agent re-attaching), else from the start.
    pub fn cursor(&mut self, mission: &str, session: &str) -> Result<i64> {
        let own: Option<i64> =
            cursors::table.find((mission, session)).select(cursors::seq).first(&mut self.conn).optional()?;
        if let Some(seq) = own {
            return Ok(seq);
        }
        let any: Option<i64> = cursors::table
            .filter(cursors::mission.eq(mission))
            .select(diesel::dsl::max(cursors::seq))
            .first(&mut self.conn)?;
        Ok(any.unwrap_or(0))
    }

    pub fn set_cursor(&mut self, mission: &str, session: &str, seq: i64) -> Result<()> {
        diesel::replace_into(cursors::table)
            .values((
                cursors::mission.eq(mission),
                cursors::session.eq(session),
                cursors::seq.eq(seq),
                cursors::updated.eq(now_ms()),
            ))
            .execute(&mut self.conn)?;
        Ok(())
    }

    /// Events of `kinds` after `since`, oldest first, at most `limit`.
    pub fn events_of(&mut self, mission: &str, since: i64, kinds: &[&str], limit: i64) -> Result<Vec<Event>> {
        let rows = events::table
            .filter(events::mission.eq(mission).and(events::seq.gt(since)).and(events::kind.eq_any(kinds)))
            .order(events::seq.asc())
            .limit(limit)
            .select(EventRow::as_select())
            .load(&mut self.conn)?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// One record, if it exists.
    pub fn record(&mut self, mission: &str, collection: &str, key: &str) -> Result<Option<Record>> {
        super::ops::record(&mut self.conn, mission, collection, key)
    }

    /// Events older than `before` (all when `None`), newest `limit` of them, oldest first: feed paging.
    pub fn events_before(&mut self, mission: &str, before: Option<i64>, limit: i64) -> Result<Vec<Event>> {
        let mut q = events::table.filter(events::mission.eq(mission)).into_boxed();
        if let Some(b) = before {
            q = q.filter(events::seq.lt(b));
        }
        let mut rows = q.order(events::seq.desc()).limit(limit).select(EventRow::as_select()).load(&mut self.conn)?;
        rows.reverse();
        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// Events about one record, newest `limit`, oldest first: its state changes (`data.collection` / `key`),
    /// answers to it (`data.need`) and buttons clicked on its row (`data.row`). The drawer's history.
    pub fn record_events(&mut self, mission: &str, collection: &str, key: &str, limit: i64) -> Result<Vec<Event>> {
        use diesel::dsl::sql;
        use diesel::sql_types::{Bool, Text};
        let about = sql::<Bool>("((json_extract(data, '$.collection') = ")
            .bind::<Text, _>(collection)
            .sql(" AND (json_extract(data, '$.key') = ")
            .bind::<Text, _>(key)
            .sql(" OR json_extract(data, '$.need') = ")
            .bind::<Text, _>(key)
            .sql(")) OR (json_extract(data, '$.row.collection') = ")
            .bind::<Text, _>(collection)
            .sql(" AND json_extract(data, '$.row.key') = ")
            .bind::<Text, _>(key)
            .sql("))");
        let mut rows = events::table
            .filter(events::mission.eq(mission))
            .filter(events::data.is_not_null())
            .filter(about)
            .order(events::seq.desc())
            .limit(limit)
            .select(EventRow::as_select())
            .load(&mut self.conn)?;
        rows.reverse();
        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// Deletes all but the newest `keep` events of `mission-control`. Returns how many went.
    pub fn prune_events(&mut self, mission: &str, keep: i64) -> Result<usize> {
        let cut: Option<i64> = events::table
            .filter(events::mission.eq(mission))
            .order(events::seq.desc())
            .offset(keep)
            .select(events::seq)
            .first(&mut self.conn)
            .optional()?;
        let Some(cut) = cut else { return Ok(0) };
        Ok(diesel::delete(events::table.filter(events::mission.eq(mission).and(events::seq.le(cut))))
            .execute(&mut self.conn)?)
    }
}
