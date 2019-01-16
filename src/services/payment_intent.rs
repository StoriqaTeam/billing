//! PaymentIntentService Services, presents CRUD operations with payment_intent

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use r2d2::ManageConnection;

use failure::Fail;

use stq_http::client::HttpClient;

use client::payments::PaymentsClient;
use models::invoice_v2::InvoiceId;
use models::PaymentIntent;
use services::accounts::AccountService;

use repos::{ReposFactory, SearchPaymentIntent};
use services::Service;

use super::types::ServiceFutureV2;

use services::types::spawn_on_pool;

pub trait PaymentIntentService {
    /// Returns payment intent object by invoice ID
    fn get_by_invoice(&self, invoice_id: InvoiceId) -> ServiceFutureV2<Option<PaymentIntent>>;
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
    fn get_by_invoice(&self, invoice_id: InvoiceId) -> ServiceFutureV2<Option<PaymentIntent>> {
        let repo_factory = self.static_context.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.static_context.db_pool.clone();
        let cpu_pool = self.static_context.cpu_pool.clone();

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let payment_intent_repo = repo_factory.create_payment_intent_repo(&conn, user_id);
            debug!("Requesting payment intent by invoice id: {}", invoice_id);

            let search = SearchPaymentIntent::InvoiceId(invoice_id);
            payment_intent_repo.get(search).map_err(ectx!(convert => invoice_id))
        })
    }
}
