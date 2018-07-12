use stq_static_resources::OrderState;
use stq_types::{CallbackId, OrderId, OrderInfoId, StoreId, UserId};

table! {
    order_info (id) {
        id -> Uuid,
        order_id -> Uuid,
        store_id -> Integer,
        customer_id -> Integer,
        callback_id -> Uuid,
        status -> VarChar,
    }
}

#[derive(Serialize, Queryable, Insertable, Debug)]
#[table_name = "order_info"]
pub struct OrderInfo {
    pub id: OrderInfoId,
    pub order_id: OrderId,
    pub store_id: StoreId,
    pub customer_id: UserId,
    pub callback_id: CallbackId,
    pub status: OrderState,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "order_info"]
pub struct NewOrderInfo {
    order_id: OrderId,
    callback_id: CallbackId,
    customer_id: UserId,
    store_id: StoreId,
}

impl NewOrderInfo {
    pub fn new(order_id: OrderId, callback_id: CallbackId, customer_id: UserId, store_id: StoreId) -> Self {
        Self {
            order_id,
            callback_id,
            customer_id,
            store_id,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable, AsChangeset)]
#[table_name = "order_info"]
pub struct SetOrderInfoPaid {
    status: OrderState,
}

impl SetOrderInfoPaid {
    pub fn new() -> Self {
        Self { status: OrderState::Paid }
    }
}
