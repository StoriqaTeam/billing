use std::fmt;

use stq_static_resources::Currency;

use models::OrderId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub store_id: StoreId,
    pub price: f64,
    pub currency_id: CurrencyId,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CreateInvoice {
    pub orders: Vec<Order>,
    pub currency_id: CurrencyId,
}

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct StoreId(pub i32);

#[derive(Clone, Copy, Debug, Default, FromStr, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct CurrencyId(pub i32);

impl fmt::Display for CurrencyId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self.0 {
                x if x == Currency::Euro as i32 => Currency::Euro.to_string(),
                x if x == Currency::Dollar as i32 => Currency::Dollar.to_string(),
                x if x == Currency::Bitcoin as i32 => Currency::Bitcoin.to_string(),
                x if x == Currency::Etherium as i32 => Currency::Etherium.to_string(),
                x if x == Currency::Stq as i32 => Currency::Stq.to_string(),
                _ => "".to_string(),
            }
        )
    }
}

mod diesel_impl {
    use diesel::deserialize::FromSql;
    use diesel::expression::bound::Bound;
    use diesel::expression::AsExpression;
    use diesel::pg::Pg;
    use diesel::row::Row;
    use diesel::serialize::Output;
    use diesel::sql_types::*;
    use diesel::types::{FromSqlRow, IsNull, ToSql};
    use diesel::Queryable;
    use std::error::Error;
    use std::io::Write;

    use super::StoreId;

    impl<'a> AsExpression<Integer> for &'a StoreId {
        type Expression = Bound<Integer, &'a StoreId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl AsExpression<Integer> for StoreId {
        type Expression = Bound<Integer, StoreId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl<'a> AsExpression<Nullable<Integer>> for &'a StoreId {
        type Expression = Bound<Nullable<Integer>, &'a StoreId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl AsExpression<Nullable<Integer>> for StoreId {
        type Expression = Bound<Nullable<Integer>, StoreId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl ToSql<Integer, Pg> for StoreId {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            ToSql::<Integer, Pg>::to_sql(&self.0, out)
        }
    }

    impl FromSqlRow<Integer, Pg> for StoreId {
        fn build_from_row<T: Row<Pg>>(row: &mut T) -> Result<Self, Box<Error + Send + Sync>> {
            FromSql::<Integer, Pg>::from_sql(row.take()).map(StoreId)
        }
    }

    impl Queryable<Integer, Pg> for StoreId {
        type Row = Self;

        fn build(row: Self::Row) -> Self {
            row
        }
    }

    impl ToSql<Nullable<Integer>, Pg> for StoreId {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            ToSql::<Nullable<Integer>, Pg>::to_sql(&self.0, out)
        }
    }

    use super::CurrencyId;

    impl<'a> AsExpression<Integer> for &'a CurrencyId {
        type Expression = Bound<Integer, &'a CurrencyId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl AsExpression<Integer> for CurrencyId {
        type Expression = Bound<Integer, CurrencyId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl ToSql<Integer, Pg> for CurrencyId {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            ToSql::<Integer, Pg>::to_sql(&self.0, out)
        }
    }

    impl FromSqlRow<Integer, Pg> for CurrencyId {
        fn build_from_row<T: Row<Pg>>(row: &mut T) -> Result<Self, Box<Error + Send + Sync>> {
            FromSql::<Integer, Pg>::from_sql(row.take()).map(CurrencyId)
        }
    }

    impl Queryable<Integer, Pg> for CurrencyId {
        type Row = Self;

        fn build(row: Self::Row) -> Self {
            row
        }
    }
}
