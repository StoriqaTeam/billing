use std::time::SystemTime;

use stq_types::{Alpha3, MerchantId, MerchantType, StoreId, UserId};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SubjectIdentifier {
    Store(StoreId),
    User(UserId),
}

#[derive(Serialize, Debug)]
pub struct Merchant {
    pub merchant_id: MerchantId,
    pub user_id: Option<UserId>,
    pub store_id: Option<StoreId>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub merchant_type: MerchantType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateUserMerchantPayload {
    pub id: UserId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStoreMerchantPayload {
    pub id: StoreId,
    pub country_code: Option<Alpha3>,
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
