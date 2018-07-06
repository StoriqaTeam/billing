use uuid::Uuid;

table! {
    order_info (id) {
        id -> Uuid,
        order_id -> Uuid,
        callback_id -> Uuid,
        status -> VarChar,
    }
}

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub Uuid);

impl OrderId {
    pub fn new() -> Self {
        OrderId(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct OrderInfoId(pub Uuid);

impl OrderInfoId {
    pub fn new() -> Self {
        OrderInfoId(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct CallbackId(pub Uuid);

impl CallbackId {
    pub fn new() -> Self {
        CallbackId(Uuid::new_v4())
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum OrderStatus {
    PaimentAwaited,
    PaimentReceived,
}

#[derive(Serialize, Queryable, Insertable, Debug)]
#[table_name = "order_info"]
pub struct OrderInfo {
    pub id: OrderInfoId,
    pub order_id: OrderId,
    pub callback_id: CallbackId,
    pub status: OrderStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "order_info"]
pub struct NewOrderInfo {
    order_id: OrderId,
    callback_id: CallbackId,
}

impl NewOrderInfo {
    pub fn new(order_id: OrderId, callback_id: CallbackId) -> Self {
        Self { order_id, callback_id }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable, AsChangeset)]
#[table_name = "order_info"]
pub struct SetOrderInfoPaid {
    status: OrderStatus,
}

impl SetOrderInfoPaid {
    pub fn new() -> Self {
        Self {
            status: OrderStatus::PaimentReceived,
        }
    }
}

mod diesel_impl {
    use diesel::expression::bound::Bound;
    use diesel::expression::AsExpression;
    use diesel::pg::Pg;
    use diesel::row::Row;
    use diesel::serialize::Output;
    use diesel::sql_types::Uuid as SqlUuid;
    use diesel::sql_types::VarChar;
    use diesel::types::{FromSqlRow, IsNull, NotNull, SingleValue, ToSql};
    use diesel::Queryable;
    use std::error::Error;
    use std::io::Write;
    use std::str;

    use uuid::Uuid;

    use super::OrderId;

    impl<'a> AsExpression<SqlUuid> for &'a OrderId {
        type Expression = Bound<SqlUuid, &'a OrderId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl AsExpression<SqlUuid> for OrderId {
        type Expression = Bound<SqlUuid, OrderId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl ToSql<SqlUuid, Pg> for OrderId {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            out.write_all(self.0.as_bytes())?;
            Ok(IsNull::No)
        }
    }

    impl FromSqlRow<SqlUuid, Pg> for OrderId {
        fn build_from_row<T: Row<Pg>>(row: &mut T) -> Result<Self, Box<Error + Send + Sync>> {
            let uuid = match row.take() {
                Some(id) => Uuid::from_bytes(id)?,
                None => Uuid::nil(),
            };
            Ok(OrderId(uuid))
        }
    }

    impl Queryable<SqlUuid, Pg> for OrderId {
        type Row = Self;

        fn build(row: Self::Row) -> Self {
            row
        }
    }

    use super::OrderInfoId;

    impl<'a> AsExpression<SqlUuid> for &'a OrderInfoId {
        type Expression = Bound<SqlUuid, &'a OrderInfoId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl AsExpression<SqlUuid> for OrderInfoId {
        type Expression = Bound<SqlUuid, OrderInfoId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl ToSql<SqlUuid, Pg> for OrderInfoId {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            out.write_all(self.0.as_bytes())?;
            Ok(IsNull::No)
        }
    }

    impl FromSqlRow<SqlUuid, Pg> for OrderInfoId {
        fn build_from_row<T: Row<Pg>>(row: &mut T) -> Result<Self, Box<Error + Send + Sync>> {
            let uuid = match row.take() {
                Some(id) => Uuid::from_bytes(id)?,
                None => Uuid::nil(),
            };
            Ok(OrderInfoId(uuid))
        }
    }

    impl Queryable<SqlUuid, Pg> for OrderInfoId {
        type Row = Self;

        fn build(row: Self::Row) -> Self {
            row
        }
    }

    use super::OrderStatus;

    impl NotNull for OrderStatus {}
    impl SingleValue for OrderStatus {}

    impl FromSqlRow<VarChar, Pg> for OrderStatus {
        fn build_from_row<R: Row<Pg>>(row: &mut R) -> Result<Self, Box<Error + Send + Sync>> {
            match row.take() {
                Some(b"payment_awaited") => Ok(OrderStatus::PaimentAwaited),
                Some(b"payment_received") => Ok(OrderStatus::PaimentReceived),
                Some(value) => Err(format!(
                    "Unrecognized enum variant for OrderStatus: {}",
                    str::from_utf8(value).unwrap_or("unreadable value")
                ).into()),
                None => Err("Unexpected null for non-null column `status`".into()),
            }
        }
    }

    impl Queryable<VarChar, Pg> for OrderStatus {
        type Row = OrderStatus;
        fn build(row: Self::Row) -> Self {
            row
        }
    }

    impl ToSql<VarChar, Pg> for OrderStatus {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            match *self {
                OrderStatus::PaimentAwaited => out.write_all(b"payment_awaited")?,
                OrderStatus::PaimentReceived => out.write_all(b"payment_received")?,
            }
            Ok(IsNull::No)
        }
    }

    impl AsExpression<VarChar> for OrderStatus {
        type Expression = Bound<VarChar, OrderStatus>;
        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl<'a> AsExpression<VarChar> for &'a OrderStatus {
        type Expression = Bound<VarChar, &'a OrderStatus>;
        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    use super::CallbackId;

    impl<'a> AsExpression<SqlUuid> for &'a CallbackId {
        type Expression = Bound<SqlUuid, &'a CallbackId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl AsExpression<SqlUuid> for CallbackId {
        type Expression = Bound<SqlUuid, CallbackId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl ToSql<SqlUuid, Pg> for CallbackId {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            out.write_all(self.0.as_bytes())?;
            Ok(IsNull::No)
        }
    }

    impl FromSqlRow<SqlUuid, Pg> for CallbackId {
        fn build_from_row<T: Row<Pg>>(row: &mut T) -> Result<Self, Box<Error + Send + Sync>> {
            let uuid = match row.take() {
                Some(id) => Uuid::from_bytes(id)?,
                None => Uuid::nil(),
            };
            Ok(CallbackId(uuid))
        }
    }

    impl Queryable<SqlUuid, Pg> for CallbackId {
        type Row = Self;

        fn build(row: Self::Row) -> Self {
            row
        }
    }
}
