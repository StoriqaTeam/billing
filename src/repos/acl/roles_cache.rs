//! RolesCache is a module that caches received from db information about user and his roles
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use models::authorization::*;

use models::UserId;

#[derive(Default, Clone)]
pub struct RolesCacheImpl {
    roles_cache: Arc<Mutex<HashMap<UserId, Vec<Role>>>>,
}

impl RolesCacheImpl {
    pub fn get(&self, user_id: UserId) -> Vec<Role> {
        let mut hash_map = self.roles_cache.lock().unwrap();
        match hash_map.entry(user_id) {
            Entry::Occupied(o) => o.get().clone(),
            Entry::Vacant(_) => vec![],
        }
    }

    pub fn clear(&self) {
        let mut hash_map = self.roles_cache.lock().unwrap();
        hash_map.clear();
    }

    pub fn remove(&self, user_id: UserId) {
        let mut hash_map = self.roles_cache.lock().unwrap();
        hash_map.remove(&user_id);
    }

    pub fn contains(&self, user_id: UserId) -> bool {
        let hash_map = self.roles_cache.lock().unwrap();
        hash_map.contains_key(&user_id)
    }

    pub fn add_roles(&self, user_id: UserId, roles: &[Role]) {
        let mut hash_map = self.roles_cache.lock().unwrap();
        hash_map.insert(user_id, roles.to_vec());
    }
}
