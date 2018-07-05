//! Enum for roles available in ACLs

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub enum Role {
    Superuser,
    User,
    StoreManager,
}

mod diesel_impl {
    use std::error::Error;
    use std::io::Write;
    use std::str;

    use diesel::deserialize::Queryable;
    use diesel::expression::bound::Bound;
    use diesel::expression::AsExpression;
    use diesel::pg::Pg;
    use diesel::row::Row;
    use diesel::serialize::Output;
    use diesel::sql_types::VarChar;
    use diesel::types::{FromSqlRow, IsNull, NotNull, SingleValue, ToSql};

    use super::Role;

    impl NotNull for Role {}
    impl SingleValue for Role {}

    impl FromSqlRow<VarChar, Pg> for Role {
        fn build_from_row<R: Row<Pg>>(row: &mut R) -> Result<Self, Box<Error + Send + Sync>> {
            match row.take() {
                Some(b"superuser") => Ok(Role::Superuser),
                Some(b"user") => Ok(Role::User),
                Some(b"store_manager") => Ok(Role::StoreManager),
                Some(value) => Err(format!(
                    "Unrecognized enum variant for Role: {}",
                    str::from_utf8(value).unwrap_or("unreadable value")
                ).into()),
                None => Err("Unexpected null for non-null column `role`".into()),
            }
        }
    }

    impl Queryable<VarChar, Pg> for Role {
        type Row = Role;
        fn build(row: Self::Row) -> Self {
            row
        }
    }

    impl ToSql<VarChar, Pg> for Role {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            match *self {
                Role::Superuser => out.write_all(b"superuser")?,
                Role::User => out.write_all(b"user")?,
                Role::StoreManager => out.write_all(b"store_manager")?,
            }
            Ok(IsNull::No)
        }
    }

    impl AsExpression<VarChar> for Role {
        type Expression = Bound<VarChar, Role>;
        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl<'a> AsExpression<VarChar> for &'a Role {
        type Expression = Bound<VarChar, &'a Role>;
        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }
}
