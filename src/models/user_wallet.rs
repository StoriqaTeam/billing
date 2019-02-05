use chrono::NaiveDateTime;
use std::fmt;
use uuid::Uuid;

use models::{TureCurrency, UserId};
use schema::user_wallets;

#[derive(Clone, Copy, Debug, PartialEq, Eq, From, FromStr, Hash, Serialize, Deserialize, DieselTypes)]
pub struct UserWalletId(Uuid);

impl UserWalletId {
    pub fn new(id: Uuid) -> Self {
        UserWalletId(id)
    }

    pub fn inner(&self) -> &Uuid {
        &self.0
    }

    pub fn into_inner(self) -> Uuid {
        self.0
    }

    pub fn generate() -> Self {
        UserWalletId(Uuid::new_v4())
    }
}

impl fmt::Display for UserWalletId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0.hyphenated()))
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, From, FromStr, Hash, Serialize, Deserialize, DieselTypes)]
pub struct WalletAddress(String);

impl WalletAddress {
    pub fn new(address: String) -> Self {
        WalletAddress(address)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct UserWallet {
    pub id: UserWalletId,
    pub address: WalletAddress,
    pub currency: TureCurrency,
    pub user_id: UserId,
    pub created_at: NaiveDateTime,
}

#[derive(Clone, Debug, Deserialize, Insertable)]
#[table_name = "user_wallets"]
pub struct NewUserWallet {
    pub id: UserWalletId,
    pub address: WalletAddress,
    pub currency: TureCurrency,
    pub user_id: UserId,
}

#[derive(Clone, Debug, Serialize, Deserialize, Queryable)]
pub struct RawUserWallet {
    pub id: UserWalletId,
    pub address: WalletAddress,
    pub currency: TureCurrency,
    pub user_id: UserId,
    pub created_at: NaiveDateTime,
}

impl RawUserWallet {
    pub fn into_domain(self) -> UserWallet {
        let RawUserWallet {
            id,
            address,
            currency,
            user_id,
            created_at,
        } = self;

        UserWallet {
            id,
            address,
            currency,
            user_id,
            created_at,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UserWalletAccess {
    pub user_id: UserId,
}

impl From<&UserWallet> for UserWalletAccess {
    fn from(user_wallet: &UserWallet) -> UserWalletAccess {
        UserWalletAccess {
            user_id: user_wallet.user_id.clone(),
        }
    }
}

impl From<&NewUserWallet> for UserWalletAccess {
    fn from(new_user_wallet: &NewUserWallet) -> UserWalletAccess {
        UserWalletAccess {
            user_id: new_user_wallet.user_id.clone(),
        }
    }
}
