use stq_types::{BillingType, StoreId};

use controller::responses::OrderResponse;
use models::order_v2::OrderId;
use models::{InternationalBillingInfo, PaymentState, ProxyCompanyBillingInfo, RussiaBillingInfo};

#[derive(Debug, Clone, Deserialize)]
pub struct OrderBillingSearchTerms {
    pub payment_state: Option<PaymentState>,
    pub store_id: Option<StoreId>,
    pub order_id: Option<OrderId>,
}

#[derive(Serialize, Debug, Clone)]
pub struct OrderBillingInfo {
    pub order: OrderResponse,
    pub billing_type: BillingType,
    pub proxy_company_billing_info: Option<ProxyCompanyBillingInfo>,
    pub russia_billing_info: Option<RussiaBillingInfo>,
    pub international_billing_info: Option<InternationalBillingInfo>,
}

#[derive(Serialize, Clone, Debug)]
pub struct OrderBillingInfoSearchResults {
    pub total_count: i64,
    pub orders: Vec<OrderBillingInfo>,
}
