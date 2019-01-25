use bigdecimal::ToPrimitive;
use failure::Fail;
use stripe::{Card as StripeCard, CardBrand as StripeCardBrand};

use stq_types::{stripe::PaymentIntentId, UserId};

use models::{fee::FeeId, order_v2::OrderId, ChargeId, CustomerId, Fee, FeeStatus, PaymentIntent, PaymentIntentStatus};
use stq_static_resources::Currency as StqCurrency;

use services::error::{Error, ErrorContext, ErrorKind};

#[derive(Debug, Deserialize, Serialize)]
pub struct PaymentIntentResponse {
    pub id: PaymentIntentId,
    pub amount: f64,
    pub amount_received: f64,
    pub client_secret: Option<String>,
    pub currency: StqCurrency,
    pub last_payment_error_message: Option<String>,
    pub receipt_email: Option<String>,
    pub charge_id: Option<ChargeId>,
    pub status: PaymentIntentStatus,
}

impl PaymentIntentResponse {
    pub fn try_from_payment_intent(other: PaymentIntent) -> Result<Self, Error> {
        let other_amount = other.amount.to_super_unit(other.currency).to_f64();
        let other_amount_received = other.amount_received.to_super_unit(other.currency).to_f64();

        match (other_amount, other_amount_received) {
            (Some(amount), Some(amount_received)) => Ok(Self {
                id: other.id,
                amount,
                amount_received,
                client_secret: other.client_secret,
                currency: other.currency.into(),
                last_payment_error_message: other.last_payment_error_message,
                receipt_email: other.receipt_email,
                charge_id: other.charge_id,
                status: other.status,
            }),
            _ => Err(ectx!(err ErrorContext::AmountConversion, ErrorKind::Internal)),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomerResponse {
    pub id: CustomerId,
    pub user_id: UserId,
    pub email: Option<String>,
    pub cards: Vec<Card>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Card {
    pub id: String,
    pub brand: CardBrand,
    pub country: String,
    pub customer: Option<String>,
    pub exp_month: u32,
    pub exp_year: u32,
    pub last4: String,
    pub name: Option<String>,
}

impl From<StripeCard> for Card {
    fn from(other: StripeCard) -> Self {
        Self {
            id: other.id,
            brand: other.brand.into(),
            country: other.country,
            customer: other.customer,
            exp_month: other.exp_month,
            exp_year: other.exp_year,
            last4: other.last4,
            name: other.name,
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Eq)]
pub enum CardBrand {
    AmericanExpress,
    DinersClub,
    Discover,
    JCB,
    Visa,
    MasterCard,
    UnionPay,
    #[serde(other)]
    Unknown,
}

impl From<StripeCardBrand> for CardBrand {
    fn from(other: StripeCardBrand) -> Self {
        match other {
            StripeCardBrand::AmericanExpress => CardBrand::AmericanExpress,
            StripeCardBrand::DinersClub => CardBrand::DinersClub,
            StripeCardBrand::Discover => CardBrand::Discover,
            StripeCardBrand::JCB => CardBrand::JCB,
            StripeCardBrand::Visa => CardBrand::Visa,
            StripeCardBrand::MasterCard => CardBrand::MasterCard,
            StripeCardBrand::UnionPay => CardBrand::UnionPay,
            StripeCardBrand::Unknown => CardBrand::Unknown,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FeeResponse {
    pub id: FeeId,
    pub order_id: OrderId,
    pub amount: f64,
    pub status: FeeStatus,
    pub currency: StqCurrency,
    pub charge_id: Option<ChargeId>,
    pub metadata: Option<serde_json::Value>,
}

impl FeeResponse {
    pub fn try_from_fee(other: Fee) -> Result<Self, Error> {
        let other_amount = other.amount.to_super_unit(other.currency).to_f64();

        match other_amount {
            Some(amount) => Ok(Self {
                id: other.id,
                order_id: other.order_id,
                amount,
                status: other.status,
                currency: other.currency.into(),
                charge_id: other.charge_id,
                metadata: other.metadata,
            }),
            _ => Err(ectx!(err ErrorContext::AmountConversion, ErrorKind::Internal)),
        }
    }
}
