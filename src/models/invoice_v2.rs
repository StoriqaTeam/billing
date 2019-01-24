use std::fmt::{self, Display};
use std::str::FromStr;
use std::time::{Duration, SystemTime};

use bigdecimal::{BigDecimal, ToPrimitive};
use chrono::NaiveDateTime;
use diesel::sql_types::Uuid as SqlUuid;
use stq_static_resources::OrderState;
use stq_types::{InvoiceId as InvoiceV1Id, ProductPrice, SagaId};
use uuid::{self, Uuid};

use models::order_v2::{OrderId, RawOrder};
use models::{
    AccountId, Amount, Currency, ExchangeRateStatus, Invoice as InvoiceV1, OrderExchangeRateId, RawOrderExchangeRate, TransactionId,
    UserId, WalletAddress,
};
use schema::amounts_received;
use schema::invoices_v2;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub paid_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub buyer_user_id: UserId,
    pub status: OrderState,
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable)]
#[table_name = "amounts_received"]
pub struct RawAmountReceived {
    pub id: TransactionId,
    pub invoice_id: InvoiceId,
    pub amount_received: Amount,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[table_name = "amounts_received"]
pub struct NewAmountReceived {
    pub id: TransactionId,
    pub invoice_id: InvoiceId,
    pub amount_received: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewInvoice {
    pub id: InvoiceId,
    pub account_id: Option<AccountId>,
    pub buyer_currency: Currency,
    pub amount_captured: Amount,
    pub buyer_user_id: UserId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[table_name = "invoices_v2"]
pub struct RawNewInvoice {
    pub id: InvoiceId,
    pub account_id: Option<AccountId>,
    pub buyer_currency: Currency,
    pub amount_captured: Amount,
    pub buyer_user_id: UserId,
    pub status: OrderState,
}

impl From<NewInvoice> for RawNewInvoice {
    fn from(invoice: NewInvoice) -> Self {
        let NewInvoice {
            id,
            account_id,
            buyer_currency,
            amount_captured,
            buyer_user_id,
        } = invoice;

        Self {
            id,
            account_id,
            buyer_currency,
            amount_captured,
            buyer_user_id,
            status: OrderState::PaymentAwaited,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceSetAmountPaid {
    pub final_amount_paid: Amount,
    pub final_cashback_amount: Amount,
    pub paid_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, AsChangeset)]
#[table_name = "invoices_v2"]
pub struct RawInvoiceSetAmountPaid {
    pub final_amount_paid: Amount,
    pub final_cashback_amount: Amount,
    pub paid_at: NaiveDateTime,
    pub status: OrderState,
}

impl From<InvoiceSetAmountPaid> for RawInvoiceSetAmountPaid {
    fn from(payload: InvoiceSetAmountPaid) -> Self {
        let InvoiceSetAmountPaid {
            final_amount_paid,
            final_cashback_amount,
            paid_at,
        } = payload;
        Self {
            final_amount_paid,
            final_cashback_amount,
            paid_at,
            status: OrderState::Paid,
        }
    }
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateDump {
    pub id: OrderExchangeRateId,
    pub exchange_rate: BigDecimal,
    pub status: ExchangeRateStatus,
    pub reserved_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDump {
    pub id: OrderId,
    pub seller_currency: Currency,
    pub seller_price: BigDecimal,
    pub seller_cashback: BigDecimal,
    pub buyer_amounts: Option<BuyerAmounts>,
    pub rates: Vec<RateDump>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceDump {
    pub id: InvoiceId,
    pub buyer_currency: Currency,
    pub amount_captured: BigDecimal,
    pub total_price: BigDecimal,
    pub total_cashback: Option<BigDecimal>,
    pub orders: Vec<OrderDump>,
    pub has_missing_rates: bool,
    pub created_at: NaiveDateTime,
    pub paid_at: Option<NaiveDateTime>,
    pub wallet_address: Option<WalletAddress>,
}

#[derive(Debug, Clone, Fail)]
pub enum InvoiceConversionError {
    #[fail(display = "conversion of total price to f64 failed")]
    TotalPriceConversionError(BigDecimal),
    #[fail(display = "conversion of amount captured to f64 failed")]
    AmountCapturedConversionError(BigDecimal),
}

impl InvoiceDump {
    pub fn try_into_v1(self) -> Result<InvoiceV1, InvoiceConversionError> {
        let InvoiceDump {
            id,
            buyer_currency,
            wallet_address,
            amount_captured,
            total_price,
            paid_at,
            ..
        } = self;

        let amount = ProductPrice(
            total_price
                .to_f64()
                .ok_or(InvoiceConversionError::TotalPriceConversionError(total_price))?,
        );
        let amount_captured = ProductPrice(
            amount_captured
                .to_f64()
                .ok_or(InvoiceConversionError::AmountCapturedConversionError(amount_captured))?,
        );

        Ok(InvoiceV1 {
            id: SagaId(id.0.clone()),
            invoice_id: InvoiceV1Id(id.0.clone()),
            transactions: json!([]),
            amount,
            currency: buyer_currency.into(),
            price_reserved: SystemTime::now() + Duration::from_secs(300), // assume that the price is reserved for 5 mins
            state: match paid_at {
                None => OrderState::PaymentAwaited,
                Some(_) => OrderState::Paid,
            },
            wallet: wallet_address.map(|address| address.into_inner()),
            amount_captured,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceDumpCalculationData {
    pub invoice: RawInvoice,
    pub orders: (RawOrder, RawOrderExchangeRate),
}

pub fn calculate_invoice_price(
    invoice: RawInvoice,
    orders: Vec<(RawOrder, Vec<RawOrderExchangeRate>)>,
    wallet_address: Option<WalletAddress>,
) -> InvoiceDump {
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

    let orders = orders
        .into_iter()
        .map(|(order, rates)| {
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
                rates
                    .iter()
                    .find(|rate| rate.status == ExchangeRateStatus::Active)
                    .map(|RawOrderExchangeRate { ref exchange_rate, .. }| exchange_rate.clone())
            };

            let seller_price = total_amount.to_super_unit(seller_currency);
            OrderDump {
                id,
                seller_currency,
                seller_price: seller_price.clone(),
                seller_cashback: cashback_amount.to_super_unit(seller_currency),
                buyer_amounts: exchange_rate.map(|exchange_rate| BuyerAmounts {
                    exchange_rate: exchange_rate.clone(),
                    currency: buyer_currency.clone(),
                    price: seller_price / exchange_rate.clone(),
                }),
                rates: rates
                    .into_iter()
                    .map(|rate| {
                        let RawOrderExchangeRate {
                            id,
                            exchange_rate,
                            status,
                            created_at,
                            ..
                        } = rate;
                        RateDump {
                            id,
                            exchange_rate,
                            status,
                            reserved_at: created_at,
                        }
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();

    let has_missing_rates = orders.iter().any(|op| op.buyer_amounts.is_none());

    // Check if the invoice has been paid. If it has, return the final prices.
    // Either all of the fields must contain a value or none of them,
    // otherwise it means that the database contains invalid data
    match (final_amount_paid, final_cashback_amount, paid_at) {
        (Some(total_price), Some(total_cashback), Some(paid_at)) => InvoiceDump {
            id,
            buyer_currency,
            amount_captured,
            total_price,
            total_cashback: Some(total_cashback),
            orders,
            has_missing_rates,
            created_at,
            paid_at: Some(paid_at),
            wallet_address,
        },
        _ => orders.clone().into_iter().fold(
            InvoiceDump {
                id,
                buyer_currency,
                amount_captured,
                total_price: BigDecimal::from(0),
                total_cashback: Some(BigDecimal::from(0)),
                orders,
                has_missing_rates,
                created_at,
                paid_at: None,
                wallet_address,
            },
            |mut invoice, order_price| {
                if let Some(BuyerAmounts { price, .. }) = order_price.buyer_amounts {
                    invoice.total_price += price;
                };
                invoice
            },
        ),
    }
}
