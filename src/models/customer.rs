use chrono::NaiveDateTime;

use models::CustomerId;
use stq_types::UserId;

use schema::customers;

#[derive(Clone, Debug, Deserialize, Serialize, Queryable)]
pub struct DbCustomer {
    pub id: CustomerId,
    pub user_id: UserId,
    pub email: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Clone, Debug, Deserialize, Serialize, Queryable, Insertable)]
#[table_name = "customers"]
pub struct NewDbCustomer {
    pub id: CustomerId,
    pub user_id: UserId,
    pub email: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, AsChangeset)]
#[table_name = "customers"]
pub struct UpdateDbCustomer {
    pub email: Option<String>,
}

pub struct CustomersAccess {
    pub user_id: UserId,
}

impl<'r> From<&'r DbCustomer> for CustomersAccess {
    fn from(other: &DbCustomer) -> Self {
        Self {
            user_id: other.user_id.clone(),
        }
    }
}
