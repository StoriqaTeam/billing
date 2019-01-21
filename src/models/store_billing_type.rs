use stq_types::{BillingType, StoreBillingTypeId, StoreId};

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

#[derive(Clone, Copy, Serialize, Debug)]
pub struct StoreBillingTypeSearch {
    pub id: Option<StoreBillingTypeId>,
    pub store_id: Option<StoreId>,
    pub billing_type: Option<BillingType>,
}
