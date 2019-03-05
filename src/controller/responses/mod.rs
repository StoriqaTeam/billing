use std::collections::HashMap;

use bigdecimal::{BigDecimal, ToPrimitive};
use chrono::NaiveDateTime;
use failure::Fail;
use stripe::{Card as StripeCard, CardBrand as StripeCardBrand};

use stq_types::{stripe::PaymentIntentId, StoreId as StqStoreId, SubscriptionPaymentId, UserId};

use models::{
    fee::FeeId,
    invoice_v2::InvoiceId,
    order_v2::{OrderId, RawOrder, StoreId},
    ChargeId, CustomerId, Fee, FeeStatus, PaymentIntent, PaymentIntentStatus, PaymentState, StoreSubscription, StoreSubscriptionStatus,
    SubscriptionPayment, SubscriptionPaymentSearchResults, SubscriptionPaymentStatus, TransactionId, WalletAddress,
};
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

#[derive(Debug, Clone, Serialize)]
pub struct OrderResponse {
    pub id: OrderId,
    pub seller_currency: StqCurrency,
    pub total_amount: f64,
    pub cashback_amount: f64,
    pub invoice_id: InvoiceId,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub store_id: StoreId,
    pub state: PaymentState,
    pub stripe_fee: Option<f64>,
}

impl OrderResponse {
    pub fn try_from_raw_order(raw_order: RawOrder) -> Result<Self, Error> {
        let total_amount = raw_order
            .total_amount
            .to_super_unit(raw_order.seller_currency)
            .to_f64()
            .ok_or(ectx!(try err ErrorContext::AmountConversion, ErrorKind::Internal))?;
        let cashback_amount = raw_order
            .cashback_amount
            .to_super_unit(raw_order.seller_currency)
            .to_f64()
            .ok_or(ectx!(try err ErrorContext::AmountConversion, ErrorKind::Internal))?;
        let stripe_fee = if let Some(s) = raw_order.stripe_fee {
            let s = s
                .to_super_unit(raw_order.seller_currency)
                .to_f64()
                .ok_or(ectx!(try err ErrorContext::AmountConversion, ErrorKind::Internal))?;
            Some(s)
        } else {
            None
        };

        Ok(OrderResponse {
            id: raw_order.id,
            seller_currency: raw_order.seller_currency.into(),
            total_amount,
            cashback_amount,
            invoice_id: raw_order.invoice_id,
            created_at: raw_order.created_at,
            updated_at: raw_order.updated_at,
            store_id: raw_order.store_id,
            state: raw_order.state,
            stripe_fee,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OrderSearchResultsResponse {
    pub total_count: i64,
    pub orders: Vec<OrderResponse>,
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

#[derive(Clone, Debug, Serialize)]
pub struct SubscriptionPaymentResponse {
    pub id: SubscriptionPaymentId,
    pub store_id: StqStoreId,
    pub amount: BigDecimal,
    pub currency: StqCurrency,
    pub charge_id: Option<ChargeId>,
    pub transaction_id: Option<TransactionId>,
    pub status: SubscriptionPaymentStatus,
    pub created_at: NaiveDateTime,
}

impl From<SubscriptionPayment> for SubscriptionPaymentResponse {
    fn from(subscription_payment: SubscriptionPayment) -> SubscriptionPaymentResponse {
        SubscriptionPaymentResponse {
            id: subscription_payment.id,
            store_id: subscription_payment.store_id,
            amount: subscription_payment.amount.to_super_unit(subscription_payment.currency),
            currency: subscription_payment.currency.into(),
            charge_id: subscription_payment.charge_id,
            transaction_id: subscription_payment.transaction_id,
            status: subscription_payment.status,
            created_at: subscription_payment.created_at,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct SubscriptionPaymentSearchResponse {
    pub total_count: i64,
    pub subscription_payments: Vec<SubscriptionPaymentResponse>,
}

impl From<SubscriptionPaymentSearchResults> for SubscriptionPaymentSearchResponse {
    fn from(data: SubscriptionPaymentSearchResults) -> Self {
        SubscriptionPaymentSearchResponse {
            total_count: data.total_count,
            subscription_payments: data
                .subscription_payments
                .into_iter()
                .map(SubscriptionPaymentResponse::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct StoreSubscriptionResponse {
    pub store_id: StqStoreId,
    pub currency: StqCurrency,
    pub value: BigDecimal,
    pub wallet_address: Option<WalletAddress>,
    pub trial_start_date: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub status: StoreSubscriptionStatus,
}

impl From<StoreSubscription> for StoreSubscriptionResponse {
    fn from(data: StoreSubscription) -> Self {
        StoreSubscriptionResponse {
            store_id: data.store_id,
            currency: data.currency.into(),
            value: data.value.to_super_unit(data.currency),
            wallet_address: data.wallet_address,
            trial_start_date: data.trial_start_date,
            created_at: data.created_at,
            updated_at: data.updated_at,
            status: data.status,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BalancesResponse {
    pub currencies: HashMap<StqCurrency, BigDecimal>,
}

impl BalancesResponse {
    pub fn new(currencies: HashMap<StqCurrency, BigDecimal>) -> Self {
        Self { currencies }
    }
}
