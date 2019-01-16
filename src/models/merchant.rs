use std::fmt;

use std::time::SystemTime;

use stq_types::{MerchantId, MerchantType, StoreId, UserId};

use schema::merchants;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SubjectIdentifier {
    Store(StoreId),
    User(UserId),
}

#[derive(Serialize, Queryable, Insertable, Debug)]
#[table_name = "merchants"]
pub struct Merchant {
    pub merchant_id: MerchantId,
    pub user_id: Option<UserId>,
    pub store_id: Option<StoreId>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub merchant_type: MerchantType,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "merchants"]
pub struct NewStoreMerchant {
    merchant_id: MerchantId,
    user_id: Option<UserId>,
    store_id: Option<StoreId>,
    merchant_type: MerchantType,
}

impl fmt::Display for NewStoreMerchant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "New Store Merchant - merchant_id: '{}'; user id: {:?}, store id: {:?}, merchant_type : {:?}",
            self.merchant_id, self.user_id, self.store_id, self.merchant_type
        )
    }
}

impl NewStoreMerchant {
    pub fn new(merchant_id: MerchantId, store_id: StoreId) -> Self {
        Self {
            merchant_id,
            user_id: None,
            store_id: Some(store_id),
            merchant_type: MerchantType::Store,
        }
    }
    pub fn merchant_id(&self) -> &MerchantId {
        &self.merchant_id
    }
    pub fn user_id(&self) -> &Option<UserId> {
        &self.user_id
    }
    pub fn store_id(&self) -> &Option<StoreId> {
        &self.store_id
    }
    pub fn merchant_type(&self) -> &MerchantType {
        &self.merchant_type
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "merchants"]
pub struct NewUserMerchant {
    merchant_id: MerchantId,
    user_id: Option<UserId>,
    store_id: Option<StoreId>,
    merchant_type: MerchantType,
}

impl fmt::Display for NewUserMerchant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "New User Merchant - merchant_id: '{}'; user id: {:?}, store id: {:?}, merchant_type : {:?}",
            self.merchant_id, self.user_id, self.store_id, self.merchant_type
        )
    }
}

impl NewUserMerchant {
    pub fn new(merchant_id: MerchantId, user_id: UserId) -> Self {
        Self {
            merchant_id,
            user_id: Some(user_id),
            store_id: None,
            merchant_type: MerchantType::User,
        }
    }
    pub fn merchant_id(&self) -> &MerchantId {
        &self.merchant_id
    }
    pub fn user_id(&self) -> &Option<UserId> {
        &self.user_id
    }
    pub fn store_id(&self) -> &Option<StoreId> {
        &self.store_id
    }
    pub fn merchant_type(&self) -> &MerchantType {
        &self.merchant_type
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateUserMerchantPayload {
    pub id: UserId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStoreMerchantPayload {
    pub id: StoreId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalBillingMerchant {
    pub id: MerchantId,
    pub balance: Option<Vec<MerchantBalance>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerchantBalance {
    pub amount: f64,
    pub currency: String,
    pub status: MerchantBalanceStatus,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum MerchantBalanceStatus {
    Active,
    Blocked,
}
