use std::sync::Arc;

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};

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

use super::error::{Error as ServiceError, ErrorKind};
use super::types::ServiceFutureV2;
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
        use stripe::EventObject::*;
        use stripe::EventType::*;

        let db_pool = self.static_context.db_pool.clone();
        let cpu_pool = self.static_context.cpu_pool.clone();
        let repo_factory = self.static_context.repo_factory.clone();

        //todo use actual values from config
        let signature_header = format!("{}", signature_header);
        let secret = self.static_context.config.stripe.secret_key.clone();

        let fut = spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let event_store_repo = repo_factory.create_event_store_repo_with_sys_acl(&conn);
            conn.transaction(move || {
                let event = Webhook::construct_event(event_payload, signature_header, secret)?;
                match (event.event_type, event.data.object) {
                    (PaymentIntentSucceeded, PaymentIntent(payment_intent)) => {
                        let payment_intent_id = payment_intent.id.clone();
                        event_store_repo
                            .add_event(Event::new(EventPayload::PaymentIntentSucceeded { payment_intent }))
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

pub fn payment_intent_success<C>(
    conn: &C,
    orders_repo: &OrdersRepo,
    invoices_repo: &InvoicesV2Repo,
    payment_intent_repo: &PaymentIntentRepo,
    payment_intent_invoices_repo: &PaymentIntentInvoiceRepo,
    payment_intent_fees_repo: &PaymentIntentFeeRepo,
    fees_repo: &FeeRepo,
    payment_intent_id: PaymentIntentId,
) -> Result<PaymentType, ServiceError>
where
    C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
{
    let payment_intent_id_cloned1 = payment_intent_id.clone();
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

    match (payment_intent_invoice, payment_intent_fee) {
        (Some(_), Some(_)) => {
            let e = format_err!(
                "Payment intent {} cannot be used for two payments at the same time.",
                payment_intent_id
            );
            Err(ectx!(err e, ErrorKind::Internal))
        }
        (Some(payment_intent_invoice), None) => {
            payment_intent_success_invoice(conn, orders_repo, invoices_repo, payment_intent_invoice, fees_repo).map(|res| {
                PaymentType::Invoice {
                    payment_intent,
                    invoice: res.0,
                    orders: res.1,
                }
            })
        }
        (None, Some(payment_intent_fee)) => payment_intent_success_fee(conn, fees_repo, payment_intent_fee).map(|_| PaymentType::Fee),
        _ => {
            let e = format_err!("Payment intent relationship by id {} not found.", payment_intent_id);
            Err(ectx!(err e, ErrorKind::Internal))
        }
    }
}

pub fn payment_intent_success_invoice<C>(
    conn: &C,
    orders_repo: &OrdersRepo,
    invoice_repo: &InvoicesV2Repo,
    payment_intent_invoice: PaymentIntentInvoice,
    _fees_repo: &FeeRepo,
) -> Result<(InvoiceV2, Vec<RawOrder>), ServiceError>
where
    C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
{
    conn.transaction::<_, ServiceError, _>(move || {
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

        Ok((invoice, orders))
    })
}

pub fn payment_intent_success_fee<C>(conn: &C, fees_repo: &FeeRepo, payment_intent_fee: PaymentIntentFee) -> Result<(), ServiceError>
where
    C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
{
    conn.transaction::<_, ServiceError, _>(move || {
        let update_fee = UpdateFee {
            status: Some(FeeStatus::Paid),
            ..Default::default()
        };

        fees_repo
            .update(payment_intent_fee.fee_id.clone(), update_fee)
            .map_err(ectx!(convert => payment_intent_fee.fee_id.clone()))
            .map(|_| ())
    })
}
