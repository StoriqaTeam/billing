//! Enum for resources available in ACLs
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resource {
    Account,
    BillingInfo,
    OrderInfo,
    UserRoles,
    Merchant,
    Invoice,
    OrderExchangeRate,
    PaymentIntent,
    ProxyCompanyBillingInfo,
    StoreBillingType,
    Customer,
    Fee,
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Resource::Account => write!(f, "account"),
            Resource::OrderInfo => write!(f, "order info"),
            Resource::UserRoles => write!(f, "user roles"),
            Resource::Merchant => write!(f, "merchant"),
            Resource::Invoice => write!(f, "invoice"),
            Resource::BillingInfo => write!(f, "billing info"),
            Resource::OrderExchangeRate => write!(f, "order exchange rate"),
            Resource::PaymentIntent => write!(f, "payment intent"),
            Resource::ProxyCompanyBillingInfo => write!(f, "proxy company billing info"),
            Resource::StoreBillingType => write!(f, "store billing type"),
            Resource::Customer => write!(f, "customer"),
            Resource::Fee => write!(f, "fee"),
        }
    }
}
