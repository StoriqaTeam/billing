use chrono::NaiveDateTime;
use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::BigInt;
use std::fmt;
use std::io::Write;
use std::str::FromStr;

use models::event::Event;
use schema::event_store;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, PartialEq, Eq, FromStr, Display)]
#[sql_type = "BigInt"]
pub struct EventEntryId(i64);
newtype_from_to_sql!(BigInt, EventEntryId, EventEntryId);

impl EventEntryId {
    pub fn new(id: i64) -> Self {
        EventEntryId(id)
    }

    pub fn inner(&self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEntry {
    pub id: EventEntryId,
    pub event: Event,
    pub status: EventStatus,
    pub attempt_count: u32,
    pub created_at: NaiveDateTime,
    pub status_updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Fail)]
#[fail(display = "failed to parse event status")]
pub struct ParseEventStatusError;

impl FromStr for EventStatus {
    type Err = ParseEventStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(EventStatus::Pending),
            "in_progress" => Ok(EventStatus::InProgress),
            "completed" => Ok(EventStatus::Completed),
            "failed" => Ok(EventStatus::Failed),
            _ => Err(ParseEventStatusError),
        }
    }
}

impl fmt::Display for EventStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            EventStatus::Pending => "pending",
            EventStatus::InProgress => "in_progress",
            EventStatus::Completed => "completed",
            EventStatus::Failed => "failed",
        };

        f.write_str(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable, QueryableByName)]
#[table_name = "event_store"]
pub struct RawEventEntry {
    pub id: EventEntryId,
    pub event: serde_json::Value,
    pub status: String,
    pub attempt_count: i32,
    pub created_at: NaiveDateTime,
    pub status_updated_at: NaiveDateTime,
}

#[derive(Debug, Fail)]
pub enum RawEventEntryError {
    #[fail(display = "failed to deserialize event")]
    InvalidEventJson(serde_json::Error),
    #[fail(display = "invalid event status")]
    InvalidStatus,
}

impl RawEventEntry {
    pub fn try_into_event_entry(self) -> Result<EventEntry, RawEventEntryError> {
        let RawEventEntry {
            id,
            event,
            status,
            attempt_count,
            created_at,
            status_updated_at,
        } = self;

        let event = match serde_json::from_value::<Event>(event) {
            Ok(event) => event,
            Err(e) => {
                return Err(RawEventEntryError::InvalidEventJson(e));
            }
        };

        let status = match EventStatus::from_str(&status) {
            Ok(status) => status,
            Err(_) => {
                return Err(RawEventEntryError::InvalidStatus);
            }
        };

        Ok(EventEntry {
            id,
            event,
            status,
            attempt_count: attempt_count as u32,
            created_at,
            status_updated_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[table_name = "event_store"]
pub struct RawNewEventEntry {
    pub event: serde_json::Value,
    pub status: String,
    pub attempt_count: i32,
}

impl RawNewEventEntry {
    pub fn try_from_event(event: Event) -> Result<Self, serde_json::Error> {
        serde_json::to_value(&event).map(|event| Self {
            event,
            status: EventStatus::Pending.to_string(),
            attempt_count: 0,
        })
    }
}
