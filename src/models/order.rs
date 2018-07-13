use std::fmt;
use stq_types::{CurrencyId, OrderId, SagaId, StoreId, UserId};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub store_id: StoreId,
    pub price: f64,
    pub currency_id: CurrencyId,
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Order - id : {}, store id: {}, price: {}, currency id : {}",
            self.id, self.store_id, self.price, self.currency_id
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CreateInvoice {
    pub orders: Vec<Order>,
    pub customer_id: UserId,
    pub currency_id: CurrencyId,
    pub saga_id: SagaId,
}

impl fmt::Display for CreateInvoice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let orders_comma_separated = self.orders.iter().fold("".to_string(), |acc, i| format!("{}, {}", acc, i));
        write!(
            f,
            "Create invoice - orders: '{}'; customer id: {}, currency id: {}, saga id : {}",
            orders_comma_separated, self.customer_id, self.currency_id, self.saga_id
        )
    }
}
