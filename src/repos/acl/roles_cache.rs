//! RolesCache is a module that caches received from db information about user and his roles
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use models::authorization::*;

#[derive(Default, Clone)]
pub struct RolesCacheImpl {
    roles_cache: Arc<Mutex<HashMap<i32, Vec<Role>>>>,
}

impl RolesCacheImpl {
    pub fn get(&self, user_id: i32) -> Vec<Role> {
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

    pub fn remove(&self, id: i32) {
        let mut hash_map = self.roles_cache.lock().unwrap();
        hash_map.remove(&id);
    }

    pub fn contains(&self, id: i32) -> bool {
        let hash_map = self.roles_cache.lock().unwrap();
        hash_map.contains_key(&id)
    }

    pub fn add_roles(&self, id: i32, roles: &[Role]) {
        let mut hash_map = self.roles_cache.lock().unwrap();
        hash_map.insert(id, roles.to_vec());
    }
}
