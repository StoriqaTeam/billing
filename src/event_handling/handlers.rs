use std::str::FromStr;

use diesel::{connection::AnsiTransactionManager, pg::Pg, Connection};
use failure::{err_msg, Fail};
use futures::{future, Future};
use r2d2::ManageConnection;
use stq_http::client::HttpClient;
use stq_static_resources::OrderState;
use stq_types::stripe::PaymentIntentId;
use stripe::PaymentIntent as StripePaymentIntent;
use uuid::Uuid;

use client::{
    payments::{CreateInternalTransaction, PaymentsClient},
    saga::{OrderStateUpdate, SagaClient},
    stores::{CurrencyExchangeInfo, StoresClient},
};
use models::{invoice_v2::InvoiceId, AccountId, AccountWithBalance, Currency, Event, EventPayload};

use repos::repo_factory::ReposFactory;
use services::accounts::AccountService;
use services::stripe::PaymentType;

use super::error::*;
use super::{spawn_on_pool, EventHandler, EventHandlerFuture};

impl<T, M, F, HC, PC, SC, STC, AS> EventHandler<T, M, F, HC, PC, SC, STC, AS>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
    HC: HttpClient + Clone,
    PC: PaymentsClient + Clone,
    SC: SagaClient + Clone,
    STC: StoresClient + Clone,
    AS: AccountService + Clone + 'static,
{
    pub fn handle_event(self, event: Event) -> EventHandlerFuture<()> {
        let Event { id: _, payload } = event;

        match payload {
            EventPayload::NoOp => Box::new(future::ok(())),
            EventPayload::InvoicePaid { invoice_id } => self.handle_invoice_paid(invoice_id),
            EventPayload::PaymentIntentPaymentFailed { payment_intent } => self.handle_payment_intent_payment_failed(payment_intent),
            EventPayload::PaymentIntentAmountCapturableUpdated { payment_intent } => {
                self.handle_payment_intent_amount_capturable_updated(payment_intent)
            }
        }
    }

    // TODO: handle this event properly
    pub fn handle_payment_intent_payment_failed(self, _payment_intent: StripePaymentIntent) -> EventHandlerFuture<()> {
        Box::new(future::ok(()))
    }

    pub fn handle_payment_intent_amount_capturable_updated(self, payment_intent: StripePaymentIntent) -> EventHandlerFuture<()> {
        if payment_intent.amount != payment_intent.amount_capturable {
            info!(
                "payment intent {} amount={}, amount_capturable={} are not equal",
                payment_intent.id, payment_intent.amount, payment_intent.amount_capturable
            );
            return Box::new(future::ok(()));
        }

        let saga_client = self.saga_client.clone();
        let fee_config = self.fee.clone();

        let payment_intent_id = PaymentIntentId(payment_intent.id.clone());
        let payment_intent_id_cloned = payment_intent_id.clone();
        let new_status = OrderState::Paid;

        let EventHandler { db_pool, cpu_pool, .. } = self;

        let fut = spawn_on_pool(db_pool, cpu_pool, {
            let repo_factory = self.repo_factory.clone();
            move |conn| {
                let orders_repo = repo_factory.create_orders_repo_with_sys_acl(&conn);
                let invoices_repo = repo_factory.create_invoices_v2_repo_with_sys_acl(&conn);
                let payment_intent_repo = repo_factory.create_payment_intent_repo_with_sys_acl(&conn);
                let payment_intent_invoices_repo = repo_factory.create_payment_intent_invoices_repo_with_sys_acl(&conn);
                let payment_intent_fees_repo = repo_factory.create_payment_intent_fees_repo_with_sys_acl(&conn);
                let fees_repo = repo_factory.create_fees_repo_with_sys_acl(&conn);

                crate::services::stripe::payment_intent_amount_capturable_updated(
                    &*conn,
                    &*orders_repo,
                    &*invoices_repo,
                    &*payment_intent_repo,
                    &*payment_intent_invoices_repo,
                    &*payment_intent_fees_repo,
                    &*fees_repo,
                    fee_config,
                    payment_intent,
                )
                .map_err(ectx!(ErrorKind::Internal => payment_intent_id))
                .map(Some)
            }
        })
        .and_then(move |payment_type| match payment_type {
            Some(PaymentType::Invoice { invoice, orders, .. }) => {
                let order_state_updates = orders
                    .into_iter()
                    .map(|order| OrderStateUpdate {
                        order_id: order.id,
                        store_id: order.store_id,
                        customer_id: invoice.buyer_user_id,
                        status: new_status,
                    })
                    .collect();

                future::Either::A(
                    saga_client
                        .update_order_states(order_state_updates)
                        .map_err(ectx!(ErrorKind::Internal => payment_intent_id_cloned)),
                )
            }
            Some(PaymentType::Fee) => future::Either::B(future::ok(())),
            None => future::Either::B(future::ok(())),
        });

        Box::new(fut)
    }

    // TODO: handle this event properly
    pub fn handle_invoice_paid(self, invoice_id: InvoiceId) -> EventHandlerFuture<()> {
        match (self.payments_client.clone(), self.account_service.clone()) {
            (Some(payments_client), Some(account_service)) => {
                let fut = Future::join3(
                    self.clone().drain_and_unlink_account(payments_client, account_service, invoice_id),
                    self.clone().mark_orders_as_paid_on_saga(invoice_id.clone()),
                    self.create_fee_for_orders(invoice_id),
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
            let saga_client = self.saga_client.clone();
            move |order_state_updates| {
                saga_client
                    .update_order_states(order_state_updates.clone())
                    .map_err(ectx!(ErrorKind::Internal => order_state_updates))
            }
        });

        Box::new(fut)
    }

    fn create_fee_for_orders(self, invoice_id: InvoiceId) -> EventHandlerFuture<()> {
        let EventHandler { db_pool, cpu_pool, .. } = self.clone();

        let fut = spawn_on_pool(db_pool, cpu_pool, {
            let repo_factory = self.repo_factory.clone();
            move |conn| {
                let invoices_repo = repo_factory.create_invoices_v2_repo_with_sys_acl(&conn);
                let orders_repo = repo_factory.create_orders_repo_with_sys_acl(&conn);

                let invoice_id_clone = invoice_id.clone();
                let _invoice = invoices_repo
                    .get(invoice_id_clone)
                    .map_err(ectx!(try convert => invoice_id_clone))?
                    .ok_or({
                        let e = format_err!("Invoice {} not found", invoice_id.clone());
                        ectx!(try err e, ErrorKind::Internal)
                    })?;

                orders_repo.get_many_by_invoice_id(invoice_id).map_err(ectx!(convert => invoice_id))
            }
        })
        .and_then({
            let currency_code = self.fee.currency_code.clone();
            move |orders| {
                Currency::from_str(&currency_code)
                    .map_err(ectx!(ErrorKind::CurrencyConversion))
                    .map(|fee_currency| (fee_currency, orders))
            }
        })
        .and_then({
            let stores_client = self.stores_client.clone();
            move |(fee_currency, orders)| {
                stores_client
                    .get_currency_exchange()
                    .map_err(ectx!(convert))
                    .and_then(|response| CurrencyExchangeInfo::try_from_request(response).map_err(ectx!(ErrorKind::CurrencyConversion)))
                    .map(move |currency_exchange_info| (currency_exchange_info, fee_currency, orders))
            }
        })
        .and_then({
            let EventHandler { db_pool, cpu_pool, .. } = self.clone();
            let order_percent = self.fee.order_percent.clone();

            move |(currency_exchange_info, fee_currency, orders)| {
                spawn_on_pool(db_pool, cpu_pool, {
                    let repo_factory = self.repo_factory.clone();
                    move |conn| {
                        let fees_repo = repo_factory.create_fees_repo_with_sys_acl(&conn);

                        for order in orders.iter() {
                            let new_fee =
                                crate::services::invoice::create_crypto_fee(order_percent, &fee_currency, &currency_exchange_info, order)
                                    .map_err(ectx!(try ErrorKind::Internal => order.id))?;

                            let _ = fees_repo
                                .create(new_fee)
                                .map_err(ectx!(try ErrorKind::Internal => order.id.clone()))?;
                        }

                        Ok(())
                    }
                })
            }
        });

        Box::new(fut)
    }
}
