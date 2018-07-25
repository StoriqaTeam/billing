use stq_static_resources::OrderState;
use stq_types::{OrderId, OrderInfoId, SagaId, StoreId, UserId};

use schema::orders_info;

#[derive(Serialize, Queryable, Insertable, Debug)]
#[table_name = "orders_info"]
pub struct OrderInfo {
    pub id: OrderInfoId,
    pub order_id: OrderId,
    pub store_id: StoreId,
    pub customer_id: UserId,
    pub saga_id: SagaId,
    pub status: OrderState,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "orders_info"]
pub struct NewOrderInfo {
    order_id: OrderId,
    customer_id: UserId,
    store_id: StoreId,
    saga_id: SagaId,
}

impl NewOrderInfo {
    pub fn new(order_id: OrderId, saga_id: SagaId, customer_id: UserId, store_id: StoreId) -> Self {
        Self {
            order_id,
            saga_id,
            customer_id,
            store_id,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable, AsChangeset)]
#[table_name = "orders_info"]
pub struct NewStatus {
    status: OrderState,
}

impl NewStatus {
    pub fn new(status: OrderState) -> Self {
        Self { status }
    }
}
