//! Action enum for authorization
use std::fmt;

// All gives all permissions.
// Index - list resources, Read - read resource with id,
// Write - Update or delete resource with id.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    All,
    Read,
    Write,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Action::All => write!(f, "all"),
            Action::Read => write!(f, "read"),
            Action::Write => write!(f, "write"),
        }
    }
}
