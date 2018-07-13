use std::time::SystemTime;

use stq_static_resources::*;
use stq_types::*;

use models::Order;

table! {
    invoices (id) {
        id -> Uuid,
        invoice_id -> Uuid,
        billing_url -> VarChar,
        transaction_id -> Nullable<VarChar>,
        transaction_captured_amount -> Nullable<Double>,
        amount -> Double,
        currency_id -> Integer,
        price_reserved -> Timestamp, // UTC 0, generated at db level
        state -> VarChar,
        wallet -> VarChar,
    }
}

#[derive(Serialize, Deserialize, Queryable, Insertable, AsChangeset, Debug, Clone)]
#[table_name = "invoices"]
pub struct Invoice {
    pub id: SagaId,
    pub invoice_id: InvoiceId,
    pub billing_url: String,
    pub transaction_id: Option<String>,
    pub transaction_captured_amount: Option<ProductPrice>,
    pub amount: ProductPrice,
    pub currency_id: CurrencyId,
    pub price_reserved: SystemTime,
    pub state: OrderState,
    pub wallet: String,
}

impl Invoice {
    pub fn new(id: SagaId, external_invoice: ExternalBillingInvoice) -> Self {
        Self {
            id,
            invoice_id: external_invoice.id,
            billing_url: external_invoice.billing_url,
            transaction_id: external_invoice.transaction.clone().map(|t| t.id),
            transaction_captured_amount: external_invoice.transaction.clone().map(|t| t.captured_amount),
            amount: external_invoice.amount,
            currency_id: external_invoice.currency_id,
            price_reserved: external_invoice.price_reserved,
            state: external_invoice.state,
            wallet: external_invoice.wallet,
        }
    }
}

#[derive(Serialize, Deserialize, Queryable, Insertable, AsChangeset, Debug, Clone)]
#[table_name = "invoices"]
pub struct UpdateInvoice {
    pub invoice_id: InvoiceId,
    pub billing_url: String,
    pub transaction_id: Option<String>,
    pub transaction_captured_amount: Option<ProductPrice>,
    pub amount: ProductPrice,
    pub currency_id: CurrencyId,
    pub price_reserved: SystemTime,
    pub state: OrderState,
    pub wallet: String,
}

impl From<ExternalBillingInvoice> for UpdateInvoice {
    fn from(external_invoice: ExternalBillingInvoice) -> Self {
        Self {
            invoice_id: external_invoice.id,
            billing_url: external_invoice.billing_url,
            transaction_id: external_invoice.transaction.clone().map(|t| t.id),
            transaction_captured_amount: external_invoice.transaction.clone().map(|t| t.captured_amount),
            amount: external_invoice.amount,
            currency_id: external_invoice.currency_id,
            price_reserved: external_invoice.price_reserved,
            state: external_invoice.state,
            wallet: external_invoice.wallet,
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
    pub transaction: Option<Transaction>,
    pub amount: ProductPrice,
    pub currency_id: CurrencyId,
    pub price_reserved: SystemTime,
    pub state: OrderState,
    pub wallet: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub captured_amount: ProductPrice,
}
