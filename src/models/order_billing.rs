use stq_types::{BillingType, StoreId};

use models::order_v2::RawOrder;
use models::{InternationalBillingInfo, PaymentState, ProxyCompanyBillingInfo, RussiaBillingInfo};

#[derive(Debug, Clone, Deserialize)]
pub struct OrderBillingSearchTerms {
    pub payment_state: Option<PaymentState>,
    pub store_id: Option<StoreId>,
}

#[derive(Serialize, Debug, Clone)]
pub struct OrderBilling {
    pub order: RawOrder,
    pub billing_type: BillingType,
    pub proxy_company_billing_info: Option<ProxyCompanyBillingInfo>,
    pub russia_billing_info: Option<RussiaBillingInfo>,
    pub international_billing_info: Option<InternationalBillingInfo>,
}

#[derive(Serialize, Clone, Debug)]
pub struct OrderBillingSearchResults {
    pub total_count: i64,
    pub orders: Vec<OrderBilling>,
}
