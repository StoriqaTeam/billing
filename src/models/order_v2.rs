use std::fmt::{self, Display};
use std::io::Write;
use std::str::FromStr;

use chrono::NaiveDateTime;
use diesel::pg::Pg;
use diesel::sql_types::Uuid as SqlUuid;
use diesel::types::{FromSql, ToSql};
use diesel::{
    deserialize,
    serialize::{self, Output},
};
use uuid::{self, Uuid};

use models::invoice_v2::InvoiceId;
use models::{Amount, Currency, PaymentState};
use schema::orders;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, PartialEq)]
#[sql_type = "SqlUuid"]
pub struct OrderId(Uuid);
newtype_from_to_sql!(SqlUuid, OrderId, OrderId);

impl OrderId {
    pub fn new(id: Uuid) -> Self {
        OrderId(id)
    }

    pub fn inner(&self) -> &Uuid {
        &self.0
    }

    pub fn into_inner(self) -> Uuid {
        self.0
    }

    pub fn generate() -> Self {
        OrderId(Uuid::new_v4())
    }
}

impl FromStr for OrderId {
    type Err = uuid::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = Uuid::parse_str(s)?;
        Ok(OrderId::new(id))
    }
}

impl Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0.hyphenated()))
    }
}

#[derive(Clone, Copy, Debug, Display, Default, PartialEq, Eq, From, FromStr, Hash, Serialize, Deserialize, DieselTypes)]
pub struct ExchangeId(Uuid);

impl ExchangeId {
    pub fn new(id: Uuid) -> Self {
        ExchangeId(id)
    }

    pub fn inner(&self) -> &Uuid {
        &self.0
    }

    pub fn generate() -> Self {
        ExchangeId(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, Display, Default, PartialEq, Eq, PartialOrd, Ord, From, FromStr, Hash, Serialize, Deserialize, DieselTypes)]
pub struct StoreId(i32);

impl StoreId {
    pub fn new(id: i32) -> Self {
        StoreId(id)
    }

    pub fn inner(&self) -> i32 {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable)]
#[table_name = "orders"]
pub struct RawOrder {
    pub id: OrderId,
    pub seller_currency: Currency,
    pub total_amount: Amount,
    pub cashback_amount: Amount,
    pub invoice_id: InvoiceId,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub store_id: StoreId,
    pub state: PaymentState,
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[table_name = "orders"]
pub struct NewOrder {
    pub id: OrderId,
    pub seller_currency: Currency,
    pub total_amount: Amount,
    pub cashback_amount: Amount,
    pub invoice_id: InvoiceId,
    pub store_id: StoreId,
}

#[derive(Debug, Clone)]
pub struct OrderAccess {
    pub invoice_id: InvoiceId,
}

#[derive(Debug, Clone)]
pub struct OrdersSearch {
    pub store_id: Option<StoreId>,
    pub state: Option<PaymentState>,
}

#[derive(Debug, Clone)]
pub struct OrderSearchResults {
    pub total_count: i64,
    pub orders: Vec<RawOrder>,
}

impl From<NewOrder> for OrderAccess {
    fn from(new_order: NewOrder) -> OrderAccess {
        OrderAccess {
            invoice_id: new_order.invoice_id.clone(),
        }
    }
}

impl From<RawOrder> for OrderAccess {
    fn from(raw_order: RawOrder) -> OrderAccess {
        OrderAccess {
            invoice_id: raw_order.invoice_id.clone(),
        }
    }
}
