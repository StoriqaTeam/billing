use std::fmt;
use stq_static_resources::Currency as StqCurrency;
use stq_types::*;
use stq_types::{OrderId as StqOrderId, StoreId as StqStoreId, UserId as StqUserId};

use models::invoice_v2::InvoiceId;
use models::order_v2::{OrderId, StoreId};
use models::{Currency, UserId};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: StqOrderId,
    #[serde(rename = "store")]
    pub store_id: StqStoreId,
    pub price: ProductPrice,
    pub quantity: Quantity,
    pub currency: StqCurrency,
    pub total_amount: ProductPrice,
    pub product_cashback: Option<CashbackPercent>,
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Order - id : {}, store id: {}, price: {}, currency : {}",
            self.id, self.store_id, self.price, self.currency
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CreateInvoice {
    pub orders: Vec<Order>,
    pub customer_id: StqUserId,
    pub currency: StqCurrency,
    pub saga_id: SagaId,
}

impl fmt::Display for CreateInvoice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let orders_comma_separated = self.orders.iter().fold("".to_string(), |acc, i| format!("{}, {}", acc, i));
        write!(
            f,
            "Create invoice - orders: '{}'; customer id: {}, currency: {}, saga id : {}",
            orders_comma_separated, self.customer_id, self.currency, self.saga_id
        )
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct CreateOrderV2 {
    pub id: OrderId,
    #[serde(rename = "store")]
    pub store_id: StoreId,
    pub currency: Currency,
    pub total_amount: f64,
    pub product_cashback: Option<f64>,
}

impl fmt::Display for CreateOrderV2 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Order - id: {}, store id: {}, currency: {}, total amount: {}, cashback %: {:?}",
            self.id, self.store_id, self.currency, self.total_amount, self.product_cashback
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CreateInvoiceV2 {
    pub orders: Vec<CreateOrderV2>,
    pub customer_id: UserId,
    pub currency: Currency,
    pub saga_id: InvoiceId,
}

impl fmt::Display for CreateInvoiceV2 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let orders_comma_separated = self.orders.iter().fold("".to_string(), |acc, i| format!("{}, {}", acc, i));
        write!(
            f,
            "Create invoice - orders: '{}'; customer id: {}, currency: {}, saga id : {}",
            orders_comma_separated, self.customer_id, self.currency, self.saga_id
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ExternalBillingToken {
    pub token: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ExternalBillingCredentials {
    username: String,
    password: String,
}

impl ExternalBillingCredentials {
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
    }
}
