use stq_types::{RussiaBillingId, StoreId, SwiftId};

use schema::russia_billing_info;

#[derive(Clone, Serialize, Queryable, Insertable, Debug)]
#[table_name = "russia_billing_info"]
pub struct RussiaBillingInfo {
    pub id: RussiaBillingId,
    pub store_id: StoreId,
    pub bank_name: String,
    pub branch_name: Option<String>,
    pub swift_bic: SwiftId,
    pub tax_id: String,
    pub correspondent_account: String,
    pub current_account: String,
    pub personal_account: Option<String>,
    pub beneficiary_full_name: String,
}

#[derive(Serialize, Deserialize, Insertable, AsChangeset, Debug, Clone)]
#[table_name = "russia_billing_info"]
pub struct UpdateRussiaBillingInfo {
    pub branch_name: Option<String>,
    pub personal_account: Option<String>,
    pub bank_name: Option<String>,
    pub swift_bic: Option<SwiftId>,
    pub tax_id: Option<String>,
    pub correspondent_account: Option<String>,
    pub current_account: Option<String>,
    pub beneficiary_full_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "russia_billing_info"]
pub struct NewRussiaBillingInfo {
    pub store_id: StoreId,
    pub bank_name: String,
    pub branch_name: Option<String>,
    pub swift_bic: SwiftId,
    pub tax_id: String,
    pub correspondent_account: String,
    pub current_account: String,
    pub personal_account: Option<String>,
    pub beneficiary_full_name: String,
}

#[derive(Clone, Serialize, Debug, Default)]
pub struct RussiaBillingInfoSearch {
    pub id: Option<RussiaBillingId>,
    pub store_id: Option<StoreId>,
    pub store_ids: Option<Vec<StoreId>>,
}

impl RussiaBillingInfoSearch {
    pub fn by_id(id: RussiaBillingId) -> RussiaBillingInfoSearch {
        RussiaBillingInfoSearch {
            id: Some(id),
            ..Default::default()
        }
    }
    pub fn by_store_id(store_id: StoreId) -> RussiaBillingInfoSearch {
        RussiaBillingInfoSearch {
            store_id: Some(store_id),
            ..Default::default()
        }
    }
    pub fn by_store_ids(store_ids: Vec<StoreId>) -> RussiaBillingInfoSearch {
        RussiaBillingInfoSearch {
            store_ids: Some(store_ids),
            ..Default::default()
        }
    }
}
