use std::io::Write;

use chrono::NaiveDateTime;
use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::VarChar;
use enum_iterator::IntoEnumIterator;

use stq_types::{Quantity, StoreId, SubscriptionId, SubscriptionPaymentId};

use models::{Amount, ChargeId, Currency, TransactionId, WalletAddress};

use schema::{store_subscription, subscription, subscription_payment};

#[derive(Clone, Debug, Serialize, Deserialize, Queryable, Insertable)]
#[table_name = "subscription"]
pub struct Subscription {
    pub id: SubscriptionId,
    pub store_id: StoreId,
    pub published_base_products_quantity: Quantity,
    pub subscription_payment_id: Option<SubscriptionPaymentId>,
    pub created_at: NaiveDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize, Queryable, Insertable)]
#[table_name = "store_subscription"]
pub struct StoreSubscription {
    pub store_id: StoreId,
    pub currency: Currency,
    pub value: Amount,
    pub wallet_address: Option<WalletAddress>,
    pub trial_start_date: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub status: StoreSubscriptionStatus,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Eq, PartialEq, Hash, IntoEnumIterator)]
#[sql_type = "VarChar"]
#[serde(rename_all = "lowercase")]
pub enum StoreSubscriptionStatus {
    Trial,
    Paid,
    Free,
}

#[derive(Clone, Debug, Serialize, Deserialize, Queryable, Insertable)]
#[table_name = "subscription_payment"]
pub struct SubscriptionPayment {
    pub id: SubscriptionPaymentId,
    pub store_id: StoreId,
    pub amount: Amount,
    pub currency: Currency,
    pub charge_id: Option<ChargeId>,
    pub transaction_id: Option<TransactionId>,
    pub status: SubscriptionPaymentStatus,
    pub created_at: NaiveDateTime,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Eq, PartialEq, Hash, IntoEnumIterator)]
#[sql_type = "VarChar"]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionPaymentStatus {
    Paid,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "subscription"]
pub struct NewSubscription {
    pub store_id: StoreId,
    pub published_base_products_quantity: Quantity,
}

#[derive(Clone, Debug, Serialize, Deserialize, AsChangeset)]
#[table_name = "subscription"]
pub struct UpdateSubscription {
    pub subscription_payment_id: SubscriptionPaymentId,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "store_subscription"]
pub struct NewStoreSubscription {
    pub store_id: StoreId,
    pub currency: Currency,
    pub value: Amount,
    pub wallet_address: Option<WalletAddress>,
    pub trial_start_date: Option<NaiveDateTime>,
}

pub struct CreateStoreSubscription {
    pub currency: Currency,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, AsChangeset)]
#[table_name = "store_subscription"]
pub struct UpdateStoreSubscription {
    pub currency: Option<Currency>,
    pub value: Option<Amount>,
    pub wallet_address: Option<WalletAddress>,
    pub trial_start_date: Option<NaiveDateTime>,
    pub status: Option<StoreSubscriptionStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "subscription_payment"]
pub struct NewSubscriptionPayment {
    pub store_id: StoreId,
    pub amount: Amount,
    pub currency: Currency,
    pub charge_id: Option<ChargeId>,
    pub transaction_id: Option<TransactionId>,
    pub status: SubscriptionPaymentStatus,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubscriptionSearch {
    pub id: Option<SubscriptionId>,
    pub store_id: Option<StoreId>,
    pub paid: Option<bool>,
    pub subscription_payment_id: Option<SubscriptionPaymentId>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoreSubscriptionSearch {
    pub store_id: Option<StoreId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionPaymentSearch {
    pub id: Option<SubscriptionPaymentId>,
    pub store_id: Option<StoreId>,
    pub status: Option<SubscriptionPaymentStatus>,
}

#[derive(Serialize, Clone, Debug)]
pub struct SubscriptionPaymentSearchResults {
    pub total_count: i64,
    pub subscription_payments: Vec<SubscriptionPayment>,
}

impl SubscriptionSearch {
    pub fn by_id(id: SubscriptionId) -> SubscriptionSearch {
        SubscriptionSearch {
            id: Some(id),
            ..Default::default()
        }
    }

    pub fn by_subscription_payment_id(id: SubscriptionPaymentId) -> SubscriptionSearch {
        SubscriptionSearch {
            subscription_payment_id: Some(id),
            ..Default::default()
        }
    }
}

impl StoreSubscriptionSearch {
    pub fn by_store_id(store_id: StoreId) -> StoreSubscriptionSearch {
        StoreSubscriptionSearch {
            store_id: Some(store_id),
            ..Default::default()
        }
    }
}

impl FromSql<VarChar, Pg> for StoreSubscriptionStatus {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        match data {
            Some(b"trial") => Ok(StoreSubscriptionStatus::Trial),
            Some(b"paid") => Ok(StoreSubscriptionStatus::Paid),
            Some(b"free") => Ok(StoreSubscriptionStatus::Free),
            Some(v) => Err(format!(
                "Unrecognized enum variant: {:?}",
                String::from_utf8(v.to_vec()).unwrap_or_else(|_| "Non - UTF8 value".to_string()),
            )
            .to_string()
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

impl ToSql<VarChar, Pg> for StoreSubscriptionStatus {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            StoreSubscriptionStatus::Trial => out.write_all(b"trial")?,
            StoreSubscriptionStatus::Paid => out.write_all(b"paid")?,
            StoreSubscriptionStatus::Free => out.write_all(b"free")?,
        };
        Ok(IsNull::No)
    }
}

impl FromSql<VarChar, Pg> for SubscriptionPaymentStatus {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        match data {
            Some(b"paid") => Ok(SubscriptionPaymentStatus::Paid),
            Some(b"failed") => Ok(SubscriptionPaymentStatus::Failed),
            Some(v) => Err(format!(
                "Unrecognized enum variant: {:?}",
                String::from_utf8(v.to_vec()).unwrap_or_else(|_| "Non - UTF8 value".to_string()),
            )
            .to_string()
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

impl ToSql<VarChar, Pg> for SubscriptionPaymentStatus {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            SubscriptionPaymentStatus::Paid => out.write_all(b"paid")?,
            SubscriptionPaymentStatus::Failed => out.write_all(b"failed")?,
        };
        Ok(IsNull::No)
    }
}
