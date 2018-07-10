use uuid::Uuid;

use models::{MerchantId, Order};

table! {
    invoices (id) {
        id -> Uuid,
        invoice_id -> Uuid,
        billing_url -> VarChar,
    }
}

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct InvoiceId(pub Uuid);

impl InvoiceId {
    pub fn new() -> Self {
        InvoiceId(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct SagaId(pub Uuid);

impl SagaId {
    pub fn new() -> Self {
        SagaId(Uuid::new_v4())
    }
}

#[derive(Serialize, Deserialize, Queryable, Insertable, Debug, Clone)]
#[table_name = "invoices"]
pub struct Invoice {
    pub id: SagaId,
    pub invoice_id: InvoiceId,
    pub billing_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "invoices"]
pub struct NewInvoice {
    pub id: SagaId,
    pub invoice_id: InvoiceId,
    pub billing_url: String,
}

impl NewInvoice {
    pub fn new(id: SagaId, invoice_id: InvoiceId, billing_url: String) -> Self {
        Self {
            id,
            invoice_id,
            billing_url,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillingOrder {
    pub merchant_id: MerchantId,
    pub amount: f64,
    pub currency: String,
}

impl BillingOrder {
    pub fn new(order: Order, merchant_id: MerchantId) -> Self {
        Self {
            merchant_id,
            amount: order.price,
            currency: order.currency_id.to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateInvoicePayload {
    callback_url: String,
    currency: String,
    orders: Vec<BillingOrder>,
}

impl CreateInvoicePayload {
    pub fn new(orders: Vec<BillingOrder>, callback_url: String, currency: String) -> Self {
        Self {
            orders,
            callback_url,
            currency,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExternalBillingInvoice {
    pub id: InvoiceId,
    pub billing_url: String,
}

mod diesel_impl {
    use diesel::expression::bound::Bound;
    use diesel::expression::AsExpression;
    use diesel::pg::Pg;
    use diesel::row::Row;
    use diesel::serialize::Output;
    use diesel::sql_types::Uuid as SqlUuid;
    use diesel::types::{FromSqlRow, IsNull, ToSql};
    use diesel::Queryable;
    use std::error::Error;
    use std::io::Write;

    use uuid::Uuid;

    use super::InvoiceId;

    impl<'a> AsExpression<SqlUuid> for &'a InvoiceId {
        type Expression = Bound<SqlUuid, &'a InvoiceId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl AsExpression<SqlUuid> for InvoiceId {
        type Expression = Bound<SqlUuid, InvoiceId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl ToSql<SqlUuid, Pg> for InvoiceId {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            out.write_all(self.0.as_bytes())?;
            Ok(IsNull::No)
        }
    }

    impl FromSqlRow<SqlUuid, Pg> for InvoiceId {
        fn build_from_row<T: Row<Pg>>(row: &mut T) -> Result<Self, Box<Error + Send + Sync>> {
            let uuid = match row.take() {
                Some(id) => Uuid::from_bytes(id)?,
                None => Uuid::nil(),
            };
            Ok(InvoiceId(uuid))
        }
    }

    impl Queryable<SqlUuid, Pg> for InvoiceId {
        type Row = Self;

        fn build(row: Self::Row) -> Self {
            row
        }
    }

    use super::SagaId;

    impl<'a> AsExpression<SqlUuid> for &'a SagaId {
        type Expression = Bound<SqlUuid, &'a SagaId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl AsExpression<SqlUuid> for SagaId {
        type Expression = Bound<SqlUuid, SagaId>;

        fn as_expression(self) -> Self::Expression {
            Bound::new(self)
        }
    }

    impl ToSql<SqlUuid, Pg> for SagaId {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> Result<IsNull, Box<Error + Send + Sync>> {
            out.write_all(self.0.as_bytes())?;
            Ok(IsNull::No)
        }
    }

    impl FromSqlRow<SqlUuid, Pg> for SagaId {
        fn build_from_row<T: Row<Pg>>(row: &mut T) -> Result<Self, Box<Error + Send + Sync>> {
            let uuid = match row.take() {
                Some(id) => Uuid::from_bytes(id)?,
                None => Uuid::nil(),
            };
            Ok(SagaId(uuid))
        }
    }

    impl Queryable<SqlUuid, Pg> for SagaId {
        type Row = Self;

        fn build(row: Self::Row) -> Self {
            row
        }
    }
}
