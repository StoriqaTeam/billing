use stq_types::{RussiaBillingId, StoreId};

use schema::russia_billing_info;

#[derive(Clone, Serialize, Queryable, Insertable, Debug)]
#[table_name = "russia_billing_info"]
pub struct RussiaBillingInfo {
    pub id: RussiaBillingId,
    pub store_id: StoreId,
    pub kpp: String,
    pub bic: String,
    pub inn: String,
    pub full_name: String,
}

#[derive(Serialize, Deserialize, Insertable, AsChangeset, Debug, Clone)]
#[table_name = "russia_billing_info"]
pub struct UpdateRussiaBillingInfo {
    pub kpp: Option<String>,
    pub bic: Option<String>,
    pub inn: Option<String>,
    pub full_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "russia_billing_info"]
pub struct NewRussiaBillingInfo {
    pub store_id: StoreId,
    pub kpp: String,
    pub bic: String,
    pub inn: String,
    pub full_name: String,
}

#[derive(Clone, Serialize, Debug, Default)]
pub struct RussiaBillingInfoSearch {
    pub id: Option<RussiaBillingId>,
    pub store_id: Option<StoreId>,
}
