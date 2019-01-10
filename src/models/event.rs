use diesel::sql_types::Uuid as SqlUuid;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, PartialEq, Eq, FromStr, Display)]
#[sql_type = "SqlUuid"]
pub struct EventId(Uuid);
derive_newtype_sql!(event, SqlUuid, EventId, EventId);

impl EventId {
    pub fn new(id: Uuid) -> Self {
        EventId(id)
    }

    pub fn inner(&self) -> Uuid {
        self.0
    }

    pub fn generate() -> Self {
        EventId(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventPayload {
    NoOp,
}
