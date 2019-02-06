use std::fmt::{self, Display};
use std::str::FromStr;

use diesel::sql_types::VarChar;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, FromSqlRow, AsExpression, Default, PartialOrd)]
#[sql_type = "VarChar"]
pub struct StripePayoutId(String);
derive_newtype_sql!(payout_id, VarChar, StripePayoutId, StripePayoutId);

impl StripePayoutId {
    pub fn new(v: String) -> Self {
        StripePayoutId(v)
    }

    pub fn inner(&self) -> String {
        self.0.clone()
    }
}

impl FromStr for StripePayoutId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(StripePayoutId::new(s.to_string()))
    }
}

impl Display for StripePayoutId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0,))
    }
}
