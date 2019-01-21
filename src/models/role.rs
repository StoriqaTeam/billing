//! Models for managing Roles

use serde_json;

use stq_types::{BillingRole, RoleId, UserId};

use schema::roles;

#[derive(Serialize, Queryable, Insertable, Debug)]
#[table_name = "roles"]
pub struct UserRole {
    pub id: RoleId,
    pub user_id: UserId,
    pub name: BillingRole,
    pub data: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "roles"]
pub struct NewUserRole {
    pub id: RoleId,
    pub user_id: UserId,
    pub name: BillingRole,
    pub data: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RemoveUserRole {
    pub user_id: UserId,
    pub name: BillingRole,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "roles"]
pub struct OldUserRole {
    pub user_id: UserId,
    pub name: BillingRole,
}
