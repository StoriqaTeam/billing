use std::fmt::{self, Display};
use std::str::FromStr;

use diesel::sql_types::VarChar;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, FromSqlRow, AsExpression, Default, PartialOrd)]
#[sql_type = "VarChar"]
pub struct CustomerId(String);
derive_newtype_sql!(customer_id, VarChar, CustomerId, CustomerId);

impl CustomerId {
    pub fn new(v: String) -> Self {
        CustomerId(v)
    }

    pub fn inner(&self) -> String {
        self.0.clone()
    }
}

impl FromStr for CustomerId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(CustomerId::new(s.to_string()))
    }
}

impl Display for CustomerId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0,))
    }
}
