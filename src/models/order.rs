use std::fmt;
use stq_static_resources::Currency;
use stq_types::*;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    #[serde(rename = "store")]
    pub store_id: StoreId,
    pub price: ProductPrice,
    pub quantity: Quantity,
    pub currency: Currency,
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
    pub customer_id: UserId,
    pub currency: Currency,
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
