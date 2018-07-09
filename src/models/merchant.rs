use models::{StoreId, UserId};
use uuid::Uuid;

table! {
    merchants (merchant_id) {
        merchant_id -> Uuid,
        user_id -> Nullable<Integer>,
        store_id -> Nullable<Integer>,
        #[sql_name = "type"]
        merchant_type -> VarChar,
    }
}

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct MerchantId(pub Uuid);

impl MerchantId {
    pub fn new() -> Self {
        MerchantId(Uuid::new_v4())
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum MerchantType {
    Store,
    User,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SubjectIdentifier {
    Store(StoreId),
    User(UserId),
}

#[derive(Serialize, Queryable, Insertable, Debug)]
#[table_name = "merchants"]
pub struct Merchant {
    pub merchant_id: MerchantId,
    pub user_id: Option<UserId>,
    pub store_id: Option<StoreId>,
    pub merchant_type: MerchantType,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "merchants"]
pub struct NewStoreMerchant {
    merchant_id: MerchantId,
    user_id: Option<UserId>,
    store_id: Option<StoreId>,
    merchant_type: MerchantType,
}

impl NewStoreMerchant {
    pub fn new(merchant_id: MerchantId, store_id: StoreId) -> Self {
        Self {
            merchant_id,
            user_id: None,
            store_id: Some(store_id),
            merchant_type: MerchantType::Store,
        }
    }
    pub fn merchant_id(&self) -> &MerchantId {
        &self.merchant_id
    }
    pub fn user_id(&self) -> &Option<UserId> {
        &self.user_id
    }
    pub fn store_id(&self) -> &Option<StoreId> {
        &self.store_id
    }
    pub fn merchant_type(&self) -> &MerchantType {
        &self.merchant_type
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "merchants"]
pub struct NewUserMerchant {
    merchant_id: MerchantId,
    user_id: Option<UserId>,
    store_id: Option<StoreId>,
    merchant_type: MerchantType,
}

impl NewUserMerchant {
    pub fn new(merchant_id: MerchantId, user_id: UserId) -> Self {
        Self {
            merchant_id,
            user_id: Some(user_id),
            store_id: None,
            merchant_type: MerchantType::User,
        }
    }
    pub fn merchant_id(&self) -> &MerchantId {
        &self.merchant_id
    }
    pub fn user_id(&self) -> &Option<UserId> {
        &self.user_id
    }
    pub fn store_id(&self) -> &Option<StoreId> {
        &self.store_id
    }
    pub fn merchant_type(&self) -> &MerchantType {
        &self.merchant_type
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateUserMerchantPayload {
    pub id: UserId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStoreMerchantPayload {
    pub id: StoreId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalBillingMerchant {
    pub id: MerchantId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerchantBalance {
    pub id: MerchantId,
    pub amount: f64,
    pub currency: String,
}

mod diesel_impl {
    use diesel::expression::bound::Bound;
    use diesel::expression::AsExpression;
    use diesel::pg::Pg;
    use diesel::row::Row;
    use diesel::serialize::Output;
    use diesel::sql_types::Uuid as SqlUuid;
    use diesel::sql_types::*;
    use diesel::types::{FromSqlRow, IsNull, NotNull, SingleValue, ToSql};
    use diesel::Queryable;
    use std::error::Error;
    use std::io::Write;
    use std::str;

    use uuid::Uuid;

    use super::MerchantId;

    impl<'a> AsExpression<SqlUuid> for &'a MerchantId {
        type Expression = Bound<SqlUuid, &'a MerchantId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl AsExpression<SqlUuid> for MerchantId {
        type Expression = Bound<SqlUuid, MerchantId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl<'a> AsExpression<Nullable<SqlUuid>> for &'a MerchantId {
        type Expression = Bound<Nullable<SqlUuid>, &'a MerchantId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl AsExpression<Nullable<SqlUuid>> for MerchantId {
        type Expression = Bound<Nullable<SqlUuid>, MerchantId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl ToSql<SqlUuid, Pg> for MerchantId {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            out.write_all(self.0.as_bytes())?;
            Ok(IsNull::No)
        }
    }

    impl FromSqlRow<SqlUuid, Pg> for MerchantId {
        fn build_from_row<T: Row<Pg>>(row: &mut T) -> Result<Self, Box<Error + Send + Sync>> {
            let uuid = match row.take() {
                Some(id) => Uuid::from_bytes(id)?,
                None => Uuid::nil(),
            };
            Ok(MerchantId(uuid))
        }
    }

    impl Queryable<SqlUuid, Pg> for MerchantId {
        type Row = Self;

        fn build(row: Self::Row) -> Self {
            row
        }
    }

    use super::MerchantType;

    impl NotNull for MerchantType {}
    impl SingleValue for MerchantType {}

    impl FromSqlRow<VarChar, Pg> for MerchantType {
        fn build_from_row<R: Row<Pg>>(row: &mut R) -> Result<Self, Box<Error + Send + Sync>> {
            match row.take() {
                Some(b"store") => Ok(MerchantType::Store),
                Some(b"user") => Ok(MerchantType::User),
                Some(value) => Err(format!(
                    "Unrecognized enum variant for MerchantType: {}",
                    str::from_utf8(value).unwrap_or("unreadable value")
                ).into()),
                None => Err("Unexpected null for non-null column `type`".into()),
            }
        }
    }

    impl Queryable<VarChar, Pg> for MerchantType {
        type Row = MerchantType;
        fn build(row: Self::Row) -> Self {
            row
        }
    }

    impl ToSql<VarChar, Pg> for MerchantType {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            match *self {
                MerchantType::Store => out.write_all(b"store")?,
                MerchantType::User => out.write_all(b"user")?,
            }
            Ok(IsNull::No)
        }
    }

    impl AsExpression<VarChar> for MerchantType {
        type Expression = Bound<VarChar, MerchantType>;
        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl<'a> AsExpression<VarChar> for &'a MerchantType {
        type Expression = Bound<VarChar, &'a MerchantType>;
        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

}
