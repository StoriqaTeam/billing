use diesel::sql_types::Uuid as SqlUuid;
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::str::FromStr;
use std::time::SystemTime;
use uuid::{self, Uuid};

use models::currency::Currency;
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
    pub created_at: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable)]
#[table_name = "accounts"]
pub struct RawAccount {
    pub id: AccountId,
    pub currency: Currency,
    pub is_pooled: bool,
    pub created_at: SystemTime,
}

impl From<RawAccount> for Account {
    fn from(raw_account: RawAccount) -> Account {
        let RawAccount {
            id,
            currency,
            is_pooled,
            created_at,
        } = raw_account;

        Account {
            id,
            currency,
            is_pooled,
            created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[table_name = "accounts"]
pub struct NewAccount {
    pub id: AccountId,
    pub currency: Currency,
    pub is_pooled: bool,
}
