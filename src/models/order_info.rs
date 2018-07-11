use uuid::Uuid;

use stq_static_resources::OrderStatus;

use super::{StoreId, UserId};

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

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize, DieselTypes)]
pub struct OrderId(pub Uuid);

impl OrderId {
    pub fn new() -> Self {
        OrderId(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize, DieselTypes)]
pub struct OrderInfoId(pub Uuid);

impl OrderInfoId {
    pub fn new() -> Self {
        OrderInfoId(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize, DieselTypes)]
pub struct CallbackId(pub Uuid);

impl CallbackId {
    pub fn new() -> Self {
        CallbackId(Uuid::new_v4())
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
    pub status: OrderStatus,
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
    status: OrderStatus,
}

impl SetOrderInfoPaid {
    pub fn new() -> Self {
        Self { status: OrderStatus::Paid }
    }
}
