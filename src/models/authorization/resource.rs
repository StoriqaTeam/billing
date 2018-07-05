//! Enum for resources available in ACLs
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resource {
    OrderInfo,
    UserRoles,
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Resource::OrderInfo => write!(f, "order info"),
            Resource::UserRoles => write!(f, "user roles"),
        }
    }
}
