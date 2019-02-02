use chrono::{NaiveDateTime, Utc};
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::query_dsl::RunQueryDsl;
use diesel::sql_types;
use diesel::{sql_query, Connection, ExpressionMethods, QueryDsl};
use failure::Fail;
use std::str::FromStr;

use models::{Event, EventEntry, EventEntryId, EventStatus, RawEventEntry, RawNewEventEntry};
use schema::event_store::dsl as EventStore;

use super::error::*;
use super::types::RepoResultV2;

pub trait EventStoreRepo {
    fn add_event(&self, event: Event) -> RepoResultV2<EventEntry>;

    fn add_scheduled_event(&self, event: Event, scheduled_on: NaiveDateTime) -> RepoResultV2<EventEntry>;

    fn get_events_for_processing(&self, limit: u32) -> RepoResultV2<Vec<EventEntry>>;

    fn reset_stuck_events(&self) -> RepoResultV2<Vec<EventEntry>>;

    fn complete_event(&self, event_entry_id: EventEntryId) -> RepoResultV2<EventEntry>;

    fn fail_event(&self, event_entry_id: EventEntryId) -> RepoResultV2<EventEntry>;
}

pub struct EventStoreRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub max_processing_attempts: u32,
    pub stuck_threshold_sec: u32,
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> EventStoreRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, max_processing_attempts: u32, stuck_threshold_sec: u32) -> Self {
        Self {
            db_conn,
            max_processing_attempts,
            stuck_threshold_sec,
        }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> EventStoreRepo for EventStoreRepoImpl<'a, T> {
    fn add_event(&self, event: Event) -> RepoResultV2<EventEntry> {
        debug!("Adding an event with ID: {}", event.id);

        let new_event_entry =
            RawNewEventEntry::try_from_event(event.clone()).map_err(ectx!(try ErrorSource::SerdeJson, ErrorKind::Internal => event))?;

        let raw_event_entry = diesel::insert_into(EventStore::event_store)
            .values(&new_event_entry)
            .get_result::<RawEventEntry>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        RawEventEntry::try_into_event_entry(raw_event_entry.clone())
            .map_err(ectx!(ErrorSource::SerdeJson, ErrorKind::Internal => raw_event_entry))
    }

    fn add_scheduled_event(&self, event: Event, scheduled_on: NaiveDateTime) -> RepoResultV2<EventEntry> {
        debug!(
            "Adding an event with ID: {} scheduled on {}",
            event.id,
            scheduled_on.format("%Y-%m-%d %H:%M:%S")
        );

        let new_event_entry = RawNewEventEntry::try_from_event_scheduled_on(event.clone(), scheduled_on)
            .map_err(ectx!(try ErrorSource::SerdeJson, ErrorKind::Internal => event))?;

        let raw_event_entry = diesel::insert_into(EventStore::event_store)
            .values(&new_event_entry)
            .get_result::<RawEventEntry>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        RawEventEntry::try_into_event_entry(raw_event_entry.clone())
            .map_err(ectx!(ErrorSource::SerdeJson, ErrorKind::Internal => raw_event_entry))
    }

    fn get_events_for_processing(&self, limit: u32) -> RepoResultV2<Vec<EventEntry>> {
        debug!("Getting events for processing (limit: {})", limit);

        let now = Utc::now().naive_utc();

        let command = sql_query(
            "
            UPDATE event_store
            SET
                attempt_count = attempt_count + 1,
                status = $1,
                status_updated_at = $2
            WHERE id IN (
                SELECT id
                FROM event_store
                WHERE status = $3 AND (scheduled_on is null OR scheduled_on <= $4)
                ORDER BY id
                LIMIT $5
                FOR UPDATE SKIP LOCKED
            )
            RETURNING *
        ",
        )
        .bind::<sql_types::VarChar, _>(EventStatus::InProgress.to_string())
        .bind::<sql_types::Timestamp, _>(now)
        .bind::<sql_types::VarChar, _>(EventStatus::Pending.to_string())
        .bind::<sql_types::Timestamp, _>(now)
        .bind::<sql_types::BigInt, _>(limit as i64);

        let raw_event_entries = command.get_results::<RawEventEntry>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(try err e, ErrorSource::Diesel, error_kind)
        })?;

        raw_event_entries
            .into_iter()
            .map(|raw_event_entry| {
                RawEventEntry::try_into_event_entry(raw_event_entry.clone())
                    .map_err(ectx!(ErrorSource::SerdeJson, ErrorKind::Internal => raw_event_entry))
            })
            .collect::<Result<Vec<_>, _>>()
    }

    fn reset_stuck_events(&self) -> RepoResultV2<Vec<EventEntry>> {
        debug!(
            "Resetting stuck events left in \"{}\" status for more than {} seconds",
            EventStatus::InProgress,
            self.stuck_threshold_sec
        );

        let stuck_threshold = chrono::NaiveTime::from_num_seconds_from_midnight_opt(self.stuck_threshold_sec, 0).ok_or({
            let e = format_err!("Invalid number of seconds for stuck threshold: {}", self.stuck_threshold_sec);
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let now = chrono::Utc::now().naive_utc();

        let command = sql_query(
            "
            UPDATE event_store
            SET
                status = CASE WHEN attempt_count >= $1 THEN $2 ELSE $3 END,
                status_updated_at = $4
            WHERE id IN (
                SELECT id
                FROM event_store
                WHERE status = $5 AND status_updated_at + $6 < $7
                FOR UPDATE SKIP LOCKED
            )
            RETURNING *
        ",
        )
        .bind::<sql_types::Integer, _>(self.max_processing_attempts as i32)
        .bind::<sql_types::VarChar, _>(EventStatus::Failed.to_string())
        .bind::<sql_types::VarChar, _>(EventStatus::Pending.to_string())
        .bind::<sql_types::Timestamp, _>(now)
        .bind::<sql_types::VarChar, _>(EventStatus::InProgress.to_string())
        .bind::<sql_types::Time, _>(stuck_threshold)
        .bind::<sql_types::Timestamp, _>(now);

        let raw_event_entries = command.get_results::<RawEventEntry>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(try err e, ErrorSource::Diesel, error_kind)
        })?;

        raw_event_entries
            .into_iter()
            .map(|raw_event_entry| {
                RawEventEntry::try_into_event_entry(raw_event_entry.clone())
                    .map_err(ectx!(ErrorSource::SerdeJson, ErrorKind::Internal => raw_event_entry))
            })
            .collect::<Result<Vec<_>, _>>()
    }

    fn complete_event(&self, event_entry_id: EventEntryId) -> RepoResultV2<EventEntry> {
        debug!("Completing an event with ID: {}", event_entry_id);

        self.db_conn.transaction(|| {
            let event_status = EventStore::event_store
                .filter(EventStore::id.eq(event_entry_id))
                .select(EventStore::status)
                .get_result::<String>(self.db_conn)
                .map_err(|e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(try err e, ErrorSource::Diesel, error_kind)
                })?;

            let event_status = EventStatus::from_str(event_status.as_str()).map_err(|_| ErrorKind::Internal)?;

            if event_status != EventStatus::InProgress {
                let e = format_err!(
                    "Cannot change status from \"{}\" to \"{}\" for event entry with ID: {}",
                    event_status,
                    EventStatus::Completed,
                    event_entry_id,
                );
                return Err(ectx!(err e, ErrorKind::Internal));
            }

            let raw_event_entry = diesel::update(EventStore::event_store)
                .filter(EventStore::id.eq(event_entry_id))
                .set((
                    EventStore::status.eq(&EventStatus::Completed.to_string()),
                    EventStore::status_updated_at.eq(chrono::Utc::now().naive_utc()),
                ))
                .get_result::<RawEventEntry>(self.db_conn)
                .map_err(|e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(try err e, ErrorSource::Diesel, error_kind)
                })?;

            RawEventEntry::try_into_event_entry(raw_event_entry.clone())
                .map_err(ectx!(ErrorSource::SerdeJson, ErrorKind::Internal => raw_event_entry))
        })
    }

    fn fail_event(&self, event_entry_id: EventEntryId) -> RepoResultV2<EventEntry> {
        debug!("Failing an event with ID: {}", event_entry_id);

        self.db_conn.transaction(|| {
            let (event_status, attempt_count) = EventStore::event_store
                .filter(EventStore::id.eq(event_entry_id))
                .select((EventStore::status, EventStore::attempt_count))
                .get_result::<(String, i32)>(self.db_conn)
                .map_err(|e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(try err e, ErrorSource::Diesel, error_kind)
                })?;

            let event_status = EventStatus::from_str(event_status.as_str()).map_err(|_| ErrorKind::Internal)?;

            let new_event_status = if attempt_count >= self.max_processing_attempts as i32 {
                EventStatus::Failed
            } else {
                EventStatus::Pending
            };

            if event_status != EventStatus::InProgress {
                let e = format_err!(
                    "Cannot change status from \"{}\" to \"{}\" for event entry with ID: {}",
                    event_status,
                    new_event_status,
                    event_entry_id,
                );
                return Err(ectx!(err e, ErrorKind::Internal));
            }

            let raw_event_entry = diesel::update(EventStore::event_store)
                .filter(EventStore::id.eq(event_entry_id))
                .set((
                    EventStore::status.eq(&new_event_status.to_string()),
                    EventStore::status_updated_at.eq(chrono::Utc::now().naive_utc()),
                ))
                .get_result::<RawEventEntry>(self.db_conn)
                .map_err(|e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(try err e, ErrorSource::Diesel, error_kind)
                })?;

            RawEventEntry::try_into_event_entry(raw_event_entry.clone())
                .map_err(ectx!(ErrorSource::SerdeJson, ErrorKind::Internal => raw_event_entry))
        })
    }
}
