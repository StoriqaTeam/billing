use stq_static_resources::Currency;
use stq_types::{InternationalBillingId, StoreId, SwiftId};

use schema::international_billing_info;

#[derive(Clone, Serialize, Queryable, Insertable, Debug)]
#[table_name = "international_billing_info"]
pub struct InternationalBillingInfo {
    pub id: InternationalBillingId,
    pub store_id: StoreId,
    pub account: String,
    pub currency: Currency,
    pub name: String,
    pub bank: String,
    pub swift: SwiftId,
    pub bank_address: String,
    pub country: String,
    pub city: String,
    pub recipient_address: String,
}

#[derive(Serialize, Deserialize, Insertable, AsChangeset, Debug, Clone)]
#[table_name = "international_billing_info"]
pub struct UpdateInternationalBillingInfo {
    pub account: Option<String>,
    pub currency: Option<Currency>,
    pub name: Option<String>,
    pub bank: Option<String>,
    pub swift: Option<SwiftId>,
    pub bank_address: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub recipient_address: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "international_billing_info"]
pub struct NewInternationalBillingInfo {
    pub store_id: StoreId,
    pub account: String,
    pub currency: Currency,
    pub name: String,
    pub bank: String,
    pub swift: SwiftId,
    pub bank_address: String,
    pub country: String,
    pub city: String,
    pub recipient_address: String,
}

#[derive(Clone, Serialize, Debug, Default)]
pub struct InternationalBillingInfoSearch {
    pub id: Option<InternationalBillingId>,
    pub store_id: Option<StoreId>,
    pub swift: Option<SwiftId>,
    pub store_ids: Option<Vec<StoreId>>,
}

impl InternationalBillingInfoSearch {
    pub fn by_id(id: InternationalBillingId) -> InternationalBillingInfoSearch {
        InternationalBillingInfoSearch {
            id: Some(id),
            ..Default::default()
        }
    }

    pub fn by_store_id(store_id: StoreId) -> InternationalBillingInfoSearch {
        InternationalBillingInfoSearch {
            store_id: Some(store_id),
            ..Default::default()
        }
    }

    pub fn by_store_ids(store_ids: Vec<StoreId>) -> InternationalBillingInfoSearch {
        InternationalBillingInfoSearch {
            store_ids: Some(store_ids),
            ..Default::default()
        }
    }
}
