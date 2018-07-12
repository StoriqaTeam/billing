//! OrderInfos repo, presents CRUD operations with db for order_info
use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;

use stq_types::{CallbackId, OrderInfoId, UserId};

use repos::legacy_acl::*;

use super::acl;
use super::types::RepoResult;
use models::authorization::*;
use models::order_info::order_info::dsl::*;
use models::{NewOrderInfo, OrderInfo, SetOrderInfoPaid};

/// OrderInfos repository, responsible for handling order_info
pub struct OrderInfoRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: Box<Acl<Resource, Action, Scope, FailureError, OrderInfo>>,
}

pub trait OrderInfoRepo {
    /// Find specific order_info by ID
    fn find(&self, order_info_id: OrderInfoId) -> RepoResult<Option<OrderInfo>>;

    /// Creates new order_info
    fn create(&self, payload: NewOrderInfo) -> RepoResult<OrderInfo>;

    /// Set specific order_info paid
    fn set_paid(&self, callback_id_arg: CallbackId) -> RepoResult<Vec<OrderInfo>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> OrderInfoRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: Box<Acl<Resource, Action, Scope, FailureError, OrderInfo>>) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> OrderInfoRepo for OrderInfoRepoImpl<'a, T> {
    /// Find specific order_info by ID
    fn find(&self, id_arg: OrderInfoId) -> RepoResult<Option<OrderInfo>> {
        order_info
            .filter(id.eq(id_arg.clone()))
            .get_result(self.db_conn)
            .optional()
            .map_err(From::from)
            .and_then(|order_info_arg: Option<OrderInfo>| {
                if let Some(ref order_info_arg) = order_info_arg {
                    acl::check(&*self.acl, Resource::OrderInfo, Action::Read, self, Some(order_info_arg))?;
                };
                Ok(order_info_arg)
            })
            .map_err(|e: FailureError| e.context(format!("Find specific order_info {:?} error occured", id_arg)).into())
    }

    /// Creates new order_info
    fn create(&self, payload: NewOrderInfo) -> RepoResult<OrderInfo> {
        let query_order_info = diesel::insert_into(order_info).values(&payload);
        query_order_info
            .get_result::<OrderInfo>(self.db_conn)
            .map_err(|e| e.context(format!("Create a new order_info {:?} error occured", payload)).into())
    }

    /// Set specific order_info paid
    fn set_paid(&self, callback_id_arg: CallbackId) -> RepoResult<Vec<OrderInfo>> {
        order_info
            .filter(callback_id.eq(callback_id_arg.clone()))
            .get_results(self.db_conn)
            .map_err(From::from)
            .and_then(|order_info_args: Vec<OrderInfo>| {
                for order_info_arg in &order_info_args {
                    acl::check(&*self.acl, Resource::OrderInfo, Action::Write, self, Some(order_info_arg))?;
                }
                Ok(order_info_args)
            })
            .and_then(|_| {
                let payload = SetOrderInfoPaid::new();
                diesel::update(order_info.filter(callback_id.eq(callback_id_arg.clone())))
                    .set(&payload)
                    .get_results::<OrderInfo>(self.db_conn)
                    .map_err(From::from)
            })
            .map_err(|e: FailureError| {
                e.context(format!("Set order info paid with callback id {:?}", callback_id_arg))
                    .into()
            })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, OrderInfo>
    for OrderInfoRepoImpl<'a, T>
{
    fn is_in_scope(&self, _order_info_id_arg: UserId, scope: &Scope, _obj: Option<&OrderInfo>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => false,
        }
    }
}
