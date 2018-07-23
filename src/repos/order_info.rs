//! OrderInfos repo, presents CRUD operations with db for order_info
use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;

use stq_static_resources::OrderState;
use stq_types::{OrderId, OrderInfoId, SagaId, UserId};

use repos::legacy_acl::*;

use super::acl;
use super::types::RepoResult;
use models::authorization::*;
use models::order_info::orders_info::dsl::*;
use models::{NewOrderInfo, NewStatus, OrderInfo};

/// OrderInfos repository, responsible for handling order_info
pub struct OrderInfoRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: Box<Acl<Resource, Action, Scope, FailureError, OrderInfo>>,
}

pub trait OrderInfoRepo {
    /// Find specific order_info by ID
    fn find(&self, order_info_id: OrderInfoId) -> RepoResult<Option<OrderInfo>>;

    /// Find specific order_info by order ID
    fn find_by_order_id(&self, order_id: OrderId) -> RepoResult<Option<OrderInfo>>;

    /// Find order_infos by saga ID
    fn find_by_saga_id(&self, saga_id: SagaId) -> RepoResult<Vec<OrderInfo>>;

    /// Creates new order_info
    fn create(&self, payload: NewOrderInfo) -> RepoResult<OrderInfo>;

    /// Set specific order_info new status
    fn update_status(&self, saga_id_arg: SagaId, new_status: OrderState) -> RepoResult<Vec<OrderInfo>>;

    /// Delete order_infos by saga ID
    fn delete_by_saga_id(&self, saga_id: SagaId) -> RepoResult<Vec<OrderInfo>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> OrderInfoRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: Box<Acl<Resource, Action, Scope, FailureError, OrderInfo>>) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> OrderInfoRepo for OrderInfoRepoImpl<'a, T> {
    /// Find specific order_info by ID
    fn find(&self, id_arg: OrderInfoId) -> RepoResult<Option<OrderInfo>> {
        orders_info
            .filter(id.eq(id_arg))
            .get_result(self.db_conn)
            .optional()
            .map_err(From::from)
            .and_then(|order_info_arg: Option<OrderInfo>| {
                if let Some(ref order_info_arg) = order_info_arg {
                    acl::check(&*self.acl, Resource::OrderInfo, Action::Read, self, Some(order_info_arg))?;
                };
                Ok(order_info_arg)
            })
            .map_err(|e: FailureError| {
                e.context(format!("Find specific order_info with id {} error occured", id_arg))
                    .into()
            })
    }

    /// Find specific order_info by order ID
    fn find_by_order_id(&self, order_id_arg: OrderId) -> RepoResult<Option<OrderInfo>> {
        orders_info
            .filter(order_id.eq(order_id_arg))
            .get_result(self.db_conn)
            .optional()
            .map_err(From::from)
            .and_then(|order_info_arg: Option<OrderInfo>| {
                if let Some(ref order_info_arg) = order_info_arg {
                    acl::check(&*self.acl, Resource::OrderInfo, Action::Read, self, Some(order_info_arg))?;
                };
                Ok(order_info_arg)
            })
            .map_err(|e: FailureError| {
                e.context(format!("Find specific order_info with order id {} error occured", order_id_arg))
                    .into()
            })
    }

    /// Find order_infos by saga ID
    fn find_by_saga_id(&self, saga_id_arg: SagaId) -> RepoResult<Vec<OrderInfo>> {
        orders_info
            .filter(saga_id.eq(saga_id_arg))
            .get_results(self.db_conn)
            .map_err(From::from)
            .and_then(|order_info_args: Vec<OrderInfo>| {
                for order_info_arg in &order_info_args {
                    acl::check(&*self.acl, Resource::OrderInfo, Action::Read, self, Some(order_info_arg))?;
                }
                Ok(order_info_args)
            })
            .map_err(|e: FailureError| {
                e.context(format!("Find order_infos by saga id {} error occured", saga_id_arg))
                    .into()
            })
    }

    /// Creates new order_info
    fn create(&self, payload: NewOrderInfo) -> RepoResult<OrderInfo> {
        let query_order_info = diesel::insert_into(orders_info).values(&payload);
        query_order_info
            .get_result::<OrderInfo>(self.db_conn)
            .map_err(From::from)
            .and_then(|order_info_arg| {
                acl::check(&*self.acl, Resource::OrderInfo, Action::Write, self, Some(&order_info_arg))?;
                Ok(order_info_arg)
            })
            .map_err(|e: FailureError| e.context(format!("Create a new order_info {:?} error occured", payload)).into())
    }

    /// Set specific order_info new status
    fn update_status(&self, saga_id_arg: SagaId, new_state: OrderState) -> RepoResult<Vec<OrderInfo>> {
        let new_status = NewStatus::new(new_state.clone());
        diesel::update(orders_info.filter(saga_id.eq(saga_id_arg)))
            .set(&new_status)
            .get_results::<OrderInfo>(self.db_conn)
            .map_err(|e| {
                e.context(format!("Set order info status {} with saga id {}", new_state, saga_id_arg))
                    .into()
            })
    }

    /// Delete order_infos by saga ID
    fn delete_by_saga_id(&self, saga_id_arg: SagaId) -> RepoResult<Vec<OrderInfo>> {
        debug!("Delete order info by saga id {}.", saga_id_arg);
        let filtered = orders_info.filter(saga_id.eq(saga_id_arg));

        let query = diesel::delete(filtered);
        query
            .get_results(self.db_conn)
            .map_err(From::from)
            .and_then(|order_info_args| {
                for order_info_arg in &order_info_args {
                    acl::check(&*self.acl, Resource::OrderInfo, Action::Write, self, Some(order_info_arg))?;
                }
                Ok(order_info_args)
            })
            .map_err(|e: FailureError| {
                e.context(format!("Delete order info by saga id {} error occured", saga_id_arg))
                    .into()
            })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, OrderInfo>
    for OrderInfoRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: UserId, scope: &Scope, obj: Option<&OrderInfo>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(obj) = obj {
                    user_id == obj.customer_id
                } else {
                    false
                }
            }
        }
    }
}
