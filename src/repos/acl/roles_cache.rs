//! RolesCache is a module that caches received from db information about user and his roles

use failure::Fail;
use stq_cache::cache::Cache;
use stq_types::{BillingRole, UserId};

pub struct RolesCacheImpl<C>
where
    C: Cache<Vec<BillingRole>>,
{
    cache: C,
}

impl<C> RolesCacheImpl<C>
where
    C: Cache<Vec<BillingRole>>,
{
    pub fn new(cache: C) -> Self {
        RolesCacheImpl { cache }
    }

    pub fn get(&self, user_id: UserId) -> Option<Vec<BillingRole>> {
        debug!("Getting roles from RolesCache at key '{}'", user_id);

        self.cache.get(user_id.to_string().as_str()).unwrap_or_else(|err| {
            let err = err.context(format!("Failed to get roles from RolesCache at key '{}'", user_id));
            error!("{}", err);
            None
        })
    }

    pub fn remove(&self, user_id: UserId) -> bool {
        debug!("Removing roles from RolesCache at key '{}'", user_id);

        self.cache.remove(user_id.to_string().as_str()).unwrap_or_else(|err| {
            let err = err.context(format!("Failed to remove roles from RolesCache at key '{}'", user_id));
            error!("{}", err);
            false
        })
    }

    pub fn set(&self, user_id: UserId, roles: Vec<BillingRole>) {
        debug!("Setting roles in RolesCache at key '{}'", user_id);

        self.cache.set(user_id.to_string().as_str(), roles).unwrap_or_else(|err| {
            let err = err.context(format!("Failed to set roles in RolesCache at key '{}'", user_id));
            error!("{}", err);
        })
    }
}
