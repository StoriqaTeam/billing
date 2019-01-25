//! PaymentIntentService Services, presents CRUD operations with payment_intent

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use r2d2::ManageConnection;

use failure::Fail;

use stq_http::client::HttpClient;

use client::payments::PaymentsClient;
use models::invoice_v2::InvoiceId;
use services::accounts::AccountService;

use repos::{ReposFactory, SearchPaymentIntent, SearchPaymentIntentInvoice};
use services::{ErrorKind, Service};

use controller::responses::PaymentIntentResponse;

use super::types::ServiceFutureV2;

use services::types::spawn_on_pool;

pub trait PaymentIntentService {
    /// Returns payment intent object by invoice ID
    fn get_by_invoice(&self, invoice_id: InvoiceId) -> ServiceFutureV2<Option<PaymentIntentResponse>>;
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > PaymentIntentService for Service<T, M, F, C, PC, AS>
{
    fn get_by_invoice(&self, invoice_id: InvoiceId) -> ServiceFutureV2<Option<PaymentIntentResponse>> {
        let repo_factory = self.static_context.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.static_context.db_pool.clone();
        let cpu_pool = self.static_context.cpu_pool.clone();

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let payment_intent_repo = repo_factory.create_payment_intent_repo(&conn, user_id);
            debug!("Requesting payment intent by invoice id: {}", invoice_id);
            let payment_intent_invoices_repo = repo_factory.create_payment_intent_invoices_repo(&conn, user_id);

            let payment_intent_invoice = payment_intent_invoices_repo
                .get(SearchPaymentIntentInvoice::InvoiceId(invoice_id))
                .map_err(ectx!(try convert => invoice_id))?
                .ok_or({
                    let e = format_err!("Record payment_intent_invoice by invoice id {} not found", invoice_id);
                    ectx!(try err e, ErrorKind::Internal)
                })?;

            payment_intent_repo
                .get(SearchPaymentIntent::Id(payment_intent_invoice.payment_intent_id))
                .map_err(ectx!(convert => invoice_id))
                .and_then(|payment_intent| {
                    if let Some(value) = payment_intent {
                        PaymentIntentResponse::try_from_payment_intent(value).map(|res| Some(res))
                    } else {
                        Ok(None)
                    }
                })
        })
    }
}
