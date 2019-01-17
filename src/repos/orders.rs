use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;

use repos::legacy_acl::*;

use models::authorization::*;
use models::invoice_v2::InvoiceId;
use models::order_v2::{NewOrder, OrderAccess, OrderId, RawOrder};
use models::PaymentState;
use models::UserId;
use schema::{invoices_v2::dsl as InvoicesV2, orders::dsl as Orders};

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type OrdersRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, OrderAccess>>;

pub struct OrdersRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: OrdersRepoAcl,
}

pub trait OrdersRepo {
    fn get(&self, order_id: OrderId) -> RepoResultV2<Option<RawOrder>>;
    fn get_many_by_invoice_id(&self, invoice_id: InvoiceId) -> RepoResultV2<Vec<RawOrder>>;
    fn create(&self, payload: NewOrder) -> RepoResultV2<RawOrder>;
    fn delete(&self, order_id: OrderId) -> RepoResultV2<Option<RawOrder>>;
    fn delete_by_invoice_id(&self, invoice_id: InvoiceId) -> RepoResultV2<Vec<RawOrder>>;
    fn update_state(&self, order_id: OrderId, state: PaymentState) -> RepoResultV2<RawOrder>;
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

        acl::check(
            &*self.acl,
            Resource::OrderInfo,
            Action::Read,
            self,
            Some(&OrderAccess { invoice_id }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let query = Orders::orders.filter(Orders::invoice_id.eq(invoice_id));

        query.get_results::<RawOrder>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
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

        acl::check(
            &*self.acl,
            Resource::OrderInfo,
            Action::Write,
            self,
            Some(&OrderAccess { invoice_id }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::delete(Orders::orders.filter(Orders::id.eq(order_id)));

        command.get_result::<RawOrder>(self.db_conn).optional().map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn delete_by_invoice_id(&self, invoice_id: InvoiceId) -> RepoResultV2<Vec<RawOrder>> {
        debug!("Deleting orders with invoice ID: {}", invoice_id);

        acl::check(
            &*self.acl,
            Resource::OrderInfo,
            Action::Write,
            self,
            Some(&OrderAccess { invoice_id }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::delete(Orders::orders.filter(Orders::invoice_id.eq(invoice_id)));

        command.get_results::<RawOrder>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
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
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, OrderAccess>
    for OrdersRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&OrderAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(OrderAccess { invoice_id }) = obj {
                    let query = InvoicesV2::invoices_v2
                        .filter(InvoicesV2::id.eq(invoice_id))
                        .select(InvoicesV2::buyer_user_id);

                    match query.get_result::<UserId>(self.db_conn).optional() {
                        Ok(None) => true,
                        Ok(Some(invoice_user_id)) => invoice_user_id.inner() == &user_id.0,
                        Err(_) => false,
                    }
                } else {
                    false
                }
            }
        }
    }
}
