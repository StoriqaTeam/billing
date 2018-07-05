//! Permission is a tuple for describing permisssions

use models::{Action, Resource, Scope};

pub struct Permission {
    pub resource: Resource,
    pub action: Action,
    pub scope: Scope,
}
