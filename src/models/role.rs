//! Models for managing Roles

use serde_json;

use stq_types::{RoleId, StoresRole, UserId};

table! {
    roles (id) {
        id -> Uuid,
        user_id -> Integer,
        name -> VarChar,
        data -> Nullable<Jsonb>,
    }
}

#[derive(Serialize, Queryable, Insertable, Debug)]
#[table_name = "roles"]
pub struct UserRole {
    pub id: RoleId,
    pub user_id: UserId,
    pub name: StoresRole,
    pub data: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "roles"]
pub struct NewUserRole {
    pub id: RoleId,
    pub user_id: UserId,
    pub name: StoresRole,
    pub data: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "roles"]
pub struct OldUserRole {
    pub user_id: UserId,
    pub name: StoresRole,
}
