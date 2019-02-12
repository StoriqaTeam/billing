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
    pub fn classify(self) -> CurrencyChoice {
        use self::Currency::*;
        use self::CurrencyChoice::*;

        match self {
            Eth => Crypto(TureCurrency::Eth),
            Stq => Crypto(TureCurrency::Stq),
            Btc => Crypto(TureCurrency::Btc),
            Eur => Fiat(FiatCurrency::Eur),
            Usd => Fiat(FiatCurrency::Usd),
            Rub => Fiat(FiatCurrency::Rub),
        }
    }

    pub fn is_fiat(self) -> bool {
        use self::CurrencyChoice::*;

        match self.classify() {
            Crypto(_) => false,
            Fiat(_) => true,
        }
    }

    pub fn try_from_stq_currency(currency: StqCurrency) -> Result<Self, ()> {
        match currency {
            StqCurrency::ETH => Ok(Currency::Eth),
            StqCurrency::STQ => Ok(Currency::Stq),
            StqCurrency::BTC => Ok(Currency::Btc),
            StqCurrency::EUR => Ok(Currency::Eur),
            StqCurrency::USD => Ok(Currency::Usd),
            StqCurrency::RUB => Ok(Currency::Rub),
        }
    }

    pub fn try_from_stripe_currency(currency: stripe::Currency) -> Result<Self, ()> {
        let currency_str = format!("{}", currency);
        Currency::from_str(&currency_str).map_err(|_| ())
    }

    pub fn try_into_stripe_currency(self) -> Result<stripe::Currency, ()> {
        let currency_str = format!("{}", self);
        stripe::Currency::from_str(&currency_str).map_err(|_| ())
    }
}

impl Into<StqCurrency> for Currency {
    fn into(self) -> StqCurrency {
        match self {
            Currency::Eth => StqCurrency::ETH,
            Currency::Stq => StqCurrency::STQ,
            Currency::Btc => StqCurrency::BTC,
            Currency::Eur => StqCurrency::EUR,
            Currency::Usd => StqCurrency::USD,
            Currency::Rub => StqCurrency::RUB,
        }
    }
}

#[derive(Debug, Clone)]
pub enum CurrencyChoice {
    Crypto(TureCurrency),
    Fiat(FiatCurrency),
}

#[derive(Debug, Clone, Fail)]
#[fail(display = "failed to parse Ture currency")]
pub struct ParseTureCurrencyError;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Eq, PartialEq, Hash, IntoEnumIterator)]
#[sql_type = "VarChar"]
#[serde(rename_all = "lowercase")]
pub enum TureCurrency {
    Eth,
    Stq,
    Btc,
}

impl FromStr for TureCurrency {
    type Err = ParseTureCurrencyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "eth" => Ok(TureCurrency::Eth),
            "stq" => Ok(TureCurrency::Stq),
            "btc" => Ok(TureCurrency::Btc),
            _ => Err(ParseTureCurrencyError),
        }
    }
}

impl Display for TureCurrency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TureCurrency::Eth => f.write_str("eth"),
            TureCurrency::Stq => f.write_str("stq"),
            TureCurrency::Btc => f.write_str("btc"),
        }
    }
}

impl FromSql<VarChar, Pg> for TureCurrency {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        match data {
            Some(b"eth") => Ok(TureCurrency::Eth),
            Some(b"stq") => Ok(TureCurrency::Stq),
            Some(b"btc") => Ok(TureCurrency::Btc),
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

impl ToSql<VarChar, Pg> for TureCurrency {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            TureCurrency::Eth => out.write_all(b"eth")?,
            TureCurrency::Stq => out.write_all(b"stq")?,
            TureCurrency::Btc => out.write_all(b"btc")?,
        };
        Ok(IsNull::No)
    }
}

impl From<TureCurrency> for Currency {
    fn from(ture_currency: TureCurrency) -> Self {
        match ture_currency {
            TureCurrency::Eth => Currency::Eth,
            TureCurrency::Stq => Currency::Stq,
            TureCurrency::Btc => Currency::Btc,
        }
    }
}

