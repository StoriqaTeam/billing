use std::sync::Arc;

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};
use stripe::PaymentIntent as StripePaymentIntent;

use failure::Fail;

use stq_http::client::HttpClient;
use stq_http::request_util::StripeSignature;

use client::payments::PaymentsClient;
use client::stripe::StripeClient;
use models::*;
use services::accounts::AccountService;
use stq_types::stripe::PaymentIntentId;
use stripe::Webhook;

use repos::ReposFactory;
use repos::{
    FeeRepo, InvoicesV2Repo, OrdersRepo, PaymentIntentFeeRepo, PaymentIntentInvoiceRepo, PaymentIntentRepo, SearchPaymentIntent,
    SearchPaymentIntentFee, SearchPaymentIntentInvoice,
};

use models::invoice_v2::RawInvoice as InvoiceV2;
use models::order_v2::RawOrder;

use super::error::{Error as ServiceError, ErrorContext, ErrorKind};
use super::types::ServiceFutureV2;
use config;
use controller::context::DynamicContext;
use controller::context::StaticContext;

use services::types::spawn_on_pool;

pub trait StripeService {
    /// Handles the callback from Stripe
    fn handle_stripe_event(&self, signature_header: StripeSignature, event_payload: String) -> ServiceFutureV2<()>;
}

pub struct StripeServiceImpl<
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
    C: HttpClient + Clone,
    PC: PaymentsClient + Clone,
    AS: AccountService + Clone,
> {
    pub db_pool: Pool<M>,
    pub cpu_pool: CpuPool,
    pub repo_factory: F,
    pub stripe_client: Arc<dyn StripeClient>,
    pub dynamic_context: DynamicContext<C, PC, AS>,
    pub static_context: StaticContext<T, M, F>,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > StripeService for StripeServiceImpl<T, M, F, C, PC, AS>
{
    fn handle_stripe_event(&self, signature_header: StripeSignature, event_payload: String) -> ServiceFutureV2<()> {
        info!(
            "stripe handle_stripe_event signature_header: {}, event_payload.len(): {}",
            signature_header,
            event_payload.len()
        );
        use stripe::EventObject::*;
        use stripe::EventType::*;

        let db_pool = self.static_context.db_pool.clone();
        let cpu_pool = self.static_context.cpu_pool.clone();
        let repo_factory = self.static_context.repo_factory.clone();

        let signature_header = format!("{}", signature_header);
        let signing_secret = self.static_context.config.stripe.signing_secret.clone();

        let fut = spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let event_store_repo = repo_factory.create_event_store_repo_with_sys_acl(&conn);
            conn.transaction(move || {
                let event = Webhook::new()
                    .construct_event(event_payload, signature_header, signing_secret)
                    .map_err(|e| {
                        warn!("stripe Webhook::construct_event error: {:?}", e);
                        ectx!(try err e, ErrorKind::Internal)
                    })?;
                info!("stripe handle_stripe_event event: {:?}", event);
                match (event.event_type, event.data.object) {
                    (PaymentIntentAmountCapturableUpdated, PaymentIntent(payment_intent)) => {
                        let payment_intent_id = payment_intent.id.clone();
                        event_store_repo
                            .add_event(Event::new(EventPayload::PaymentIntentAmountCapturableUpdated { payment_intent }))
                            .map_err(ectx!(try convert => payment_intent_id))?;
                    }
                    (PaymentIntentPaymentFailed, PaymentIntent(payment_intent)) => {
                        let payment_intent_id = payment_intent.id.clone();
                        event_store_repo
                            .add_event(Event::new(EventPayload::PaymentIntentPaymentFailed { payment_intent }))
                            .map_err(ectx!(try convert => payment_intent_id))?;
                    }
                    (event_type, event_object) => {
                        warn!(
                            "stripe handle_stripe_event unprocessable event - type: {:?}, object: {:?}",
                            event_type, event_object
                        );
                    }
                };
                Ok(())
            })
        });

        Box::new(fut)
    }
}

pub enum PaymentType {
    Invoice {
        payment_intent: PaymentIntent,
        invoice: InvoiceV2,
        orders: Vec<RawOrder>,
    },
    Fee,
}

