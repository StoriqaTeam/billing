use models::{StoreId, UserId};
use uuid::Uuid;

table! {
    merchants (merchant_id) {
        merchant_id -> Uuid,
        user_id -> Nullable<Integer>,
        store_id -> Nullable<Integer>,
        #[sql_name = "type"]
        merchant_type -> VarChar,
    }
}

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize, DieselTypes)]
pub struct MerchantId(pub Uuid);

impl MerchantId {
    pub fn new() -> Self {
        MerchantId(Uuid::new_v4())
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq, Hash, DieselTypes)]
pub enum MerchantType {
    Store,
    User,
}

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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerchantBalance {
    pub id: MerchantId,
    pub amount: f64,
    pub currency: String,
}
