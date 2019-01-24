use std::fmt::{self, Display};
use std::num::ParseIntError;
use std::str::FromStr;

use diesel::sql_types::Int4 as SqlInt4;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Default, PartialEq)]
#[sql_type = "SqlInt4"]
pub struct FeeId(i32);
derive_newtype_sql!(fee_id, SqlInt4, FeeId, FeeId);

impl FeeId {
    pub fn new(id: i32) -> Self {
        FeeId(id)
    }

    pub fn inner(&self) -> &i32 {
        &self.0
    }
}

impl FromStr for FeeId {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s.parse()?;
        Ok(FeeId::new(id))
    }
}

impl Display for FeeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0,))
    }
}
