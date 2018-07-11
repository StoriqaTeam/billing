use uuid::Uuid;

use models::{MerchantId, Order};

table! {
    invoices (id) {
        id -> Uuid,
        invoice_id -> Uuid,
        billing_url -> VarChar,
    }
}

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize, DieselTypes)]
pub struct InvoiceId(pub Uuid);

impl InvoiceId {
    pub fn new() -> Self {
        InvoiceId(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, Default, FromStr, Display, Eq, PartialEq, Hash, Serialize, Deserialize, DieselTypes)]
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
