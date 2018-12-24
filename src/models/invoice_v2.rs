use std::fmt::{self, Display};
use std::str::FromStr;

use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use diesel::sql_types::Uuid as SqlUuid;
use stq_static_resources::OrderState;
use uuid::{self, Uuid};

use models::order_v2::{OrderId, RawOrder};
use models::{AccountId, Amount, Currency, RawOrderExchangeRate, UserId};
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

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash, DieselTypes)]
pub struct WalletAddress(String);

impl From<String> for WalletAddress {
    fn from(address: String) -> Self {
        WalletAddress::new(address)
    }
}

impl WalletAddress {
    pub fn new(address: String) -> Self {
        WalletAddress(address)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
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
    pub paid_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub buyer_user_id: UserId,
    pub status: OrderState,
    pub wallet_address: Option<WalletAddress>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[table_name = "invoices_v2"]
pub struct NewInvoice {
    pub id: InvoiceId,
    pub account_id: Option<AccountId>,
    pub buyer_currency: Currency,
    pub amount_captured: Amount,
    pub buyer_user_id: UserId,
    pub wallet_address: Option<WalletAddress>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuyerAmounts {
    pub exchange_rate: BigDecimal,
    pub currency: Currency,
    pub price: BigDecimal,
    pub cashback: BigDecimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderPrice {
    pub order_id: OrderId,
    pub seller_currency: Currency,
    pub seller_price: BigDecimal,
    pub seller_cashback: BigDecimal,
    pub buyer_amounts: Option<BuyerAmounts>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoicePrice {
    pub invoice_id: InvoiceId,
    pub buyer_currency: Currency,
    pub amount_captured: BigDecimal,
    pub total_price: BigDecimal,
    pub total_cashback: BigDecimal,
    pub order_prices: Vec<OrderPrice>,
    pub has_missing_rates: bool,
    pub created_at: NaiveDateTime,
    pub paid_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoicePriceCalculationData {
    pub invoice: RawInvoice,
    pub orders: (RawOrder, RawOrderExchangeRate),
}

pub fn calculate_invoice_price(invoice: RawInvoice, orders: Vec<(RawOrder, Option<RawOrderExchangeRate>)>) -> InvoicePrice {
    let RawInvoice {
        id,
        buyer_currency,
        amount_captured,
        final_amount_paid,
        final_cashback_amount,
        created_at,
        paid_at,
        ..
    } = invoice;

    let amount_captured = amount_captured.to_super_unit(buyer_currency);
    let final_amount_paid = final_amount_paid.map(|amount| amount.to_super_unit(buyer_currency));
    let final_cashback_amount = final_cashback_amount.map(|amount| amount.to_super_unit(buyer_currency));

    let order_prices = orders
        .into_iter()
        .map(|(order, rate)| {
            let RawOrder {
                id,
                seller_currency,
                cashback_amount,
                total_amount,
                ..
            } = order;

            let exchange_rate = if buyer_currency == seller_currency {
                Some(BigDecimal::from(1))
            } else {
                rate.map(|RawOrderExchangeRate { exchange_rate, .. }| exchange_rate)
            };

            OrderPrice {
                order_id: id,
                seller_currency,
                seller_price: total_amount.to_super_unit(seller_currency),
                seller_cashback: cashback_amount.to_super_unit(seller_currency),
                buyer_amounts: exchange_rate.map(|exchange_rate| BuyerAmounts {
                    exchange_rate: exchange_rate.clone(),
                    currency: buyer_currency.clone(),
                    price: Amount::new(decimal_to_u128_round_up(
                        u128_to_decimal(total_amount.inner()) * exchange_rate.clone(),
                    ))
                    .to_super_unit(buyer_currency.clone()),
                    cashback: Amount::new(decimal_to_u128_round_down(u128_to_decimal(cashback_amount.inner()) * exchange_rate))
                        .to_super_unit(buyer_currency.clone()),
                }),
            }
        })
        .collect::<Vec<_>>();

    let has_missing_rates = order_prices.iter().any(|op| op.buyer_amounts.is_none());

    // Check if the invoice has been paid. If it has, return the final prices.
    // Either all of the fields must contain a value or none of them,
    // otherwise it means that the database contains invalid data
    match (final_amount_paid, final_cashback_amount, paid_at) {
        (Some(total_price), Some(total_cashback), Some(paid_at)) => InvoicePrice {
            invoice_id: id,
            buyer_currency,
            amount_captured,
            total_price,
            total_cashback,
            order_prices,
            has_missing_rates,
            created_at,
            paid_at: Some(paid_at),
        },
        _ => order_prices.clone().into_iter().fold(
            InvoicePrice {
                invoice_id: id,
                buyer_currency,
                amount_captured,
                total_price: BigDecimal::from(0),
                total_cashback: BigDecimal::from(0),
                order_prices,
                has_missing_rates,
                created_at,
                paid_at: None,
            },
            |mut invoice, order_price| {
                if let Some(BuyerAmounts { price, cashback, .. }) = order_price.buyer_amounts {
                    invoice.total_price = invoice.total_price + price;
                    invoice.total_cashback = invoice.total_cashback + cashback;
                };
                invoice
            },
        ),
    }
}

fn u128_to_decimal(value: u128) -> BigDecimal {
    value.to_string().parse().unwrap() // unwrap always succeeds
}

fn decimal_to_u128_round_up(value: BigDecimal) -> u128 {
    let i = value.with_scale(0);
    let rounded = if value > i { i + BigDecimal::from(1) } else { i };

    rounded.to_string().parse().unwrap() // unwrap always succeeds
}

fn decimal_to_u128_round_down(value: BigDecimal) -> u128 {
    value.with_scale(0).to_string().parse().unwrap() // unwrap always succeeds
}
