use diesel::sql_types::Uuid as SqlUuid;
use std::fmt;
use stripe::PaymentIntent;
use uuid::Uuid;

use models::invoice_v2::InvoiceId;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, PartialEq, Eq, FromStr)]
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

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0.hyphenated()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub payload: EventPayload,
}

impl Event {
    pub fn new(payload: EventPayload) -> Self {
        Self {
            id: EventId::generate(),
            payload,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum EventPayload {
    NoOp,
    InvoicePaid { invoice_id: InvoiceId },
    PaymentIntentPaymentFailed { payment_intent: PaymentIntent },
    PaymentIntentSucceeded { payment_intent: PaymentIntent },
}

impl fmt::Debug for EventPayload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = serde_json::to_string(self).unwrap_or(format!("{{\"{}\": <serialization failed>}}", self));
        f.write_str(&s)
    }
}

impl fmt::Display for EventPayload {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            EventPayload::NoOp => "NoOp",
            EventPayload::InvoicePaid { .. } => "InvoicePaid",
            EventPayload::PaymentIntentPaymentFailed { .. } => "PaymentIntentPaymentFailed",
            EventPayload::PaymentIntentSucceeded { .. } => "PaymentIntentSucceeded",
        };

        f.write_str(&s)
    }
}
