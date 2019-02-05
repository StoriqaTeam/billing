use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::sql_types::Bool;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;

use repos::legacy_acl::*;

use models::authorization::*;
use models::invoice_v2::InvoiceId;
use models::order_v2::{NewOrder, OrderAccess, OrderId, OrderSearchResults, OrdersSearch, RawOrder, StoreId};
use models::{Amount, PaymentState, UserId, UserRole};
use schema::roles::dsl as UserRolesDsl;
use schema::{invoices_v2::dsl as InvoicesV2, orders::dsl as Orders};

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type OrdersRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, OrderAccess>>;

type BoxedExpr = Box<BoxableExpression<crate::schema::orders::table, Pg, SqlType = Bool>>;

pub struct OrdersRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: OrdersRepoAcl,
}

pub trait OrdersRepo {
    fn get(&self, order_id: OrderId) -> RepoResultV2<Option<RawOrder>>;
    fn get_many_by_invoice_id(&self, invoice_id: InvoiceId) -> RepoResultV2<Vec<RawOrder>>;
    fn search(&self, skip: i64, count: i64, search: OrdersSearch) -> RepoResultV2<OrderSearchResults>;
    fn create(&self, payload: NewOrder) -> RepoResultV2<RawOrder>;
    fn delete(&self, order_id: OrderId) -> RepoResultV2<Option<RawOrder>>;
    fn delete_by_invoice_id(&self, invoice_id: InvoiceId) -> RepoResultV2<Vec<RawOrder>>;
    fn update_state(&self, order_id: OrderId, state: PaymentState) -> RepoResultV2<RawOrder>;
    fn update_stripe_fee(&self, order_id: OrderId, stripe_fee: Amount) -> RepoResultV2<RawOrder>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> OrdersRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: OrdersRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> OrdersRepo for OrdersRepoImpl<'a, T> {
    fn get(&self, order_id: OrderId) -> RepoResultV2<Option<RawOrder>> {
        debug!("Getting an order with ID: {}", order_id);

        let query = Orders::orders.filter(Orders::id.eq(order_id));

        query
            .get_result(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|order: Option<RawOrder>| {
                if let Some(ref order) = order {
                    acl::check(
                        &*self.acl,
                        Resource::OrderInfo,
                        Action::Read,
                        self,
                        Some(&OrderAccess::from(order.clone())),
                    )
                    .map_err(ectx!(try ErrorKind::Forbidden))?;
                };
                Ok(order)
            })
    }

