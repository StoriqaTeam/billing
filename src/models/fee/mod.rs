use std::fmt::{self, Display};

pub mod fee_id;
pub use self::fee_id::FeeId;

use chrono::NaiveDateTime;

use serde_json;

use models::order_v2::OrderId;
use models::{Amount, ChargeId, Currency};
use schema::fees;

#[derive(Clone, Debug, Deserialize, Serialize, Queryable)]
pub struct Fee {
    pub id: FeeId,
    pub order_id: OrderId,
    pub amount: Amount,
    pub status: FeeStatus,
    pub currency: Currency,
    pub charge_id: Option<ChargeId>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub crypto_currency: Option<Currency>,
    pub crypto_amount: Option<Amount>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Queryable, Insertable)]
#[table_name = "fees"]
pub struct NewFee {
    pub order_id: OrderId,
    pub amount: Amount,
    pub status: FeeStatus,
    pub currency: Currency,
    pub charge_id: Option<ChargeId>,
    pub metadata: Option<serde_json::Value>,
    pub crypto_currency: Option<Currency>,
    pub crypto_amount: Option<Amount>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, AsChangeset)]
#[table_name = "fees"]
pub struct UpdateFee {
    pub order_id: Option<OrderId>,
    pub amount: Option<Amount>,
    pub status: Option<FeeStatus>,
    pub currency: Option<Currency>,
    pub charge_id: Option<ChargeId>,
    pub metadata: Option<serde_json::Value>,
    pub crypto_currency: Option<Currency>,
    pub crypto_amount: Option<Amount>,
}

#[derive(Clone, Debug, Deserialize, Serialize, DieselTypes, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FeeStatus {
    NotPaid,
    Paid,
    Fail,
}

impl Display for FeeStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FeeStatus::NotPaid => write!(f, "NotPaid"),
            FeeStatus::Paid => write!(f, "Paid"),
            FeeStatus::Fail => write!(f, "Fail"),
        }
    }
}
