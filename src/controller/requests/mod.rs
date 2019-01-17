#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewCustomerWithSourceRequest {
    pub email: Option<String>,
    pub card_token: String,
}
