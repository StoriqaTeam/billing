use stq_types::{Alpha3, BillingType, StoreBillingTypeId, StoreId};

use schema::store_billing_type;

#[derive(Clone, Copy, Serialize, Queryable, Insertable, Debug)]
#[table_name = "store_billing_type"]
pub struct StoreBillingType {
    pub id: StoreBillingTypeId,
    pub store_id: StoreId,
    pub billing_type: BillingType,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "store_billing_type"]
pub struct NewStoreBillingType {
    pub store_id: StoreId,
    pub billing_type: BillingType,
}

#[derive(Clone, Serialize, Debug, Default)]
pub struct StoreBillingTypeSearch {
    pub id: Option<StoreBillingTypeId>,
    pub store_id: Option<StoreId>,
    pub store_ids: Option<Vec<StoreId>>,
    pub billing_type: Option<BillingType>,
}

#[derive(Serialize, Deserialize, Insertable, AsChangeset, Debug, Clone, Default)]
#[table_name = "store_billing_type"]
pub struct UpdateStoreBillingType {
    pub store_id: Option<StoreId>,
    pub billing_type: Option<BillingType>,
}

impl StoreBillingTypeSearch {
    pub fn by_store_id(store_id: StoreId) -> StoreBillingTypeSearch {
        StoreBillingTypeSearch {
            store_id: Some(store_id),
            ..Default::default()
        }
    }

    pub fn by_store_ids(store_ids: Vec<StoreId>) -> StoreBillingTypeSearch {
        StoreBillingTypeSearch {
            store_ids: Some(store_ids),
            ..Default::default()
        }
    }
}

pub fn country_to_billing_type(country: &Alpha3) -> BillingType {
    match country.0.as_ref() {
        "RUS" => BillingType::Russia,
        _ => BillingType::International,
    }
}
