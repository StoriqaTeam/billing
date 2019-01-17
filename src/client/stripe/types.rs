use stripe::Currency as StripeCurrency;
use stripe::TokenId;

use super::{Error, ErrorContext, ErrorKind};
use failure::Fail;
use models::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewCustomer {
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewCustomerWithSource {
    pub email: Option<String>,
    pub token: TokenId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewCharge {
    pub customer_id: CustomerId,
    pub amount: Amount,
    pub currency: Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Charge {
    pub id: ChargeId,
    pub customer_id: CustomerId,
    pub amount: Amount,
    pub currency: Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewRefund {
    pub charge_id: ChargeId,
    pub amount: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refund {
    pub id: ChargeId,
    pub amount: Amount,
    pub status: ChargeStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChargeStatus {
    Succeeded,
    Pending,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PayoutStatus {
    Paid,
    Pending,
    InTransit,
    Canceled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPayOut {
    pub customer_id: CustomerId,
    pub amount: Amount,
    pub currency: Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayOut {
    pub id: PayOutId,
    pub amount: Amount,
    pub currency: Currency,
    pub status: PayoutStatus,
}

impl Currency {
    pub fn convert(&self) -> Result<StripeCurrency, Error> {
        match self {
            Currency::Eur => Ok(StripeCurrency::EUR),
            Currency::Usd => Ok(StripeCurrency::USD),
            Currency::Rub => Ok(StripeCurrency::RUB),
            _ => Err(ectx!(err ErrorContext::Currency, ErrorKind::MalformedInput)),
        }
    }
}