pub fn payment_intent_amount_capturable_updated<C>(
    conn: &C,
    orders_repo: &OrdersRepo,
    invoices_repo: &InvoicesV2Repo,
    payment_intent_repo: &PaymentIntentRepo,
    payment_intent_invoices_repo: &PaymentIntentInvoiceRepo,
    payment_intent_fees_repo: &PaymentIntentFeeRepo,
    fees_repo: &FeeRepo,
    fee_config: config::FeeValues,
    payment_intent: StripePaymentIntent,
) -> Result<PaymentType, ServiceError>
where
    C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
{
    let payment_intent_id = PaymentIntentId(payment_intent.id.clone());
    let payment_intent_id_cloned1 = payment_intent_id.clone();

    let payment_intent_update = update_payment_intent(payment_intent);
    let payment_intent = payment_intent_repo
        .get(SearchPaymentIntent::Id(payment_intent_id.clone()))
        .map_err(ectx!(try convert => payment_intent_id_cloned1))?
        .ok_or({
            let e = format_err!("Payment intent {} not found", payment_intent_id);
            ectx!(try err e, ErrorKind::Internal)
        })?;

    let payment_intent_id_cloned2 = payment_intent_id.clone();
    let payment_intent_invoice = payment_intent_invoices_repo
        .get(SearchPaymentIntentInvoice::PaymentIntentId(payment_intent_id.clone()))
        .map_err(ectx!(try convert => payment_intent_id_cloned2))?;

    let payment_intent_id_cloned3 = payment_intent_id.clone();
    let payment_intent_fee = payment_intent_fees_repo
        .get(SearchPaymentIntentFee::PaymentIntentId(payment_intent_id.clone()))
        .map_err(ectx!(try convert => payment_intent_id_cloned3))?;
    let payment_intent_id_cloned4 = payment_intent_id.clone();

    conn.transaction::<_, ServiceError, _>(move || {
        payment_intent_repo
            .update(payment_intent_id.clone(), payment_intent_update)
            .map_err(ectx!(try convert => payment_intent_id_cloned4))?;
        match (payment_intent_invoice, payment_intent_fee) {
            (Some(_), Some(_)) => {
                let e = format_err!(
                    "Payment intent {} cannot be used for two payments at the same time.",
                    payment_intent_id
                );
                Err(ectx!(err e, ErrorKind::Internal))
            }
            (Some(payment_intent_invoice), None) => {
                payment_intent_amount_capturable_updated_invoice(orders_repo, invoices_repo, fees_repo, fee_config, payment_intent_invoice)
                    .map(|res| PaymentType::Invoice {
                        payment_intent,
                        invoice: res.0,
                        orders: res.1,
                    })
            }
            (None, Some(payment_intent_fee)) => {
                payment_intent_amount_capturable_updated_fee(fees_repo, payment_intent_fee).map(|_| PaymentType::Fee)
            }
            _ => {
                let e = format_err!("Payment intent relationship by id {} not found.", payment_intent_id);
                Err(ectx!(err e, ErrorKind::Internal))
            }
        }
    })
}

fn update_payment_intent(payment_intent: StripePaymentIntent) -> UpdatePaymentIntent {
    UpdatePaymentIntent {
        charge_id: payment_intent
            .charges
            .data
            .into_iter()
            .find(|charge| charge.paid)
            .map(|charge| ChargeId::new(charge.id)),
        ..Default::default()
    }
}

pub fn payment_intent_amount_capturable_updated_invoice(
    orders_repo: &OrdersRepo,
    invoice_repo: &InvoicesV2Repo,
    fees_repo: &FeeRepo,
    fee_config: config::FeeValues,
    payment_intent_invoice: PaymentIntentInvoice,
) -> Result<(InvoiceV2, Vec<RawOrder>), ServiceError> {
    let invoice_id = payment_intent_invoice.invoice_id;
    let invoice = invoice_repo
        .get(invoice_id.clone())
        .map_err(ectx!(try convert => invoice_id.clone()))?
        .ok_or({
            let e = format_err!("Invoice {} not found", invoice_id.clone());
            ectx!(try err e, ErrorKind::Internal)
        })?;

    let orders = orders_repo
        .get_many_by_invoice_id(invoice.id)
        .map_err(ectx!(try convert => invoice_id))?;

    for order in orders.iter() {
        let new_fee = create_fee(fee_config.order_percent, order)?;
        let _ = fees_repo.create(new_fee).map_err(ectx!(try convert => order.id.clone()))?;
    }

    Ok((invoice, orders))
}

fn create_fee(order_percent: u64, order: &RawOrder) -> Result<NewFee, ServiceError> {
    let hundred_percents = 100u64;

    let amount = order
        .total_amount
        .checked_div(Amount::from(hundred_percents))
        .and_then(|one_percent| one_percent.checked_mul(Amount::from(order_percent)))
        .ok_or(ectx!(try err ErrorContext::AmountConversion, ErrorKind::Internal))?;

    Ok(NewFee {
        order_id: order.id,
        amount,
        status: FeeStatus::NotPaid,
        currency: order.seller_currency.clone(),
        charge_id: None,
        metadata: None,
        crypto_currency: None,
        crypto_amount: None,
    })
}

pub fn payment_intent_amount_capturable_updated_fee(fees_repo: &FeeRepo, payment_intent_fee: PaymentIntentFee) -> Result<(), ServiceError> {
    let update_fee = UpdateFee {
        status: Some(FeeStatus::Paid),
        ..Default::default()
    };

    fees_repo
        .update(payment_intent_fee.fee_id.clone(), update_fee)
        .map_err(ectx!(convert => payment_intent_fee.fee_id.clone()))
        .map(|_| ())
}
