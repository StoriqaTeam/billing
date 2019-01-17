//! Order Services, presents CRUD operations with orders

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Fail;
use futures::Future;
use r2d2::ManageConnection;

use stq_http::client::HttpClient;

use super::error::ErrorKind;
use super::types::ServiceFutureV2;
use client::payments::PaymentsClient;
use models::order_v2::OrderId;
use repos::{ReposFactory, SearchPaymentIntent};
use services::accounts::AccountService;
use services::types::spawn_on_pool;
use services::Service;

pub trait OrderService {
    /// Capturing charge on order and setting order state to InProgress
    fn order_capture(&self, order_id: OrderId) -> ServiceFutureV2<()>;
    /// Refunding charge on order and setting order state to Cancel
    fn order_refund(&self, order_id: OrderId) -> ServiceFutureV2<()>;
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > OrderService for Service<T, M, F, C, PC, AS>
{
    fn order_capture(&self, order_id: OrderId) -> ServiceFutureV2<()> {
        let repo_factory = self.static_context.repo_factory.clone();
        let stripe_client = self.static_context.stripe_client.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.static_context.db_pool.clone();
        let cpu_pool = self.static_context.cpu_pool.clone();

        Box::new(
            spawn_on_pool(db_pool, cpu_pool, move |conn| {
                let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
                debug!("Requesting order by id: {}", order_id);
                let order = orders_repo.get(order_id).map_err(ectx!(try convert => order_id))?.ok_or({
                    let e = format_err!("Order {} not found", order_id);
                    ectx!(try err e, ErrorKind::Internal)
                })?;

                let payment_intent_repo = repo_factory.create_payment_intent_repo(&conn, user_id);
                let search = SearchPaymentIntent::InvoiceId(order.invoice_id);
                let search_clone = search.clone();
                let payment_intent = payment_intent_repo
                    .get(search.clone())
                    .map_err(ectx!(try convert => search))?
                    .ok_or({
                        let e = format_err!("payment intent {:?} not found", search_clone);
                        ectx!(try err e, ErrorKind::Internal)
                    })?;

                let payment_intent_id = payment_intent.id;
                payment_intent
                    .charge_id
                    .ok_or({
                        let e = format_err!("charge is absent in payment intent {:?}", payment_intent_id);
                        ectx!(err e, ErrorKind::Internal)
                    })
                    .map(|charge_id| (charge_id, order.total_amount))
            })
            .and_then(move |(charge_id, total_amount)| {
                stripe_client
                    .capture_charge(charge_id.clone(), total_amount)
                    .map_err(ectx!(convert => charge_id, total_amount))
                    .map(|_| ())
            }),
        )
    }
    fn order_refund(&self, order_id: OrderId) -> ServiceFutureV2<()> {
        let repo_factory = self.static_context.repo_factory.clone();
        let stripe_client = self.static_context.stripe_client.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.static_context.db_pool.clone();
        let cpu_pool = self.static_context.cpu_pool.clone();

        Box::new(
            spawn_on_pool(db_pool, cpu_pool, move |conn| {
                let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
                debug!("Requesting order by id: {}", order_id);
                let order = orders_repo.get(order_id).map_err(ectx!(try convert => order_id))?.ok_or({
                    let e = format_err!("Order {} not found", order_id);
                    ectx!(try err e, ErrorKind::Internal)
                })?;

                let payment_intent_repo = repo_factory.create_payment_intent_repo(&conn, user_id);
                let search = SearchPaymentIntent::InvoiceId(order.invoice_id);
                let search_clone = search.clone();
                let payment_intent = payment_intent_repo
                    .get(search.clone())
                    .map_err(ectx!(try convert => search))?
                    .ok_or({
                        let e = format_err!("payment intent {:?} not found", search_clone);
                        ectx!(try err e, ErrorKind::Internal)
                    })?;

                let payment_intent_id = payment_intent.id;
                payment_intent
                    .charge_id
                    .ok_or({
                        let e = format_err!("charge is absent in payment intent {:?}", payment_intent_id);
                        ectx!(err e, ErrorKind::Internal)
                    })
                    .map(|charge_id| (charge_id, order.total_amount))
            })
            .and_then(move |(charge_id, total_amount)| {
                stripe_client
                    .refund(charge_id.clone(), total_amount, order_id)
                    .map_err(ectx!(convert => charge_id, total_amount, order_id))
                    .map(|_| ())
            }),
        )
    }
}
