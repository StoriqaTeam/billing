use chrono::NaiveDateTime;
use stq_types::stripe::PaymentIntentId;

use models::invoice_v2::InvoiceId;
use schema::payment_intents_invoices;

#[derive(Clone, Debug, Deserialize, Serialize, Queryable)]
pub struct PaymentIntentInvoice {
    pub id: i32,
    pub invoice_id: InvoiceId,
    pub payment_intent_id: PaymentIntentId,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Clone, Debug, Deserialize, Serialize, Queryable, Insertable)]
#[table_name = "payment_intents_invoices"]
pub struct NewPaymentIntentInvoice {
    pub invoice_id: InvoiceId,
    pub payment_intent_id: PaymentIntentId,
}
