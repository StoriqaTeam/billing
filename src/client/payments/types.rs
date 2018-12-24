use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use failure::Fail;
use std::str::FromStr;
use uuid::Uuid;

use models::{Amount, Currency};

use super::error::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccount {
    pub id: Uuid,
    pub currency: Currency,
    pub name: String,
    pub callback_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub balance: Amount,
    pub currency: Currency,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountResponse {
    pub id: Uuid,
    pub balance: String,
    pub currency: String,
    pub name: String,
}

impl AccountResponse {
    pub fn try_into_account(self) -> Result<Account, Error> {
        let AccountResponse {
            id,
            balance,
            currency,
            name,
        } = self;

        let balance = Amount::from_str(&balance).map_err(ectx!(try ErrorKind::Internal => balance))?;
        let currency = Currency::from_str(&currency).map_err(ectx!(try ErrorKind::Internal => currency))?;

        Ok(Account {
            id,
            balance,
            currency,
            name,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetRate {
    pub id: Uuid,
    pub from: Currency,
    pub to: Currency,
    pub amount_currency: Currency,
    pub amount: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetRateResponse {
    pub id: Uuid,
    pub from: Currency,
    pub to: Currency,
    pub amount: Amount,
    pub rate: f64,
    pub expiration: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rate {
    pub id: Uuid,
    pub from: Currency,
    pub to: Currency,
    pub amount: Amount,
    pub rate: BigDecimal,
    pub expiration: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<GetRateResponse> for Rate {
    fn from(response: GetRateResponse) -> Self {
        let GetRateResponse {
            id,
            from,
            to,
            amount,
            rate,
            expiration,
            created_at,
            updated_at,
        } = response;

        Rate {
            id,
            from,
            to,
            amount,
            rate: BigDecimal::from(rate),
            expiration,
            created_at,
            updated_at,
        }
    }
}
