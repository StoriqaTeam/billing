use std::fmt::{self, Display};
use std::str::FromStr;

use diesel::sql_types::VarChar;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, FromSqlRow, AsExpression, Default, PartialOrd)]
#[sql_type = "VarChar"]
pub struct ChargeId(String);
derive_newtype_sql!(charge_id, VarChar, ChargeId, ChargeId);

impl ChargeId {
    pub fn new(v: String) -> Self {
        ChargeId(v)
    }

    pub fn inner(&self) -> String {
        self.0.clone()
    }
}

impl FromStr for ChargeId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ChargeId::new(s.to_string()))
    }
}

impl Display for ChargeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0,))
    }
}
