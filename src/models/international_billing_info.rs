use stq_types::{InternationalBillingId, StoreId, SwiftId};

use schema::international_billing_info;

#[derive(Clone, Serialize, Queryable, Insertable, Debug)]
#[table_name = "international_billing_info"]
pub struct InternationalBillingInfo {
    pub id: InternationalBillingId,
    pub store_id: StoreId,
    pub swift_bic: SwiftId,
    pub bank_name: String,
    pub full_name: String,
    pub iban: String,
}

#[derive(Serialize, Deserialize, Insertable, AsChangeset, Debug, Clone)]
#[table_name = "international_billing_info"]
pub struct UpdateInternationalBillingInfo {
    pub swift_bic: Option<SwiftId>,
    pub bank_name: Option<String>,
    pub full_name: Option<String>,
    pub iban: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "international_billing_info"]
pub struct NewInternationalBillingInfo {
    pub store_id: StoreId,
    pub swift_bic: SwiftId,
    pub bank_name: String,
    pub full_name: String,
    pub iban: String,
}

#[derive(Clone, Serialize, Debug, Default)]
pub struct InternationalBillingInfoSearch {
    pub id: Option<InternationalBillingId>,
    pub store_id: Option<StoreId>,
    pub swift_bic: Option<SwiftId>,
}
