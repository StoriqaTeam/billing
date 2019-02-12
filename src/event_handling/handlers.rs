use std::str::FromStr;

use bigdecimal::BigDecimal;
use chrono::Utc;
use diesel::{connection::AnsiTransactionManager, pg::Pg, Connection};
use failure::Fail;
use futures::{future, Future, IntoFuture};
use r2d2::ManageConnection;
use stq_http::client::HttpClient;
use stq_static_resources::OrderState;
use stq_types::stripe::PaymentIntentId;
use stripe::CaptureMethod;
use stripe::PaymentIntent as StripePaymentIntent;
use uuid::Uuid;

use client::{
    payments::{CreateInternalTransaction, PaymentsClient},
    saga::{OrderStateUpdate, SagaClient},
    stores::{CurrencyExchangeInfo, StoresClient},
    stripe::StripeClient,
};
use models::{
    invoice_v2::{InvoiceId, InvoiceSetAmountPaid, PaymentFlow, RawInvoice},
    order_v2::OrderId,
    AccountId, AccountWithBalance, Amount, Currency, Event, EventPayload, PaymentState, PayoutId,
};
use repos::{ReposFactory, SearchPaymentIntent, SearchPaymentIntentInvoice};

use services::accounts::AccountService;
use services::payment_intent::cancel_payment_intent;
use services::stripe::PaymentType;

use super::error::*;
use super::{spawn_on_pool, EventHandler, EventHandlerFuture};

