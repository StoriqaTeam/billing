//! Models for managing Roles

use serde_json;
use uuid::Uuid;

use models::Role;

table! {
    user_roles (id) {
        id -> Uuid,
        user_id -> Integer,
        role -> VarChar,
        data -> Nullable<Jsonb>,
    }
}

#[derive(Clone, Copy, Debug, Display, FromStr, PartialEq, Hash, Serialize, Deserialize, DieselTypes)]
pub struct RoleId(pub Uuid);

impl RoleId {
    pub fn new() -> Self {
        RoleId(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, Display, FromStr, PartialEq, Hash, Serialize, Deserialize, Eq, DieselTypes)]
pub struct UserId(pub i32);

#[derive(Serialize, Queryable, Insertable, Debug)]
#[table_name = "user_roles"]
pub struct UserRole {
    pub id: RoleId,
    pub user_id: UserId,
    pub role: Role,
    pub data: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "user_roles"]
pub struct NewUserRole {
    pub id: RoleId,
    pub user_id: UserId,
    pub role: Role,
    pub data: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "user_roles"]
pub struct OldUserRole {
    pub user_id: UserId,
    pub role: Role,
}
