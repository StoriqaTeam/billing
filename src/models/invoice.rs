use std::fmt;
use std::str::FromStr;
use std::time::SystemTime;

use chrono::prelude::*;
use serde_json;

use stq_static_resources::*;
use stq_types::*;

use models::Order;
use schema::invoices;

#[derive(Serialize, Deserialize, Queryable, Insertable, AsChangeset, Debug, Clone)]
#[table_name = "invoices"]
pub struct Invoice {
    pub id: SagaId,
    pub invoice_id: InvoiceId,
    pub transactions: serde_json::Value,
    pub amount: ProductPrice,
    pub currency: Currency,
    pub price_reserved: SystemTime,
    pub state: OrderState,
    pub wallet: Option<String>,
    pub amount_captured: ProductPrice,
}

impl Invoice {
    pub fn new(id: SagaId, external_invoice: ExternalBillingInvoice) -> Self {
        let currency = external_invoice.currency;
        let state = external_invoice.status.into();
        let amount = ProductPrice(f64::from_str(&external_invoice.amount).unwrap_or_default());
        let amount_captured = ProductPrice(f64::from_str(&external_invoice.amount_captured).unwrap_or_default());
        let transactions: Vec<Transaction> = external_invoice
            .transactions
            .unwrap_or_default()
            .into_iter()
            .map(|t| t.into())
            .collect();
        let transactions = serde_json::to_value(transactions).unwrap_or_default();
        let price_reserved = external_invoice.expired.into();
        Self {
            id,
            invoice_id: external_invoice.id,
            transactions,
            amount,
            amount_captured,
            currency,
            price_reserved,
            state,
            wallet: external_invoice.wallet,
        }
    }
}

#[derive(Serialize, Deserialize, Queryable, Insertable, AsChangeset, Debug, Clone)]
#[table_name = "invoices"]
pub struct UpdateInvoice {
    pub transactions: serde_json::Value,
    pub amount: ProductPrice,
    pub currency: Currency,
    pub price_reserved: SystemTime,
    pub state: OrderState,
    pub wallet: Option<String>,
    pub amount_captured: ProductPrice,
}

impl From<ExternalBillingInvoice> for UpdateInvoice {
    fn from(external_invoice: ExternalBillingInvoice) -> Self {
        let currency = external_invoice.currency;
        let state = external_invoice.status.into();
        let amount = ProductPrice(f64::from_str(&external_invoice.amount).unwrap_or_default());
        let amount_captured = ProductPrice(f64::from_str(&external_invoice.amount_captured).unwrap_or_default());
        let transactions: Vec<Transaction> = external_invoice
            .transactions
            .unwrap_or_default()
            .into_iter()
            .map(|t| t.into())
            .collect();
        let transactions = serde_json::to_value(transactions).unwrap_or_default();
        let price_reserved = external_invoice.expired.into();
        Self {
            amount,
            amount_captured,
            transactions,
            currency,
            price_reserved,
            state,
            wallet: external_invoice.wallet,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub amount_captured: ProductPrice,
}

impl From<ExternalBillingTransaction> for Transaction {
    fn from(external_transaction: ExternalBillingTransaction) -> Self {
        let amount_captured = ProductPrice(f64::from_str(&external_transaction.amount_captured).unwrap_or_default());

        Self {
            id: external_transaction.txid,
            amount_captured,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillingOrder {
    pub merchant: MerchantId,
    pub amount: ProductPrice,
    pub currency: String,
    pub description: Option<String>,
}

impl BillingOrder {
    pub fn new(order: &Order, merchant: MerchantId) -> Self {
        Self {
            merchant,
            amount: ProductPrice(order.price.0 * f64::from(order.quantity.0)),
            currency: order.currency.to_string(),
            description: Some(format!("Order - id : {}", order.id)),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateInvoicePayload {
    callback_url: String,
    currency: String,
    amount: ProductPrice,
    timeout_s: i32,
    purchases: Vec<BillingOrder>,
}

impl CreateInvoicePayload {
    pub fn new(purchases: Vec<BillingOrder>, callback_url: String, currency: String, timeout_s: i32) -> Self {
        let amount = purchases.iter().fold(0.0, |acc, x| acc + x.amount.0); //TODO: ON EXTERNAL BILLING SIDE
        let amount = ProductPrice(amount);
        Self {
            purchases,
            callback_url,
            currency,
            amount,
            timeout_s,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExternalBillingInvoice {
    pub id: InvoiceId,
    pub amount_captured: String,
    pub transactions: Option<Vec<ExternalBillingTransaction>>,
    pub wallet: Option<String>,
    pub amount: String,
    pub currency: Currency,
    pub status: ExternalBillingStatus,
    pub expired: DateTime<Utc>,
}

impl fmt::Display for ExternalBillingInvoice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ExternalBillingInvoice - id : {}, amount_captured: {}, transactions: {:?}, wallet: {:?}, amount: {}, currency: {}, status: {:?}, expired: {}",
            self.id, self.amount_captured, self.transactions, self.wallet, self.amount, self.currency, self.status, self.expired,
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ExternalBillingStatus {
    New,
    Wallet,
    Waiting,
    Timeout,
    Done,
}

impl From<ExternalBillingStatus> for OrderState {
    fn from(external_invoice_status: ExternalBillingStatus) -> OrderState {
        match external_invoice_status {
            ExternalBillingStatus::New => OrderState::New,
            ExternalBillingStatus::Wallet => OrderState::PaymentAwaited,
            ExternalBillingStatus::Waiting => OrderState::TransactionPending,
            ExternalBillingStatus::Timeout => OrderState::AmountExpired,
            ExternalBillingStatus::Done => OrderState::Paid,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExternalBillingTransaction {
    pub txid: String,
    pub amount_captured: String,
}
