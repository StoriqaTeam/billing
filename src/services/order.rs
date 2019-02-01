//! Order Services, presents CRUD operations with orders

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Fail;
use future::Either;
use futures::Future;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};
use validator::{ValidationError, ValidationErrors};

use stq_http::client::HttpClient;
use stq_types::UserId;

use super::error::{ErrorContext, ErrorKind};
use super::types::ServiceFutureV2;
use client::payments::PaymentsClient;
use client::stripe::StripeClient;
use controller::responses::{OrderResponse, OrderSearchResultsResponse};
use models::order_v2::{OrderId, OrdersSearch, RawOrder};
use models::PaymentState;
use models::{Event, EventPayload};
use repos::{ReposFactory, SearchPaymentIntent, SearchPaymentIntentInvoice};
use services::accounts::AccountService;
use services::error::Error as ServiceError;
use services::types::spawn_on_pool;
use services::Service;

pub trait OrderService {
    /// Capturing charge on order and setting order state to InProgress
    fn order_capture(&self, order_id: OrderId) -> ServiceFutureV2<()>;
    /// Refunding charge on order and setting order state to Cancel
    fn order_decline(&self, order_id: OrderId) -> ServiceFutureV2<()>;
    /// Update order payment state
    fn update_order_state(&self, order_id: OrderId, state: PaymentState) -> ServiceFutureV2<()>;
    // Search orders
    fn search_orders(&self, skip: i64, count: i64, payload: OrdersSearch) -> ServiceFutureV2<OrderSearchResultsResponse>;
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
                        Either::A(order_capture_fiat(cpu_pool, db_pool, repo_factory, order))
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

    fn update_order_state(&self, order_id: OrderId, state: PaymentState) -> ServiceFutureV2<()> {
        let repo_factory = self.static_context.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.static_context.db_pool.clone();
        let cpu_pool = self.static_context.cpu_pool.clone();

        let fut = spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
            info!("Set new payment state order by id: {}, payment_state: {:?}", order_id, state);

            let order = orders_repo.get(order_id).map_err(ectx!(try convert => order_id))?.ok_or({
                let e = format_err!("Order {} not found", order_id);
                ectx!(try err e, ErrorKind::Internal)
            })?;

            if check_change_order_payment_state(order.state, state) {
                orders_repo
                    .update_state(order_id, state)
                    .map_err(ectx!(convert => order_id, state))
                    .map(|_| ())
            } else {
                let mut errors = ValidationErrors::new();
                let mut error = ValidationError::new("wrong_state");
                error.message = Some(format!("Cannot change order state from \"{}\" to \"{}\"", order.state, state).into());
                errors.add("order", error);
                return Err(ectx!(err ErrorContext::OrderState ,ErrorKind::Validation(serde_json::to_value(errors).unwrap_or_default())));
            }
        });

        Box::new(fut)
    }

    fn search_orders(&self, skip: i64, count: i64, payload: OrdersSearch) -> ServiceFutureV2<OrderSearchResultsResponse> {
        let repo_factory = self.static_context.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.static_context.db_pool.clone();
        let cpu_pool = self.static_context.cpu_pool.clone();

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
            debug!("Requesting orders  {:?}", payload);

            let search_result = orders_repo.search(skip, count, payload).map_err(ectx!(try convert))?;
            let orders = search_result
                .orders
                .into_iter()
                .map(OrderResponse::try_from_raw_order)
                .collect::<Result<Vec<_>, ServiceError>>()?;
            Ok(OrderSearchResultsResponse {
                total_count: search_result.total_count,
                orders,
            })
        })
    }
}

fn order_capture_fiat<T, F, M>(cpu_pool: CpuPool, db_pool: Pool<M>, repo_factory: F, order: RawOrder) -> ServiceFutureV2<()>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    F: ReposFactory<T>,
    M: ManageConnection<Connection = T>,
{
    let fut = spawn_on_pool(db_pool, cpu_pool, move |conn| {
        let event_store_repo = repo_factory.create_event_store_repo_with_sys_acl(&conn);
        let event = Event::new(EventPayload::PaymentIntentCapture { order });
        event_store_repo.add_event(event.clone()).map_err(ectx!(try convert => event))?;
        Ok(())
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
        let payment_intent_invoices_repo = repo_factory_.create_payment_intent_invoices_repo(&conn, user_id);

        let order_invoice_id_cloned = order.invoice_id.clone();
        let payment_intent_invoice = payment_intent_invoices_repo
            .get(SearchPaymentIntentInvoice::InvoiceId(order.invoice_id))
            .map_err(ectx!(try convert => order_invoice_id_cloned))?
            .ok_or({
                let e = format_err!("Record payment_intent_invoice by invoice id {} not found", order.invoice_id);
                ectx!(try err e, ErrorKind::Internal)
            })?;

        let search = SearchPaymentIntent::Id(payment_intent_invoice.payment_intent_id);
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

fn check_change_order_payment_state(current_state: PaymentState, new_state: PaymentState) -> bool {
    use models::PaymentState::*;

    match (current_state, new_state) {
        (Initial, Captured)
        | (Initial, Declined)
        | (Captured, RefundNeeded)
        | (Captured, PaymentToSellerNeeded)
        | (RefundNeeded, Refunded)
        | (PaymentToSellerNeeded, PaidToSeller) => true,
        _ => {
            error!("Change state from {} to {} unreachable.", current_state, new_state);
            false
        }
    }
}
