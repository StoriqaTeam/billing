use bigdecimal::ToPrimitive;

use stq_types::stripe::PaymentIntentId;

use models::{invoice_v2::InvoiceId, Currency};
use models::{ChargeId, PaymentIntent, PaymentIntentStatus};

use failure::Fail;

use services::error::{Error, ErrorContext, ErrorKind};

#[derive(Debug, Deserialize, Serialize)]
pub struct PaymentIntentResponse {
    pub id: PaymentIntentId,
    pub invoice_id: InvoiceId,
    pub amount: f64,
    pub amount_received: f64,
    pub client_secret: Option<String>,
    pub currency: Currency,
    pub last_payment_error_message: Option<String>,
    pub receipt_email: Option<String>,
    pub charge_id: Option<ChargeId>,
    pub status: PaymentIntentStatus,
}

impl PaymentIntentResponse {
    pub fn try_from_payment_intent(other: PaymentIntent) -> Result<Self, Error> {
        let other_amount = other.amount.to_super_unit(other.currency).to_f64();
        let other_amount_received = other.amount_received.to_super_unit(other.currency).to_f64();

        match (other_amount, other_amount_received) {
            (Some(amount), Some(amount_received)) => Ok(Self {
                id: other.id,
                invoice_id: other.invoice_id,
                amount,
                amount_received,
                client_secret: other.client_secret,
                currency: other.currency,
                last_payment_error_message: other.last_payment_error_message,
                receipt_email: other.receipt_email,
                charge_id: other.charge_id,
                status: other.status,
            }),
            _ => Err(ectx!(err ErrorContext::AmountConversion, ErrorKind::Internal)),
        }
    }
}
