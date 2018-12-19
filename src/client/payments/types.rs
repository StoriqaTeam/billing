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
