use models::{MerchantId, Order};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillingOrder {
    pub merchant_id: MerchantId,
    pub amount: f64,
    pub currency: String,
}

impl BillingOrder {
    pub fn new(order: Order, merchant_id: MerchantId) -> Self {
        Self {
            merchant_id,
            amount: order.price,
            currency: order.currency_id.to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateInvoicePayload {
    callback_url: String,
    currency: String,
    orders: Vec<BillingOrder>,
}

impl CreateInvoicePayload {
    pub fn new(orders: Vec<BillingOrder>, callback_url: String, currency: String) -> Self {
        Self {
            orders,
            callback_url,
            currency,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalBillingOrder {
    pub billing_url: String,
}
