use stq_static_resources::OrderState;

use models::{
    order_v2::{OrderId, StoreId},
    UserId,
};

#[derive(Debug, Clone, Serialize)]
pub struct OrderStateUpdate {
    pub order_id: OrderId,
    pub store_id: StoreId,
    pub customer_id: UserId,
    pub status: OrderState,
}