impl<T, M, F, HC, PC, SC, STC, STRC, AS> EventHandler<T, M, F, HC, PC, SC, STC, STRC, AS>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
    HC: HttpClient + Clone,
    PC: PaymentsClient + Clone,
    SC: SagaClient + Clone,
    STC: StoresClient + Clone,
    STRC: StripeClient + Clone,
    AS: AccountService + Clone + 'static,
{
    pub fn handle_event(self, event: Event) -> EventHandlerFuture<()> {
        let Event { id: _, payload } = event;

        match payload {
            EventPayload::NoOp => Box::new(future::ok(())),
            EventPayload::InvoicePaid { invoice_id } => self.handle_invoice_paid(invoice_id),
            EventPayload::PaymentIntentPaymentFailed { payment_intent } => self.handle_payment_intent_payment_failed(payment_intent),
            EventPayload::PaymentIntentAmountCapturableUpdated { payment_intent } => {
                self.handle_payment_intent_succeeded_or_amount_capturable_updated(payment_intent)
            }
            EventPayload::PaymentIntentSucceeded { payment_intent } => {
                self.handle_payment_intent_succeeded_or_amount_capturable_updated(payment_intent)
            }
            EventPayload::PaymentIntentCapture { order_id } => self.handle_payment_intent_capture(order_id),
            EventPayload::PaymentExpired { invoice_id } => self.handle_payment_expired(invoice_id),
            EventPayload::PayoutInitiated { payout_id } => self.handle_payout_initiated(payout_id),
        }
    }

    // TODO: handle this event properly
    pub fn handle_payment_intent_payment_failed(self, _payment_intent: StripePaymentIntent) -> EventHandlerFuture<()> {
        Box::new(future::ok(()))
    }

    pub fn handle_payment_intent_succeeded_or_amount_capturable_updated(
        self,
        payment_intent: StripePaymentIntent,
    ) -> EventHandlerFuture<()> {
        if payment_intent.capture_method == CaptureMethod::Manual && payment_intent.amount != payment_intent.amount_capturable {
            info!(
                "payment intent with id {} amount={}, amount_capturable={} are not equal. Payment intent: {:?}",
                payment_intent.id, payment_intent.amount, payment_intent.amount_capturable, payment_intent
            );
            return Box::new(future::ok(()));
        }

        let saga_client = self.saga_client.clone();
        let fee_config = self.fee.clone();

        let amount_paid = payment_intent.amount.clone();
        let payment_intent_id = PaymentIntentId(payment_intent.id.clone());
        let payment_intent_id_cloned = payment_intent_id.clone();
        let new_status = OrderState::Paid;

        let EventHandler {
            db_pool,
            cpu_pool,
            repo_factory,
            ..
        } = self;

        let fut = spawn_on_pool(db_pool.clone(), cpu_pool.clone(), {
            let repo_factory = repo_factory.clone();
            move |conn| {
                let orders_repo = repo_factory.create_orders_repo_with_sys_acl(&conn);
                let invoices_repo = repo_factory.create_invoices_v2_repo_with_sys_acl(&conn);
                let payment_intent_repo = repo_factory.create_payment_intent_repo_with_sys_acl(&conn);
                let payment_intent_invoices_repo = repo_factory.create_payment_intent_invoices_repo_with_sys_acl(&conn);
                let payment_intent_fees_repo = repo_factory.create_payment_intent_fees_repo_with_sys_acl(&conn);
                let fees_repo = repo_factory.create_fees_repo_with_sys_acl(&conn);

                crate::services::stripe::payment_intent_succeeded_or_amount_capturable_updated(
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
        .and_then({
            let db_pool = db_pool.clone();
            let cpu_pool = cpu_pool.clone();
            let repo_factory = repo_factory.clone();
            move |payment_type| match payment_type {
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

                    let saga_update_states = saga_client
                        .update_order_states(order_state_updates)
                        .map_err(ectx!(ErrorKind::Internal => payment_intent_id_cloned));

                    let set_invoice_paid = spawn_on_pool(db_pool, cpu_pool, move |conn| {
                        let invoices_repo = repo_factory.create_invoices_v2_repo_with_sys_acl(&conn);

                        let invoice_set_amount_paid = InvoiceSetAmountPaid {
                            final_amount_paid: Amount::new(amount_paid as u128),
                            final_cashback_amount: Amount::new(0u128),
                            paid_at: Utc::now().naive_utc(),
                        };

                        let invoice_id = invoice.id.clone();
                        invoices_repo
                            .set_amount_paid_fiat(invoice_id.clone(), invoice_set_amount_paid.clone())
                            .map_err(ectx!(convert => invoice_id, invoice_set_amount_paid))
                    });

                    future::Either::A(Future::join(saga_update_states, set_invoice_paid).map(|_| ()))
                }
                Some(PaymentType::Fee) => future::Either::B(future::ok(())),
                None => future::Either::B(future::ok(())),
            }
        });

        Box::new(fut)
    }

    pub fn handle_invoice_paid(self, invoice_id: InvoiceId) -> EventHandlerFuture<()> {
        let fut = self
            .clone()
            .get_ture_context()
            .into_future()
            .and_then(move |(payments_client, account_service)| {
                Box::new(
                    Future::join3(
                        self.clone().drain_and_unlink_account(payments_client, account_service, invoice_id),
                        self.clone().set_orders_status(invoice_id.clone(), OrderState::Paid),
                        self.create_fee_for_orders(invoice_id),
                    )
                    .map(|_| ()),
                )
            });

        Box::new(fut)
    }

    pub fn handle_payment_expired(self, invoice_id: InvoiceId) -> EventHandlerFuture<()> {
        let fut = self.clone().get_invoice(invoice_id).and_then(move |invoice| match invoice.paid_at {
            Some(_) => future::Either::A(future::ok(())), // do nothing if the invoice has already been paid
            None => future::Either::B(future::lazy(move || self.process_payment_expired(invoice))),
        });

        Box::new(fut)
    }

    fn process_payment_expired(self, invoice: RawInvoice) -> EventHandlerFuture<()> {
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let stripe_client = self.stripe_client.clone();
        let repo_factory = self.repo_factory.clone();

        let fut = match invoice.payment_flow() {
            PaymentFlow::Crypto => future::Either::A(future::lazy(move || {
                self.clone()
                    .get_ture_context()
                    .into_future()
                    .and_then(move |(payments_client, account_service)| {
                        Future::join(
                            self.clone().drain_and_unlink_account(payments_client, account_service, invoice.id),
                            self.set_orders_status(invoice.id.clone(), OrderState::AmountExpired),
                        )
                    })
            })),
            PaymentFlow::Fiat => future::Either::B(future::lazy(move || {
                Future::join(
                    self.set_orders_status(invoice.id.clone(), OrderState::AmountExpired),
                    cancel_payment_intent(db_pool, cpu_pool, stripe_client, repo_factory, invoice.id.clone())
                        .map_err(ectx!(ErrorKind::Internal => invoice.id)),
                )
            })),
        }
        .map(|_| ());

        Box::new(fut)
    }

    fn drain_and_unlink_account(self, payments_client: PC, account_service: AS, invoice_id: InvoiceId) -> EventHandlerFuture<()> {
        let fut = self.clone().get_invoice(invoice_id).and_then({
            let self_ = self.clone();
            move |RawInvoice {
                      id: invoice_id,
                      account_id,
                      ..
                  }| match account_id {
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

    fn get_invoice(self, invoice_id: InvoiceId) -> EventHandlerFuture<RawInvoice> {
        let EventHandler { db_pool, cpu_pool, .. } = self.clone();
        spawn_on_pool(db_pool, cpu_pool, {
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
            }
        })
    }

    fn set_orders_status(self, invoice_id: InvoiceId, status: OrderState) -> EventHandlerFuture<()> {
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
                        status: status.clone(),
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

    pub fn handle_payment_intent_capture(self, order_id: OrderId) -> EventHandlerFuture<()> {
        let db_pool_ = self.db_pool.clone();
        let cpu_pool_ = self.cpu_pool.clone();
        let repo_factory_ = self.repo_factory.clone();
        let stripe_client = self.stripe_client.clone();

        let fut = spawn_on_pool(db_pool_, cpu_pool_, move |conn| {
            let payment_intent_repo = repo_factory_.create_payment_intent_repo_with_sys_acl(&conn);
            let orders_repo = repo_factory_.create_orders_repo_with_sys_acl(&conn);
            let payment_intent_invoices_repo = repo_factory_.create_payment_intent_invoices_repo_with_sys_acl(&conn);
            let order = orders_repo.get(order_id).map_err(ectx!(try convert => order_id))?.ok_or({
                let e = format_err!("Record order with id {} not found", order_id);
                ectx!(try err e, ErrorKind::Internal)
            })?;

            if order.state != PaymentState::Initial || order.stripe_fee.is_some() {
                let e = format_err!("there is no need to perform capture payment intent");
                return Err(ectx!(err e, ErrorKind::AlreadyDone));
            }

            let order_invoice_id_cloned = order.invoice_id.clone();
            let payment_intent_invoice = payment_intent_invoices_repo
                .get(SearchPaymentIntentInvoice::InvoiceId(order.invoice_id.clone()))
                .map_err(ectx!(try convert => order_invoice_id_cloned))?
                .ok_or({
                    let e = format_err!("Record payment_intent_invoice by invoice id {} not found", order.invoice_id);
                    ectx!(try err e, ErrorKind::Internal)
                })?;

            let search = SearchPaymentIntent::Id(payment_intent_invoice.payment_intent_id);
            let search_clone = search.clone();
            payment_intent_repo
                .get(search.clone())
                .map_err(ectx!(try convert => search))?
                .ok_or({
                    let e = format_err!("payment intent {:?} not found", search_clone);
                    ectx!(err e, ErrorKind::Internal)
                })
                .map(|payment_intent| (payment_intent, order.total_amount, order.seller_currency))
        })
        .and_then(move |(payment_intent, total_amount, currency)| {
            let stripe_client_clone = stripe_client.clone();
            payment_intent
                .charge_id
                .ok_or({
                    let e = format_err!("payment intent charge paid not found");
                    ectx!(err e, ErrorKind::Internal)
                })
                .into_future()
                .and_then(move |charge_id| stripe_client.get_charge(charge_id.clone()).map_err(ectx!(convert => charge_id)))
                .and_then(move |charge| {
                    charge.balance_transaction.ok_or({
                        let e = format_err!("charge balance transaction id not found");
                        ectx!(err e, ErrorKind::Internal)
                    })
                })
                .and_then(move |balance_transaction| {
                    stripe_client_clone
                        .retrieve_balance_transaction(balance_transaction.clone())
                        .map_err(ectx!(convert => balance_transaction))
                })
                .map(move |balance_transaction| {
                    let total_amount_super_unit = total_amount.to_super_unit(currency);
                    let fee_procent = balance_transaction.fee as f64 / balance_transaction.amount as f64;
                    let stripe_fee = Amount::from_super_unit(currency, total_amount_super_unit * BigDecimal::from(fee_procent));
                    stripe_fee
                })
        })
        .and_then({
            let db_pool = self.db_pool.clone();
            let cpu_pool = self.cpu_pool.clone();
            let repo_factory = self.repo_factory.clone();
            move |stripe_fee| {
                spawn_on_pool(db_pool, cpu_pool, move |conn| {
                    let orders_repo = repo_factory.create_orders_repo_with_sys_acl(&conn);
                    info!("Setting order {} state \'Captured\'", order_id);
                    orders_repo
                        .update_state(order_id, PaymentState::Captured)
                        .map_err(ectx!(try convert => order_id))?;
                    orders_repo
                        .update_stripe_fee(order_id, stripe_fee)
                        .map_err(ectx!(convert => order_id, stripe_fee))
                        .map(|_| ())
                })
            }
        })
        .then(|res| {
            if let Err(ref res) = res {
                if res.kind() == ErrorKind::AlreadyDone {
                    return Ok(());
                }
            }
            res
        });
        Box::new(fut)
    }

    // TODO: implement payout processing
    pub fn handle_payout_initiated(self, _payout_id: PayoutId) -> EventHandlerFuture<()> {
        Box::new(future::ok(()))
    }
}
