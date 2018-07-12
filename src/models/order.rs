use stq_types::{CurrencyId, OrderId, SagaId, StoreId, UserId};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub store_id: StoreId,
    pub price: f64,
    pub currency_id: CurrencyId,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CreateInvoice {
    pub orders: Vec<Order>,
    pub customer_id: UserId,
    pub currency_id: CurrencyId,
    pub saga_id: SagaId,
}
