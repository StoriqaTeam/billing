use chrono::NaiveDateTime;
use diesel::sql_types::Uuid as SqlUuid;
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::str::FromStr;
use uuid::{self, Uuid};

use config;
use models::currency::Currency;
use models::Amount;
use models::TransactionId;
use schema::accounts;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, AsExpression, FromSqlRow)]
#[sql_type = "SqlUuid"]
pub struct AccountId(Uuid);
derive_newtype_sql!(account_id, SqlUuid, AccountId, AccountId);

impl AccountId {
    pub fn new(id: Uuid) -> Self {
        AccountId(id)
    }

    pub fn inner(&self) -> &Uuid {
        &self.0
    }

    pub fn into_inner(self) -> Uuid {
        self.0
    }

    pub fn generate() -> Self {
        AccountId(Uuid::new_v4())
    }
}

impl FromStr for AccountId {
    type Err = uuid::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = Uuid::parse_str(s)?;
        Ok(AccountId::new(id))
    }
}

impl Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0.hyphenated()))
    }
}

#[derive(Clone, Debug, Display, Default, PartialEq, Eq, From, FromStr, Hash, Serialize, Deserialize, DieselTypes)]
pub struct WalletAddress(String);

impl WalletAddress {
    pub fn new(address: String) -> Self {
        WalletAddress(address)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountCount {
    pub pooled: HashMap<Currency, u64>,
    pub unpooled: HashMap<Currency, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub currency: Currency,
    pub is_pooled: bool,
    pub created_at: NaiveDateTime,
    pub wallet_address: Option<WalletAddress>,
}

impl Account {
    pub fn is_fiat(&self) -> bool {
        self.wallet_address.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountWithBalance {
    #[serde(flatten)]
    pub account: Account,
    pub balance: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable)]
#[table_name = "accounts"]
pub struct RawAccount {
    pub id: AccountId,
    pub currency: Currency,
    pub is_pooled: bool,
    pub created_at: NaiveDateTime,
    pub wallet_address: Option<WalletAddress>,
}

impl From<RawAccount> for Account {
    fn from(raw_account: RawAccount) -> Account {
        let RawAccount {
            id,
            currency,
            is_pooled,
            created_at,
            wallet_address,
        } = raw_account;

        Account {
            id,
            currency,
            is_pooled,
            created_at,
            wallet_address,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[table_name = "accounts"]
pub struct NewAccount {
    pub id: AccountId,
    pub currency: Currency,
    pub is_pooled: bool,
    pub wallet_address: Option<WalletAddress>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentsCallback {
    pub url: String,
    pub transaction_id: TransactionId,
    pub amount_captured: Amount,
    pub currency: Currency,
    pub address: WalletAddress,
    pub account_id: AccountId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SystemAccountType {
    Main,
    Cashback,
}

impl Display for SystemAccountType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SystemAccountType::Main => f.write_str("Main"),
            SystemAccountType::Cashback => f.write_str("Cashback"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemAccount {
    pub id: AccountId,
    pub currency: Currency,
    pub account_type: SystemAccountType,
}

impl Display for SystemAccount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{} {}", self.account_type, self.currency))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemAccounts(pub Vec<SystemAccount>);

impl SystemAccounts {
    pub fn get(&self, currency: Currency, account_type: SystemAccountType) -> Option<AccountId> {
        self.0
            .iter()
            .find(|account| account.currency == currency && account.account_type == account_type)
            .map(|account| account.id)
    }
}

impl From<config::Accounts> for SystemAccounts {
    fn from(config: config::Accounts) -> SystemAccounts {
        let config::Accounts {
            main_stq,
            main_eth,
            main_btc,
            cashback_stq,
        } = config;

        SystemAccounts(vec![
            SystemAccount {
                id: AccountId::new(main_stq),
                currency: Currency::Stq,
                account_type: SystemAccountType::Main,
            },
            SystemAccount {
                id: AccountId::new(main_eth),
                currency: Currency::Eth,
                account_type: SystemAccountType::Main,
            },
            SystemAccount {
                id: AccountId::new(main_btc),
                currency: Currency::Btc,
                account_type: SystemAccountType::Main,
            },
            SystemAccount {
                id: AccountId::new(cashback_stq),
                currency: Currency::Stq,
                account_type: SystemAccountType::Cashback,
            },
        ])
    }
}
