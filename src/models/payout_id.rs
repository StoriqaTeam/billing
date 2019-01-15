use std::fmt::{self, Display};
use std::str::FromStr;

use diesel::sql_types::VarChar;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, FromSqlRow, AsExpression, Default, PartialOrd)]
#[sql_type = "VarChar"]
pub struct PayOutId(String);
derive_newtype_sql!(payout_id, VarChar, PayOutId, PayOutId);

impl PayOutId {
    pub fn new(v: String) -> Self {
        PayOutId(v)
    }

    pub fn inner(&self) -> String {
        self.0.clone()
    }
}

impl FromStr for PayOutId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PayOutId::new(s.to_string()))
    }
}

impl Display for PayOutId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0,))
    }
}
