use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use failure::Fail;
use std::str::FromStr;
use uuid::Uuid;

use models::{Amount, Currency, WalletAddress};

use super::error::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub account_address: WalletAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResponse {
    pub id: Uuid,
    pub balance: String,
    pub currency: String,
    pub user_id: u32,
    pub account_address: String,
    pub name: String,
    pub erc_20_approved: bool,
}

impl AccountResponse {
    pub fn try_into_account(self) -> Result<Account, Error> {
        let AccountResponse {
            id,
            balance,
            currency,
            name,
            account_address,
            ..
        } = self;

        let balance = Amount::from_str(&balance).map_err(ectx!(try ErrorKind::Internal => balance))?;
        let currency = Currency::from_str(&currency).map_err(ectx!(try ErrorKind::Internal => currency))?;
        let account_address = WalletAddress::from(account_address);

        Ok(Account {
            id,
            balance,
            currency,
            name,
            account_address,
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
#[serde(rename_all = "camelCase")]
pub struct RefreshRateResponse {
    pub rate: GetRateResponse,
    pub is_new_rate: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateRefresh {
    pub rate: Rate,
    pub is_new_rate: bool,
}

impl From<RefreshRateResponse> for RateRefresh {
    fn from(response: RefreshRateResponse) -> Self {
        let RefreshRateResponse { rate, is_new_rate } = response;

        RateRefresh {
            rate: Rate::from(rate),
            is_new_rate,
        }
    }
}
