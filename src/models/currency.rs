use std::fmt::{self, Display};
use std::io::Write;
use std::str::FromStr;

use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::VarChar;
use enum_iterator::IntoEnumIterator;
use failure::Fail;
use stq_static_resources::Currency as StqCurrency;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Eq, PartialEq, Hash, IntoEnumIterator)]
#[sql_type = "VarChar"]
#[serde(rename_all = "lowercase")]
pub enum Currency {
    Eth,
    Stq,
    Btc,
    Eur,
    Usd,
    Rub,
}

#[derive(Debug, Clone, Fail)]
#[fail(display = "failed to parse currency")]
pub struct ParseCurrencyError;

impl FromStr for Currency {
    type Err = ParseCurrencyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "eth" => Ok(Currency::Eth),
            "stq" => Ok(Currency::Stq),
            "btc" => Ok(Currency::Btc),
            "eur" => Ok(Currency::Eur),
            "usd" => Ok(Currency::Usd),
            "rub" => Ok(Currency::Rub),
            _ => Err(ParseCurrencyError),
        }
    }
}

impl FromSql<VarChar, Pg> for Currency {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        match data {
            Some(b"eth") => Ok(Currency::Eth),
            Some(b"stq") => Ok(Currency::Stq),
            Some(b"btc") => Ok(Currency::Btc),
            Some(b"eur") => Ok(Currency::Eur),
            Some(b"usd") => Ok(Currency::Usd),
            Some(b"rub") => Ok(Currency::Rub),
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

impl ToSql<VarChar, Pg> for Currency {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            Currency::Eth => out.write_all(b"eth")?,
            Currency::Stq => out.write_all(b"stq")?,
            Currency::Btc => out.write_all(b"btc")?,
            Currency::Eur => out.write_all(b"eur")?,
            Currency::Usd => out.write_all(b"usd")?,
            Currency::Rub => out.write_all(b"rub")?,
        };
        Ok(IsNull::No)
    }
}

impl Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Currency::Eth => f.write_str("eth"),
            Currency::Stq => f.write_str("stq"),
            Currency::Btc => f.write_str("btc"),
            Currency::Eur => f.write_str("eur"),
            Currency::Usd => f.write_str("usd"),
            Currency::Rub => f.write_str("rub"),
        }
    }
}

impl Currency {
    pub fn try_from_stq_currency(currency: StqCurrency) -> Result<Self, ()> {
        match currency {
            StqCurrency::ETH => Ok(Currency::Eth),
            StqCurrency::STQ => Ok(Currency::Stq),
            StqCurrency::BTC => Ok(Currency::Btc),
            _ => Err(()),
        }
    }
}

impl Into<StqCurrency> for Currency {
    fn into(self) -> StqCurrency {
        match self {
            Currency::Eth => StqCurrency::ETH,
            Currency::Stq => StqCurrency::STQ,
            Currency::Btc => StqCurrency::BTC,
        }
    }
}
