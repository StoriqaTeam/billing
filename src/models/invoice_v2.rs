use std::fmt::{self, Display};
use std::str::FromStr;
use std::time::SystemTime;

use diesel::sql_types::Uuid as SqlUuid;
use uuid::{self, Uuid};

use models::{AccountId, Amount, Currency, UserId};
use schema::invoices_v2;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, PartialEq)]
#[sql_type = "SqlUuid"]
pub struct InvoiceId(Uuid);
derive_newtype_sql!(invoice_v2, SqlUuid, InvoiceId, InvoiceId);

impl InvoiceId {
    pub fn new(id: Uuid) -> Self {
        InvoiceId(id)
    }

    pub fn inner(&self) -> &Uuid {
        &self.0
    }

    pub fn generate() -> Self {
        InvoiceId(Uuid::new_v4())
    }
}

impl FromStr for InvoiceId {
    type Err = uuid::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = Uuid::parse_str(s)?;
        Ok(InvoiceId::new(id))
    }
}

impl Display for InvoiceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0.hyphenated()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable)]
#[table_name = "invoices_v2"]
pub struct RawInvoice {
    pub id: InvoiceId,
    pub account_id: Option<AccountId>,
    pub buyer_currency: Currency,
    pub amount_captured: Amount,
    pub final_amount_paid: Option<Amount>,
    pub final_cashback_amount: Option<Amount>,
    pub paid_at: Option<SystemTime>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub buyer_user_id: UserId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[table_name = "invoices_v2"]
pub struct NewInvoice {
    pub id: InvoiceId,
    pub account_id: Option<AccountId>,
    pub buyer_currency: Currency,
    pub amount_captured: Amount,
    pub buyer_user_id: UserId,
}

#[derive(Debug, Clone)]
pub struct InvoiceAccess {
    pub user_id: UserId,
}

impl From<NewInvoice> for InvoiceAccess {
    fn from(new_invoice: NewInvoice) -> InvoiceAccess {
        InvoiceAccess {
            user_id: new_invoice.buyer_user_id.clone(),
        }
    }
}

impl From<RawInvoice> for InvoiceAccess {
    fn from(raw_invoice: RawInvoice) -> InvoiceAccess {
        InvoiceAccess {
            user_id: raw_invoice.buyer_user_id.clone(),
        }
    }
}
