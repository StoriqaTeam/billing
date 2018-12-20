//! Enum for resources available in ACLs
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resource {
    Account,
    OrderInfo,
    UserRoles,
    Merchant,
    Invoice,
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Resource::Account => write!(f, "account"),
            Resource::OrderInfo => write!(f, "order info"),
            Resource::UserRoles => write!(f, "user roles"),
            Resource::Merchant => write!(f, "merchant"),
            Resource::Invoice => write!(f, "invoice"),
        }
    }
}
