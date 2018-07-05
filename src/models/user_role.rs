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

#[derive(Clone, Copy, Debug, Display, FromStr, PartialEq, Hash, Serialize, Deserialize)]
pub struct RoleId(pub Uuid);

impl RoleId {
    pub fn new() -> Self {
        RoleId(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, Display, FromStr, PartialEq, Hash, Serialize, Deserialize, Eq)]
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

mod diesel_impl {
    use diesel::deserialize::FromSql;
    use diesel::deserialize::FromSqlRow;
    use diesel::expression::bound::Bound;
    use diesel::expression::AsExpression;
    use diesel::pg::Pg;
    use diesel::row::Row;
    use diesel::serialize::Output;
    use diesel::serialize::{IsNull, ToSql};
    use diesel::sql_types::Uuid as SqlUuid;
    use diesel::sql_types::*;
    use diesel::Queryable;
    use std::error::Error;
    use std::io::Write;

    use uuid::Uuid;

    use super::RoleId;

    impl<'a> AsExpression<SqlUuid> for &'a RoleId {
        type Expression = Bound<SqlUuid, &'a RoleId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl AsExpression<SqlUuid> for RoleId {
        type Expression = Bound<SqlUuid, RoleId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl ToSql<SqlUuid, Pg> for RoleId {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            out.write_all(self.0.as_bytes())?;
            Ok(IsNull::No)
        }
    }

    impl FromSqlRow<SqlUuid, Pg> for RoleId {
        fn build_from_row<T: Row<Pg>>(row: &mut T) -> Result<Self, Box<Error + Send + Sync>> {
            let uuid = match row.take() {
                Some(id) => Uuid::from_bytes(id)?,
                None => Uuid::nil(),
            };
            Ok(RoleId(uuid))
        }
    }

    impl Queryable<SqlUuid, Pg> for RoleId {
        type Row = Self;

        fn build(row: Self::Row) -> Self {
            row
        }
    }

    use super::UserId;

    impl<'a> AsExpression<Integer> for &'a UserId {
        type Expression = Bound<Integer, &'a UserId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl AsExpression<Integer> for UserId {
        type Expression = Bound<Integer, UserId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl ToSql<Integer, Pg> for UserId {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            ToSql::<Integer, Pg>::to_sql(&self.0, out)
        }
    }

    impl FromSqlRow<Integer, Pg> for UserId {
        fn build_from_row<T: Row<Pg>>(row: &mut T) -> Result<Self, Box<Error + Send + Sync>> {
            FromSql::<Integer, Pg>::from_sql(row.take()).map(UserId)
        }
    }

    impl Queryable<Integer, Pg> for UserId {
        type Row = Self;

        fn build(row: Self::Row) -> Self {
            row
        }
    }
}
