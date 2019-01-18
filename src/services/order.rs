//! Order Services, presents CRUD operations with orders

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Fail;
use future::Either;
use futures::{Future, IntoFuture};
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};
use validator::{ValidationError, ValidationErrors};

use stq_http::client::HttpClient;
use stq_types::UserId;

use super::error::{ErrorContext, ErrorKind};
use super::types::ServiceFutureV2;
use client::payments::PaymentsClient;
use client::stripe::StripeClient;
use models::order_v2::{OrderId, RawOrder};
use models::PaymentState;
use repos::{ReposFactory, SearchPaymentIntent};
use services::accounts::AccountService;
use services::types::spawn_on_pool;
use services::Service;

pub trait OrderService {
    /// Capturing charge on order and setting order state to InProgress
    fn order_capture(&self, order_id: OrderId) -> ServiceFutureV2<()>;
    /// Refunding charge on order and setting order state to Cancel
    fn order_decline(&self, order_id: OrderId) -> ServiceFutureV2<()>;
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

                if order.state != PaymentState::Initial {
                    let mut errors = ValidationErrors::new();
                    let mut error = ValidationError::new("wrong_state");
                    error.message = Some(format!("Cannot capture order in state is \"{}\"", order.state,).into());
                    errors.add("order", error);
                    return Err(
                        ectx!(err ErrorContext::OrderState ,ErrorKind::Validation(serde_json::to_value(errors).unwrap_or_default())),
                    );
                }

                Ok(order)
            })
            .and_then({
                let repo_factory = self.static_context.repo_factory.clone();
                let db_pool = self.static_context.db_pool.clone();
                let cpu_pool = self.static_context.cpu_pool.clone();
                move |order| {
                    if order.seller_currency.is_fiat() {
                        Either::A(order_capture_fiat(cpu_pool, db_pool, repo_factory, user_id, stripe_client, order))
                    } else {
                        Either::B(order_capture_crypto(cpu_pool, db_pool, repo_factory, user_id, order))
                    }
                }
            }),
        )
    }
    fn order_decline(&self, order_id: OrderId) -> ServiceFutureV2<()> {
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

                if order.state != PaymentState::Initial {
                    let mut errors = ValidationErrors::new();
                    let mut error = ValidationError::new("wrong_state");
                    error.message = Some(format!("Cannot capture order in state is \"{}\"", order.state,).into());
                    errors.add("order", error);
                    return Err(
                        ectx!(err ErrorContext::OrderState ,ErrorKind::Validation(serde_json::to_value(errors).unwrap_or_default())),
                    );
                }

                Ok(order)
            })
            .and_then({
                let repo_factory = self.static_context.repo_factory.clone();
                let db_pool = self.static_context.db_pool.clone();
                let cpu_pool = self.static_context.cpu_pool.clone();
                move |order| {
                    if order.seller_currency.is_fiat() {
                        Either::A(order_decline_fiat(cpu_pool, db_pool, repo_factory, user_id, stripe_client, order))
                    } else {
                        Either::B(order_decline_crypto(cpu_pool, db_pool, repo_factory, user_id, order))
                    }
                }
            }),
        )
    }
}

