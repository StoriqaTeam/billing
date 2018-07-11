//! Enum for roles available in ACLs

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone, DieselTypes)]
pub enum Role {
    Superuser,
    User,
    StoreManager,
}
