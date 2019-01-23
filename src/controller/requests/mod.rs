use models::PaymentState;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewCustomerWithSourceRequest {
    pub email: Option<String>,
    pub card_token: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OrderPaymentStateRequest {
    pub state: PaymentState,
}
