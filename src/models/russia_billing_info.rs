use stq_types::{RussiaBillingId, StoreId, UserId};

use schema::russia_billing_info;

#[derive(Clone, Serialize, Queryable, Insertable, Debug)]
#[table_name = "russia_billing_info"]
pub struct RussiaBillingInfo {
    pub id: RussiaBillingId,
    pub store_id: StoreId,
    pub user_id: UserId,
    pub kpp: Option<String>,
    pub bic: Option<String>,
}

#[derive(Serialize, Deserialize, Insertable, AsChangeset, Debug, Clone)]
#[table_name = "russia_billing_info"]
pub struct UpdateRussiaBillingInfo {
    pub kpp: Option<String>,
    pub bic: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "russia_billing_info"]
pub struct NewRussiaBillingInfo {
    pub store_id: StoreId,
    pub user_id: UserId,
    pub kpp: Option<String>,
    pub bic: Option<String>,
}

#[derive(Clone, Serialize, Debug, Default)]
pub struct RussiaBillingInfoSearch {
    pub id: Option<RussiaBillingId>,
    pub store_id: Option<StoreId>,
    pub user_id: Option<UserId>,
}
