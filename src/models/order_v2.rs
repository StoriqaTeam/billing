use std::fmt::{self, Display};
use std::io::Write;
use std::str::FromStr;
use std::time::SystemTime;

use diesel::pg::Pg;
use diesel::sql_types::Uuid as SqlUuid;
use diesel::types::{FromSql, ToSql};
use diesel::{
    deserialize,
    serialize::{self, Output},
};
use uuid::{self, Uuid};

use models::invoice_v2::InvoiceId;
use models::{Amount, Currency};
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

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, PartialEq)]
#[sql_type = "SqlUuid"]
pub struct ExchangeId(Uuid);
newtype_from_to_sql!(SqlUuid, ExchangeId, ExchangeId);

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

impl FromStr for ExchangeId {
    type Err = uuid::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = Uuid::parse_str(s)?;
        Ok(ExchangeId::new(id))
    }
}

impl Display for ExchangeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0.hyphenated()))
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
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[table_name = "orders"]
pub struct NewOrder {
    pub id: OrderId,
    pub seller_currency: Currency,
    pub total_amount: Amount,
    pub cashback_amount: Amount,
    pub invoice_id: InvoiceId,
}

#[derive(Debug, Clone)]
pub struct OrderAccess {
    pub invoice_id: InvoiceId,
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
