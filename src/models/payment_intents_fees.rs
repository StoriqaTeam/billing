use chrono::NaiveDateTime;
use stq_types::stripe::PaymentIntentId;

use models::fee::FeeId;
use schema::payment_intents_fees;

#[derive(Clone, Debug, Deserialize, Serialize, Queryable)]
pub struct PaymentIntentFee {
    pub id: i32,
    pub fee_id: FeeId,
    pub payment_intent_id: PaymentIntentId,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Clone, Debug, Deserialize, Serialize, Queryable, Insertable)]
#[table_name = "payment_intents_fees"]
pub struct NewPaymentIntentFee {
    pub fee_id: FeeId,
    pub payment_intent_id: PaymentIntentId,
}

#[derive(Debug, Clone, Copy)]
pub struct PaymentIntentFeeAccess {
    pub fee_id: FeeId,
}
