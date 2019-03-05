use stq_static_resources::Currency as StqCurrency;

use models::order_v2::OrderId as Orderv2Id;
use models::{CreateStoreSubscription, CustomerId, NewSubscription, PaymentState, StoreSubscriptionStatus, UpdateStoreSubscription};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewCustomerWithSourceRequest {
    pub email: Option<String>,
    pub card_token: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeleteCustomerRequest {
    pub customer_id: CustomerId,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCustomerRequest {
    pub email: Option<String>,
    pub card_token: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OrderPaymentStateRequest {
    pub state: PaymentState,
}

#[derive(Deserialize, Debug, Clone)]
pub struct FeesPayByOrdersRequest {
    pub order_ids: Vec<Orderv2Id>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSubscriptionsRequest {
    pub subscriptions: Vec<NewSubscription>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateStoreSubscriptionRequest {
    pub currency: StqCurrency,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateStoreSubscriptionRequest {
    pub currency: Option<StqCurrency>,
    pub status: Option<StoreSubscriptionStatus>,
}

impl From<UpdateStoreSubscriptionRequest> for UpdateStoreSubscription {
    fn from(data: UpdateStoreSubscriptionRequest) -> Self {
        UpdateStoreSubscription {
            currency: data.currency.map(|c| c.into()),
            status: data.status,
            ..Default::default()
        }
    }
}

impl From<CreateStoreSubscriptionRequest> for CreateStoreSubscription {
    fn from(data: CreateStoreSubscriptionRequest) -> Self {
        CreateStoreSubscription {
            currency: data.currency.into(),
        }
    }
}
