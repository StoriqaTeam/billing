use diesel::{connection::AnsiTransactionManager, pg::Pg, Connection};
use failure::{err_msg, Fail};
use futures::{future, Future, IntoFuture};
use hyper::Method;
use r2d2::ManageConnection;
use stq_http::client::HttpClient;
use stq_static_resources::OrderState;
use uuid::Uuid;

use client::payments::{CreateInternalTransaction, PaymentsClient};
use models::{invoice_v2::InvoiceId, order_v2::OrderStateUpdate, AccountId, AccountWithBalance, Event, EventPayload};
use repos::repo_factory::ReposFactory;
use services::accounts::AccountService;

use super::error::*;
use super::{spawn_on_pool, EventHandler, EventHandlerFuture};

impl<T, M, F, HC, PC, AS> EventHandler<T, M, F, HC, PC, AS>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
    HC: HttpClient + Clone,
    PC: PaymentsClient + Clone,
    AS: AccountService + Clone + 'static,
{
    pub fn handle_event(self, event: Event) -> EventHandlerFuture<()> {
        let Event { id: _, payload } = event;

        match payload {
            EventPayload::NoOp => Box::new(future::ok(())),
            EventPayload::InvoicePaid { invoice_id } => self.handle_invoice_paid(invoice_id),
        }
    }

    pub fn handle_invoice_paid(self, invoice_id: InvoiceId) -> EventHandlerFuture<()> {
        match (self.payments_client.clone(), self.account_service.clone()) {
            (Some(payments_client), Some(account_service)) => {
                let fut = Future::join(
                    self.clone().drain_and_unlink_account(payments_client, account_service, invoice_id),
                    self.mark_orders_as_paid_on_saga(invoice_id),
                )
                .map(|_| ());

                Box::new(fut)
            }
            _ => {
                let e = err_msg("Payments integration must be configured for the InvoicePaid event to be processed");
                Box::new(future::err(ectx!(err e, ErrorKind::Internal)))
            }
        }
    }

    fn drain_and_unlink_account(self, payments_client: PC, account_service: AS, invoice_id: InvoiceId) -> EventHandlerFuture<()> {
        let EventHandler { db_pool, cpu_pool, .. } = self.clone();

        let fut = spawn_on_pool(db_pool, cpu_pool, {
            let repo_factory = self.repo_factory.clone();
            move |conn| {
                let invoices_repo = repo_factory.create_invoices_v2_repo_with_sys_acl(&conn);
                let invoice_id_clone = invoice_id.clone();
                invoices_repo
                    .get(invoice_id_clone)
                    .map_err(ectx!(try convert => invoice_id_clone))?
                    .ok_or({
                        let e = format_err!("Invoice {} not found", invoice_id);
                        ectx!(err e, ErrorKind::Internal)
                    })
                    .map(|invoice| (invoice.id, invoice.account_id))
            }
        })
        .and_then({
            let self_ = self.clone();
            move |(invoice_id, account_id)| match account_id {
                // Don't do anything if the account is already unlinked
                None => future::Either::A(future::ok(())),
                // Drain and unlink the account
                Some(account_id) => future::Either::B(future::lazy(move || {
                    self_.clone().drain_account(payments_client, account_service, account_id).and_then({
                        let db_pool = self_.db_pool.clone();
                        let cpu_pool = self_.cpu_pool.clone();
                        let repo_factory = self_.repo_factory.clone();
                        move |_| {
                            spawn_on_pool(db_pool, cpu_pool, move |conn| {
                                let invoices_repo = repo_factory.create_invoices_v2_repo_with_sys_acl(&conn);
                                invoices_repo
                                    .unlink_account(invoice_id)
                                    .map(|_| ())
                                    .map_err(ectx!(convert => invoice_id))
                            })
                        }
                    })
                })),
            }
        });

        Box::new(fut)
    }

    fn drain_account(self, payments_client: PC, account_service: AS, account_id: AccountId) -> EventHandlerFuture<()> {
        let account_id = account_id.into_inner();
        let fut = account_service
            .get_account(account_id)
            .map_err(ectx!(ErrorKind::Internal => account_id))
            .and_then({
                let account_service = account_service.clone();
                move |AccountWithBalance { account, balance }| {
                    let currency = account.currency;
                    account_service
                        .get_main_account(currency)
                        .map(move |AccountWithBalance { account: main_account, .. }| (account_id, balance, main_account.id.into_inner()))
                        .map_err(ectx!(ErrorKind::Internal => currency))
                }
            })
            .and_then(move |(account_id, balance, main_account_id)| {
                let input = CreateInternalTransaction {
                    id: Uuid::new_v4(),
                    from: account_id,
                    to: main_account_id,
                    amount: balance,
                };

                payments_client
                    .create_internal_transaction(input.clone())
                    .map_err(ectx!(ErrorKind::Internal => input))
            });

        Box::new(fut)
    }

    fn mark_orders_as_paid_on_saga(self, invoice_id: InvoiceId) -> EventHandlerFuture<()> {
        let EventHandler { db_pool, cpu_pool, .. } = self.clone();

        let fut = spawn_on_pool(db_pool, cpu_pool, {
            let repo_factory = self.repo_factory.clone();
            move |conn| {
                let invoices_repo = repo_factory.create_invoices_v2_repo_with_sys_acl(&conn);
                let orders_repo = repo_factory.create_orders_repo_with_sys_acl(&conn);

                let invoice_id_clone = invoice_id.clone();
                let invoice = invoices_repo
                    .get(invoice_id_clone)
                    .map_err(ectx!(try convert => invoice_id_clone))?
                    .ok_or({
                        let e = format_err!("Invoice {} not found", invoice_id.clone());
                        ectx!(try err e, ErrorKind::Internal)
                    })?;

                let orders = orders_repo
                    .get_many_by_invoice_id(invoice_id)
                    .map_err(ectx!(try convert => invoice_id))?;

                Ok(orders
                    .into_iter()
                    .map(|order| OrderStateUpdate {
                        order_id: order.id,
                        store_id: order.store_id,
                        customer_id: invoice.buyer_user_id.clone(),
                        status: OrderState::Paid,
                    })
                    .collect::<Vec<_>>())
            }
        })
        .and_then({
            let http_client = self.http_client.clone();
            let saga_url = self.saga_url.clone();
            move |order_state_updates| {
                serde_json::to_string(&order_state_updates)
                    .map_err(ectx!(ErrorSource::SerdeJson, ErrorKind::Internal => order_state_updates))
                    .into_future()
                    .and_then(move |body| {
                        let url = format!("{}/orders/update_state", saga_url);
                        http_client
                            .request_json::<()>(Method::Post, url.clone(), Some(body.clone()), None)
                            .map_err(ectx!(ErrorKind::Internal => Method::Post, url, Some(body), None as Option<hyper::Headers>))
                    })
            }
        });

        Box::new(fut)
    }
}
