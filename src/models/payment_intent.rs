use chrono::NaiveDateTime;
use stq_types::stripe::PaymentIntentId;

use models::ChargeId;
use models::{Amount, Currency};
use schema::payment_intent;

#[derive(Clone, Debug, Deserialize, Serialize, Queryable)]
pub struct PaymentIntent {
    pub id: PaymentIntentId,
    pub amount: Amount,
    pub amount_received: Amount,
    pub client_secret: Option<String>,
    pub currency: Currency,
    pub last_payment_error_message: Option<String>,
    pub receipt_email: Option<String>,
    pub charge_id: Option<ChargeId>,
    pub status: PaymentIntentStatus,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Clone, Debug, Deserialize, Serialize, Queryable, Insertable)]
#[table_name = "payment_intent"]
pub struct NewPaymentIntent {
    pub id: PaymentIntentId,
    pub amount: Amount,
    pub amount_received: Amount,
    pub client_secret: Option<String>,
    pub currency: Currency,
    pub last_payment_error_message: Option<String>,
    pub receipt_email: Option<String>,
    pub charge_id: Option<ChargeId>,
    pub status: PaymentIntentStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, AsChangeset, Default)]
#[table_name = "payment_intent"]
pub struct UpdatePaymentIntent {
    pub status: Option<PaymentIntentStatus>,
    pub amount: Option<Amount>,
    pub amount_received: Option<Amount>,
    pub client_secret: Option<String>,
    pub currency: Option<Currency>,
    pub last_payment_error_message: Option<String>,
    pub receipt_email: Option<String>,
    pub charge_id: Option<ChargeId>,
}

#[derive(Clone, Debug, Deserialize, Serialize, DieselTypes, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentIntentStatus {
    RequiresSource,
    RequiresConfirmation,
    RequiresSourceAction,
    Processing,
    RequiresCapture,
    Canceled,
    Succeeded,
    #[serde(other)]
    Other,
}

impl PaymentIntentStatus {
    pub fn is_cancellable(&self) -> bool {
        match self {
            PaymentIntentStatus::RequiresSource
            | PaymentIntentStatus::RequiresConfirmation
            | PaymentIntentStatus::RequiresSourceAction
            | PaymentIntentStatus::RequiresCapture => true,
            _ => false,
        }
    }
}

pub struct PaymentIntentAccess {
    pub id: PaymentIntentId,
}

impl<'r> From<&'r PaymentIntent> for PaymentIntentAccess {
    fn from(payment_intent: &PaymentIntent) -> PaymentIntentAccess {
        PaymentIntentAccess {
            id: payment_intent.id.clone(),
        }
    }
}

impl From<stripe::PaymentIntentStatus> for PaymentIntentStatus {
    fn from(status: stripe::PaymentIntentStatus) -> PaymentIntentStatus {
        use stripe::PaymentIntentStatus::*;
        match status {
            RequiresSource => PaymentIntentStatus::RequiresSource,
            RequiresConfirmation => PaymentIntentStatus::RequiresConfirmation,
            RequiresSourceAction => PaymentIntentStatus::RequiresSourceAction,
            Processing => PaymentIntentStatus::Processing,
            RequiresCapture => PaymentIntentStatus::RequiresCapture,
            Canceled => PaymentIntentStatus::Canceled,
            Succeeded => PaymentIntentStatus::Succeeded,
            Other => PaymentIntentStatus::Other,
        }
    }
}