impl TureCurrency {
    pub fn try_from_currency(currency: Currency) -> Result<Self, ()> {
        match currency {
            Currency::Eth => Ok(TureCurrency::Eth),
            Currency::Stq => Ok(TureCurrency::Stq),
            Currency::Btc => Ok(TureCurrency::Btc),
            Currency::Usd | Currency::Eur | Currency::Rub => Err(()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Eq, PartialEq, Hash, IntoEnumIterator)]
#[sql_type = "VarChar"]
#[serde(rename_all = "lowercase")]
pub enum FiatCurrency {
    Eur,
    Usd,
    Rub,
}

impl From<FiatCurrency> for Currency {
    fn from(fiat_currency: FiatCurrency) -> Self {
        match fiat_currency {
            FiatCurrency::Usd => Currency::Usd,
            FiatCurrency::Eur => Currency::Eur,
            FiatCurrency::Rub => Currency::Rub,
        }
    }
}

#[derive(Debug, Clone, Fail)]
pub enum ConversionError {
    #[fail(display = "unsupported currency: {}", _0)]
    UnsupportedCurrency(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_into_stripe_currency() {
        use self::Currency::*;
        for currency in Currency::into_enum_iter() {
            match currency {
                Eth => assert_eq!(currency.try_into_stripe_currency(), Err(())),
                Stq => assert_eq!(currency.try_into_stripe_currency(), Err(())),
                Btc => assert_eq!(currency.try_into_stripe_currency(), Err(())),
                Eur => assert_eq!(currency.try_into_stripe_currency(), Ok(stripe::Currency::EUR)),
                Usd => assert_eq!(currency.try_into_stripe_currency(), Ok(stripe::Currency::USD)),
                Rub => assert_eq!(currency.try_into_stripe_currency(), Ok(stripe::Currency::RUB)),
            }
        }
    }

    #[test]
    fn test_try_from_stripe_currency() {
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::EUR), Ok(Currency::Eur));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::USD), Ok(Currency::Usd));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::RUB), Ok(Currency::Rub));

        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::AED), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::AFN), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::ALL), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::AMD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::ANG), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::AOA), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::ARS), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::AUD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::AWG), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::AZN), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::BAM), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::BBD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::BDT), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::BGN), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::BIF), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::BMD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::BND), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::BOB), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::BRL), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::BSD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::BWP), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::BZD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::CAD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::CDF), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::CHF), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::CLP), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::CNY), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::COP), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::CRC), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::CVE), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::CZK), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::DJF), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::DKK), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::DOP), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::DZD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::EEK), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::EGP), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::ETB), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::FJD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::FKP), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::GBP), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::GEL), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::GIP), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::GMD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::GNF), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::GTQ), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::GYD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::HKD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::HNL), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::HRK), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::HTG), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::HUF), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::IDR), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::ILS), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::INR), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::ISK), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::JMD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::JPY), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::KES), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::KGS), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::KHR), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::KMF), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::KRW), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::KYD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::KZT), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::LAK), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::LBP), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::LKR), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::LRD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::LSL), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::LTL), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::LVL), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::MAD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::MDL), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::MGA), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::MKD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::MNT), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::MOP), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::MRO), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::MUR), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::MVR), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::MWK), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::MXN), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::MYR), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::MZN), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::NAD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::NGN), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::NIO), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::NOK), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::NPR), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::NZD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::PAB), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::PEN), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::PGK), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::PHP), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::PKR), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::PLN), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::PYG), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::QAR), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::RON), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::RSD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::RWF), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::SAR), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::SBD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::SCR), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::SEK), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::SGD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::SHP), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::SLL), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::SOS), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::SRD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::STD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::SVC), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::SZL), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::THB), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::TJS), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::TOP), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::TRY), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::TTD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::TWD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::TZS), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::UAH), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::UGX), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::UYU), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::UZS), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::VEF), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::VND), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::VUV), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::WST), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::XAF), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::XCD), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::XOF), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::XPF), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::YER), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::ZAR), Err(()));
        assert_eq!(Currency::try_from_stripe_currency(stripe::Currency::ZMW), Err(()));
    }
}