    fn get_many_by_invoice_id(&self, invoice_id: InvoiceId) -> RepoResultV2<Vec<RawOrder>> {
        debug!("Getting orders with invoice ID: {}", invoice_id);

        let query = Orders::orders.filter(Orders::invoice_id.eq(invoice_id));

        let results = query.get_results::<RawOrder>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(try err e, ErrorSource::Diesel, error_kind)
        })?;

        for result in &results {
            acl::check(
                &*self.acl,
                Resource::OrderInfo,
                Action::Read,
                self,
                Some(&OrderAccess {
                    invoice_id,
                    store_id: result.store_id,
                }),
            )
            .map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(results)
    }

    fn search(&self, skip: i64, count: i64, search_params: OrdersSearch) -> RepoResultV2<OrderSearchResults> {
        debug!("Searching orders, skip={}, count={}, search {:?}", skip, count, search_params);
        let query: BoxedExpr = into_expr(search_params).unwrap_or(Box::new(true.into_sql::<Bool>()));

        let orders = Orders::orders
            .filter(&query)
            .offset(skip)
            .limit(count)
            .order_by(Orders::created_at.desc())
            .get_results::<RawOrder>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        let total_count = Orders::orders.filter(&query).count().get_result::<i64>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(try err e, ErrorSource::Diesel, error_kind)
        })?;

        for order in &orders {
            acl::check(
                &*self.acl,
                Resource::OrderInfo,
                Action::Read,
                self,
                Some(&OrderAccess {
                    invoice_id: order.invoice_id,
                    store_id: order.store_id,
                }),
            )
            .map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(OrderSearchResults { total_count, orders })
    }

    fn create(&self, payload: NewOrder) -> RepoResultV2<RawOrder> {
        debug!("Creating an order using payload: {:?}", payload);

        acl::check(&*self.acl, Resource::OrderInfo, Action::Write, self, Some(&payload.clone().into()))
            .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(Orders::orders).values(&payload);

        command.get_result::<RawOrder>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn delete(&self, order_id: OrderId) -> RepoResultV2<Option<RawOrder>> {
        debug!("Deleting an order with ID: {}", order_id);

        let invoice_id = Orders::orders
            .filter(Orders::id.eq(order_id))
            .select(Orders::invoice_id)
            .get_result::<InvoiceId>(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        let invoice_id = match invoice_id {
            None => {
                return Ok(None);
            }
            Some(invoice_id) => invoice_id,
        };

        let command = diesel::delete(Orders::orders.filter(Orders::id.eq(order_id)));

        let deleted_order = command.get_result::<RawOrder>(self.db_conn).optional().map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(try err e, ErrorSource::Diesel, error_kind)
        })?;

        if let Some(ref deleted_order) = deleted_order {
            acl::check(
                &*self.acl,
                Resource::OrderInfo,
                Action::Write,
                self,
                Some(&OrderAccess {
                    invoice_id,
                    store_id: deleted_order.store_id,
                }),
            )
            .map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(deleted_order)
    }

    fn delete_by_invoice_id(&self, invoice_id: InvoiceId) -> RepoResultV2<Vec<RawOrder>> {
        debug!("Deleting orders with invoice ID: {}", invoice_id);

        let command = diesel::delete(Orders::orders.filter(Orders::invoice_id.eq(invoice_id)));

        let deleted_orders = command.get_results::<RawOrder>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(try err e, ErrorSource::Diesel, error_kind)
        })?;

        for deleted_order in &deleted_orders {
            acl::check(
                &*self.acl,
                Resource::OrderInfo,
                Action::Write,
                self,
                Some(&OrderAccess {
                    invoice_id,
                    store_id: deleted_order.store_id,
                }),
            )
            .map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(deleted_orders)
    }

    fn update_state(&self, order_id: OrderId, state: PaymentState) -> RepoResultV2<RawOrder> {
        debug!("Updating state of order with ID: {} - {}", order_id, state);

        acl::check(&*self.acl, Resource::OrderInfo, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let filter = Orders::orders.filter(Orders::id.eq(order_id));

        let query = diesel::update(filter).set(Orders::state.eq(state));
        query.get_result::<RawOrder>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }
    fn update_stripe_fee(&self, order_id: OrderId, stripe_fee: Amount) -> RepoResultV2<RawOrder> {
        debug!("Updating stripe_fee of order with ID: {} - {}", order_id, stripe_fee);

        acl::check(&*self.acl, Resource::OrderInfo, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let filter = Orders::orders.filter(Orders::id.eq(order_id));

        let query = diesel::update(filter).set(Orders::stripe_fee.eq(stripe_fee));
        query.get_result::<RawOrder>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, OrderAccess>
    for OrdersRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&OrderAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(OrderAccess { invoice_id, store_id }) = obj {
                    user_is_buyer(self.db_conn, user_id, invoice_id.clone())
                        || user_is_store_manager(self.db_conn, user_id, store_id.clone())
                } else {
                    false
                }
            }
        }
    }
}

fn user_is_buyer<T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static>(
    conn: &T,
    user_id: stq_types::UserId,
    invoice_id: InvoiceId,
) -> bool {
    let query = InvoicesV2::invoices_v2
        .filter(InvoicesV2::id.eq(invoice_id))
        .select(InvoicesV2::buyer_user_id);

    match query.get_result::<UserId>(conn).optional() {
        Ok(None) => true,
        Ok(Some(invoice_user_id)) => invoice_user_id.inner() == user_id.0,
        Err(_) => false,
    }
}

fn user_is_store_manager<T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static>(
    conn: &T,
    user_id: stq_types::UserId,
    store_id: StoreId,
) -> bool {
    UserRolesDsl::roles
        .filter(UserRolesDsl::user_id.eq(user_id))
        .get_results::<UserRole>(conn)
        .map_err(From::from)
        .map(|user_roles_arg| {
            user_roles_arg
                .iter()
                .any(|user_role_arg| user_role_arg.data.clone().map(|data| data == store_id.inner()).unwrap_or_default())
        })
        .unwrap_or_else(|_: FailureError| false)
}

fn into_expr(search: OrdersSearch) -> Option<BoxedExpr> {
    let mut query: Option<BoxedExpr> = None;

    let OrdersSearch { store_id, state, order_id } = search;

    if let Some(store_id_filter) = store_id {
        let new_condition = Orders::store_id.eq(store_id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(state_filter) = state {
        let new_condition = Orders::state.eq(state_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(order_id_filter) = order_id {
        let new_condition = Orders::id.eq(order_id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    query
}

fn and(old_condition: Option<BoxedExpr>, new_condition: BoxedExpr) -> BoxedExpr {
    if let Some(old_condition) = old_condition {
        Box::new(old_condition.and(new_condition))
    } else {
        new_condition
    }
}
