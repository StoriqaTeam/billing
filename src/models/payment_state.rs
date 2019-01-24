use std::fmt::{self, Display};
use std::io::Write;
use std::str::FromStr;

use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::VarChar;
use enum_iterator::IntoEnumIterator;
use failure::Fail;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Eq, PartialEq, Hash, IntoEnumIterator)]
#[sql_type = "VarChar"]
#[serde(rename_all = "lowercase")]
pub enum PaymentState {
    /// Order created and maybe paid by customer
    Initial,
    /// Store manager declined the order
    Declined,
    /// Store manager confirmed the order, money was captured
    Captured,
    /// Need money refund to customer
    RefundNeeded,
    /// Money was refunded to customer
    Refunded,
    /// Money was paid to seller
    PaidToSeller,
    /// Need money payment to seller
    PaymentToSellerNeeded,
}

#[derive(Debug, Clone, Fail)]
#[fail(display = "failed to parse payment state")]
pub struct ParsePaymentStateError;

impl FromStr for PaymentState {
    type Err = ParsePaymentStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "initial" => Ok(PaymentState::Initial),
            "declined" => Ok(PaymentState::Declined),
            "captured" => Ok(PaymentState::Captured),
            "refunded" => Ok(PaymentState::Refunded),
            "refund_needed" => Ok(PaymentState::RefundNeeded),
            "paid_to_seller" => Ok(PaymentState::PaidToSeller),
            "payment_to_seller_needed" => Ok(PaymentState::PaymentToSellerNeeded),
            _ => Err(ParsePaymentStateError),
        }
    }
}

impl FromSql<VarChar, Pg> for PaymentState {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        match data {
            Some(b"initial") => Ok(PaymentState::Initial),
            Some(b"declined") => Ok(PaymentState::Declined),
            Some(b"captured") => Ok(PaymentState::Captured),
            Some(b"refunded") => Ok(PaymentState::Refunded),
            Some(b"refund_needed") => Ok(PaymentState::RefundNeeded),
            Some(b"paid_to_seller") => Ok(PaymentState::PaidToSeller),
            Some(b"payment_to_seller_needed") => Ok(PaymentState::PaymentToSellerNeeded),
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

impl ToSql<VarChar, Pg> for PaymentState {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            PaymentState::Initial => out.write_all(b"initial")?,
            PaymentState::Declined => out.write_all(b"declined")?,
            PaymentState::Captured => out.write_all(b"captured")?,
            PaymentState::Refunded => out.write_all(b"refunded")?,
            PaymentState::RefundNeeded => out.write_all(b"refund_needed")?,
            PaymentState::PaidToSeller => out.write_all(b"paid_to_seller")?,
            PaymentState::PaymentToSellerNeeded => out.write_all(b"payment_to_seller_needed")?,
        };
        Ok(IsNull::No)
    }
}

impl Display for PaymentState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PaymentState::Initial => f.write_str("initial"),
            PaymentState::Declined => f.write_str("declined"),
            PaymentState::Captured => f.write_str("captured"),
            PaymentState::Refunded => f.write_str("refunded"),
            PaymentState::RefundNeeded => f.write_str("refund_needed"),
            PaymentState::PaidToSeller => f.write_str("paid_to_seller"),
            PaymentState::PaymentToSellerNeeded => f.write_str("payment_to_seller_needed"),
        }
    }
}
