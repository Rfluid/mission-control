//! Diesel row structs and their conversions to wire types.

use super::schema::{events, mission_config, missions, records};
use crate::model::{Event, Mission, Record};
use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = missions, check_for_backend(diesel::sqlite::Sqlite))]
pub struct MissionRow {
    pub id: String,
    pub title: String,
    pub session: Option<String>,
    pub project_dir: Option<String>,
    pub token: String,
    pub created: i64,
    pub archived: bool,
}

impl From<MissionRow> for Mission {
    fn from(r: MissionRow) -> Self {
        Self {
            id: r.id,
            title: r.title,
            session: r.session,
            project_dir: r.project_dir,
            created: r.created,
            archived: r.archived,
        }
    }
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = records, check_for_backend(diesel::sqlite::Sqlite))]
pub struct RecordRow {
    pub mission: String,
    pub collection: String,
    pub key: String,
    pub state: Option<String>,
    pub body: String,
    pub updated: i64,
}

impl TryFrom<RecordRow> for Record {
    type Error = anyhow::Error;
    fn try_from(r: RecordRow) -> anyhow::Result<Self> {
        Ok(Self {
            collection: r.collection,
            key: r.key,
            state: r.state,
            body: serde_json::from_str(&r.body)?,
            updated: r.updated,
        })
    }
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = events, check_for_backend(diesel::sqlite::Sqlite))]
pub struct EventRow {
    pub seq: i64,
    pub mission: String,
    pub kind: String,
    pub text: String,
    pub data: Option<String>,
    pub at: i64,
}

impl TryFrom<EventRow> for Event {
    type Error = anyhow::Error;
    fn try_from(r: EventRow) -> anyhow::Result<Self> {
        Ok(Self {
            seq: r.seq,
            mission: r.mission,
            kind: r.kind,
            text: r.text,
            data: r.data.as_deref().map(serde_json::from_str).transpose()?,
            at: r.at,
        })
    }
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = mission_config, check_for_backend(diesel::sqlite::Sqlite))]
pub struct ConfigRow {
    pub mission: String,
    pub version: i64,
    pub doc: String,
    pub note: Option<String>,
    pub at: i64,
}
