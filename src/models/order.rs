use std::fmt;

use stq_static_resources::Currency;

use models::{OrderId, SagaId, UserId};

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

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize, DieselTypes)]
pub struct StoreId(pub i32);

#[derive(Clone, Copy, Debug, Default, FromStr, Eq, PartialEq, Hash, Serialize, Deserialize, DieselTypes)]
pub struct CurrencyId(pub i32);

impl fmt::Display for CurrencyId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self.0 {
                x if x == Currency::Euro as i32 => Currency::Euro.to_string(),
                x if x == Currency::Dollar as i32 => Currency::Dollar.to_string(),
                x if x == Currency::Bitcoin as i32 => Currency::Bitcoin.to_string(),
                x if x == Currency::Etherium as i32 => Currency::Etherium.to_string(),
                x if x == Currency::Stq as i32 => Currency::Stq.to_string(),
                _ => "".to_string(),
            }
        )
    }
}
