use chrono::NaiveDateTime;
use stq_types::stripe::{ChargeId, PaymentIntentId};

use models::{invoice_v2::InvoiceId, Amount, Currency};
use schema::payment_intent;

#[derive(Clone, Debug, Deserialize, Serialize, Queryable)]
pub struct PaymentIntent {
    pub id: PaymentIntentId,
    pub invoice_id: InvoiceId,
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
    pub invoice_id: InvoiceId,
    pub amount: Amount,
    pub amount_received: Amount,
    pub client_secret: Option<String>,
    pub currency: Currency,
    pub last_payment_error_message: Option<String>,
    pub receipt_email: Option<String>,
    pub charge_id: Option<ChargeId>,
    pub status: PaymentIntentStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, AsChangeset)]
#[table_name = "payment_intent"]
pub struct UpdatePaymentIntent {
    status: Option<PaymentIntentStatus>,
}

#[derive(Clone, Debug, Deserialize, Serialize, DieselTypes)]
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

impl From<NewPaymentIntent> for PaymentIntent {
    fn from(new_payment_intent: NewPaymentIntent) -> PaymentIntent {
        let now = chrono::offset::Utc::now().naive_utc();
        PaymentIntent {
            id: new_payment_intent.id,
            invoice_id: new_payment_intent.invoice_id,
            amount: new_payment_intent.amount,
            amount_received: new_payment_intent.amount_received,
            client_secret: new_payment_intent.client_secret,
            currency: new_payment_intent.currency,
            last_payment_error_message: new_payment_intent.last_payment_error_message,
            receipt_email: new_payment_intent.receipt_email,
            charge_id: new_payment_intent.charge_id,
            status: new_payment_intent.status,
            created_at: now,
            updated_at: now,
        }
    }
}
