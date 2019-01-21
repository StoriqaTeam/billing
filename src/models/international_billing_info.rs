use stq_types::{InternationalBillingId, StoreId, SwiftId, UserId};

use schema::international_billing_info;

#[derive(Clone, Serialize, Queryable, Insertable, Debug)]
#[table_name = "international_billing_info"]
pub struct InternationalBillingInfo {
    pub id: InternationalBillingId,
    pub store_id: StoreId,
    pub user_id: UserId,
    pub swift_id: Option<SwiftId>,
}

#[derive(Serialize, Deserialize, Insertable, AsChangeset, Debug, Clone)]
#[table_name = "international_billing_info"]
pub struct UpdateInternationalBillingInfo {
    pub swift_id: Option<SwiftId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "international_billing_info"]
pub struct NewInternationalBillingInfo {
    pub store_id: StoreId,
    pub user_id: UserId,
    pub swift_id: Option<SwiftId>,
}

#[derive(Clone, Serialize, Debug, Default)]
pub struct InternationalBillingInfoSearch {
    pub id: Option<InternationalBillingId>,
    pub store_id: Option<StoreId>,
    pub user_id: Option<UserId>,
    pub swift_id: Option<SwiftId>,
}
