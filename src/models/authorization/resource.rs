//! Enum for resources available in ACLs
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resource {
    Account,
    BillingInfo,
    OrderInfo,
    UserRoles,
    Invoice,
    OrderExchangeRate,
    PaymentIntent,
    ProxyCompanyBillingInfo,
    StoreBillingType,
    Subscription,
    StoreSubscription,
    StoreSubscriptionStatus,
    SubscriptionPayment,
    Customer,
    Fee,
    PaymentIntentInvoice,
    PaymentIntentFee,
    UserWallet,
    Payout,
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Resource::Account => write!(f, "account"),
            Resource::OrderInfo => write!(f, "order info"),
            Resource::UserRoles => write!(f, "user roles"),
            Resource::Invoice => write!(f, "invoice"),
            Resource::BillingInfo => write!(f, "billing info"),
            Resource::OrderExchangeRate => write!(f, "order exchange rate"),
            Resource::PaymentIntent => write!(f, "payment intent"),
            Resource::ProxyCompanyBillingInfo => write!(f, "proxy company billing info"),
            Resource::StoreBillingType => write!(f, "store billing type"),
            Resource::Subscription => write!(f, "subscription"),
            Resource::StoreSubscription => write!(f, "store subscription"),
            Resource::StoreSubscriptionStatus => write!(f, "store subscription status"),
            Resource::SubscriptionPayment => write!(f, "subscription payment"),
            Resource::Customer => write!(f, "customer"),
            Resource::Fee => write!(f, "fee"),
            Resource::PaymentIntentInvoice => write!(f, "payment_intent_invoice"),
            Resource::PaymentIntentFee => write!(f, "payment_intent_fee"),
            Resource::UserWallet => write!(f, "user wallet"),
            Resource::Payout => write!(f, "payout"),
        }
    }
}