fn order_capture_fiat<T, F, M>(
    cpu_pool: CpuPool,
    db_pool: Pool<M>,
    repo_factory: F,
    user_id: Option<UserId>,
    stripe_client: std::sync::Arc<dyn StripeClient>,
    order: RawOrder,
) -> ServiceFutureV2<()>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    F: ReposFactory<T>,
    M: ManageConnection<Connection = T>,
{
    let db_pool_ = db_pool.clone();
    let cpu_pool_ = cpu_pool.clone();
    let repo_factory_ = repo_factory.clone();
    let order_id = order.id;

    let fut = spawn_on_pool(db_pool_, cpu_pool_, move |conn| {
        let payment_intent_repo = repo_factory_.create_payment_intent_repo(&conn, user_id);
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
            .map(|charge_id| (charge_id, order.total_amount, order.seller_currency))
    })
    .and_then(move |(charge_id, total_amount, currency)| {
        currency
            .convert()
            .map_err(ectx!(convert => currency))
            .into_future()
            .and_then(move |currency| {
                let stripe_client_clone = stripe_client.clone();
                stripe_client
                    .capture_charge(charge_id.clone(), total_amount)
                    .map_err(ectx!(convert => charge_id, total_amount))
                    .and_then(move |_| {
                        stripe_client_clone
                            .create_payout(total_amount, currency, order_id)
                            .map_err(ectx!(convert => total_amount, currency, order_id))
                    })
            })
    })
    .and_then({
        let db_pool = db_pool.clone();
        let cpu_pool = cpu_pool.clone();
        let repo_factory = repo_factory.clone();
        move |_| {
            spawn_on_pool(db_pool, cpu_pool, move |conn| {
                let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
                info!("Setting order {} state \'Captured\'", order_id);
                orders_repo
                    .update_state(order_id, PaymentState::Captured)
                    .map_err(ectx!(convert => order_id))
                    .map(|_| ())
            })
        }
    });
    Box::new(fut)
}

fn order_decline_fiat<T, F, M>(
    cpu_pool: CpuPool,
    db_pool: Pool<M>,
    repo_factory: F,
    user_id: Option<UserId>,
    stripe_client: std::sync::Arc<dyn StripeClient>,
    order: RawOrder,
) -> ServiceFutureV2<()>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    F: ReposFactory<T>,
    M: ManageConnection<Connection = T>,
{
    let db_pool_ = db_pool.clone();
    let cpu_pool_ = cpu_pool.clone();
    let repo_factory_ = repo_factory.clone();
    let order_id = order.id;

    let fut = spawn_on_pool(db_pool_, cpu_pool_, move |conn| {
        let payment_intent_repo = repo_factory_.create_payment_intent_repo(&conn, user_id);
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
    })
    .and_then({
        let db_pool = db_pool.clone();
        let cpu_pool = cpu_pool.clone();
        let repo_factory = repo_factory.clone();
        move |_| {
            spawn_on_pool(db_pool, cpu_pool, move |conn| {
                let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
                info!("Setting order {} state \'Declined\'", order_id);
                orders_repo
                    .update_state(order_id, PaymentState::Declined)
                    .map_err(ectx!(convert => order_id))
                    .map(|_| ())
            })
        }
    });
    Box::new(fut)
}

fn order_capture_crypto<T, F, M>(
    cpu_pool: CpuPool,
    db_pool: Pool<M>,
    repo_factory: F,
    user_id: Option<UserId>,
    order: RawOrder,
) -> ServiceFutureV2<()>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    F: ReposFactory<T>,
    M: ManageConnection<Connection = T>,
{
    let order_id = order.id;

    let fut = spawn_on_pool(db_pool, cpu_pool, move |conn| {
        let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
        info!("Setting order {} state \'Captured\'", order_id);
        orders_repo
            .update_state(order_id, PaymentState::Captured)
            .map_err(ectx!(convert => order_id))
            .map(|_| ())
    });
    Box::new(fut)
}

fn order_decline_crypto<T, F, M>(
    cpu_pool: CpuPool,
    db_pool: Pool<M>,
    repo_factory: F,
    user_id: Option<UserId>,
    order: RawOrder,
) -> ServiceFutureV2<()>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    F: ReposFactory<T>,
    M: ManageConnection<Connection = T>,
{
    let order_id = order.id;

    let fut = spawn_on_pool(db_pool, cpu_pool, move |conn| {
        let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
        info!("Setting order {} state \'RefundNeeded\'", order_id);
        orders_repo
            .update_state(order_id, PaymentState::RefundNeeded)
            .map_err(ectx!(convert => order_id))
            .map(|_| ())
    });
    Box::new(fut)
}
