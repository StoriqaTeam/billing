use std::fmt::{self, Display};
use std::io::Write;
use std::str::FromStr;
use std::time::SystemTime;

use bigdecimal::BigDecimal;
use diesel::pg::Pg;
use diesel::sql_types::{BigInt, VarChar};
use diesel::types::{FromSql, IsNull, ToSql};
use diesel::{
    deserialize,
    serialize::{self, Output},
};

use models::order_v2::{ExchangeId, OrderId};
use schema::order_exchange_rates;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, PartialEq)]
#[sql_type = "BigInt"]
pub struct OrderExchangeRateId(i64);
derive_newtype_sql!(order_exchange_rate, BigInt, OrderExchangeRateId, OrderExchangeRateId);

impl OrderExchangeRateId {
    pub fn new(id: i64) -> Self {
        OrderExchangeRateId(id)
    }

    pub fn inner(&self) -> i64 {
        self.0
    }
}

impl FromStr for OrderExchangeRateId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        i64::from_str(s).map(OrderExchangeRateId::new)
    }
}

impl Display for OrderExchangeRateId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.inner().to_string())
    }
}

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Eq, PartialEq, Hash)]
#[sql_type = "VarChar"]
#[serde(rename_all = "lowercase")]
pub enum ExchangeRateStatus {
    Active,
    Expired,
}

#[derive(Debug, Clone, Fail)]
#[fail(display = "failed to parse exchange rate status")]
pub struct ParseExchangeRateStatusError;

impl FromStr for ExchangeRateStatus {
    type Err = ParseExchangeRateStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "active" => Ok(ExchangeRateStatus::Active),
            "expired" => Ok(ExchangeRateStatus::Expired),
            _ => Err(ParseExchangeRateStatusError),
        }
    }
}

impl FromSql<VarChar, Pg> for ExchangeRateStatus {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        match data {
            Some(b"active") => Ok(ExchangeRateStatus::Active),
            Some(b"expired") => Ok(ExchangeRateStatus::Expired),
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

impl ToSql<VarChar, Pg> for ExchangeRateStatus {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            ExchangeRateStatus::Active => out.write_all(b"active"),
            ExchangeRateStatus::Expired => out.write_all(b"expired"),
        }?;
        Ok(IsNull::No)
    }
}

impl Display for ExchangeRateStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExchangeRateStatus::Active => f.write_str("active"),
            ExchangeRateStatus::Expired => f.write_str("expired"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable)]
#[table_name = "order_exchange_rates"]
pub struct RawOrderExchangeRate {
    pub id: OrderExchangeRateId,
    pub order_id: OrderId,
    pub exchange_id: Option<ExchangeId>,
    pub exchange_rate: BigDecimal,
    pub status: ExchangeRateStatus,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewOrderExchangeRate {
    pub order_id: OrderId,
    pub exchange_id: Option<ExchangeId>,
    pub exchange_rate: BigDecimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[table_name = "order_exchange_rates"]
pub struct RawNewOrderExchangeRate {
    pub order_id: OrderId,
    pub exchange_id: Option<ExchangeId>,
    pub exchange_rate: BigDecimal,
    pub status: ExchangeRateStatus,
}

impl From<NewOrderExchangeRate> for RawNewOrderExchangeRate {
    fn from(new_rate: NewOrderExchangeRate) -> RawNewOrderExchangeRate {
        let NewOrderExchangeRate {
            order_id,
            exchange_id,
            exchange_rate,
        } = new_rate;

        RawNewOrderExchangeRate {
            order_id,
            exchange_id,
            exchange_rate,
            status: ExchangeRateStatus::Active,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, AsChangeset)]
#[table_name = "order_exchange_rates"]
pub struct SetExchangeRateStatus {
    pub status: ExchangeRateStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestExchangeRates {
    pub active_rate: RawOrderExchangeRate,
    pub last_expired_rate: Option<RawOrderExchangeRate>,
}

#[derive(Debug, Clone)]
pub struct OrderExchangeRateAccess {
    pub order_id: OrderId,
}

impl From<NewOrderExchangeRate> for OrderExchangeRateAccess {
    fn from(new_order_exchange_rate: NewOrderExchangeRate) -> OrderExchangeRateAccess {
        OrderExchangeRateAccess {
            order_id: new_order_exchange_rate.order_id.clone(),
        }
    }
}

impl From<RawOrderExchangeRate> for OrderExchangeRateAccess {
    fn from(raw_order_exchange_rate: RawOrderExchangeRate) -> OrderExchangeRateAccess {
        OrderExchangeRateAccess {
            order_id: raw_order_exchange_rate.order_id.clone(),
        }
    }
}
