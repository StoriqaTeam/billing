use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::sql_types::Bool;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;

use stq_types::{StoreId, UserId};

use models::authorization::*;
use models::{NewStoreSubscription, StoreSubscription, StoreSubscriptionSearch, UserRole};
use repos::legacy_acl::*;

use schema::roles::dsl as UserRolesDsl;
use schema::store_subscription::dsl as StoreSubscriptionDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

pub type StoreSubscriptionRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, StoreSubscriptionAccess>>;

type BoxedExpr = Box<BoxableExpression<crate::schema::store_subscription::table, Pg, SqlType = Bool>>;

pub struct StoreSubscriptionAccess {
    store_id: StoreId,
}

pub struct StoreSubscriptionRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: StoreSubscriptionRepoAcl,
}

pub trait StoreSubscriptionRepo {
    fn create(&self, new_store_subscription: NewStoreSubscription) -> RepoResultV2<StoreSubscription>;
    fn get(&self, search: StoreSubscriptionSearch) -> RepoResultV2<Option<StoreSubscription>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> StoreSubscriptionRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: StoreSubscriptionRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> StoreSubscriptionRepo
    for StoreSubscriptionRepoImpl<'a, T>
{
    fn create(&self, new_subscription: NewStoreSubscription) -> RepoResultV2<StoreSubscription> {
        debug!("create store subscription {:?}.", new_subscription);
        acl::check(
            &*self.acl,
            Resource::StoreSubscription,
            Action::Write,
            self,
            Some(&StoreSubscriptionAccess {
                store_id: new_subscription.store_id,
            }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(StoreSubscriptionDsl::store_subscription).values(&new_subscription);

        command.get_result::<StoreSubscription>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn get(&self, search: StoreSubscriptionSearch) -> RepoResultV2<Option<StoreSubscription>> {
        debug!("get store subscription {:?}.", search);

        let query: Option<BoxedExpr> = into_expr(search);

        let query = query.ok_or_else(|| {
            let e = format_err!("subscription search is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let store_subscription = crate::schema::store_subscription::table
            .filter(query)
            .get_result::<StoreSubscription>(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        if let Some(ref store_subscription) = store_subscription {
            acl::check(
                &*self.acl,
                Resource::Subscription,
                Action::Read,
                self,
                Some(&StoreSubscriptionAccess {
                    store_id: store_subscription.store_id,
                }),
            )
            .map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(store_subscription)
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, StoreSubscriptionAccess>
    for StoreSubscriptionRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: UserId, scope: &Scope, obj: Option<&StoreSubscriptionAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(StoreSubscriptionAccess { store_id }) = obj {
                    UserRolesDsl::roles
                        .filter(UserRolesDsl::user_id.eq(user_id))
                        .get_results::<UserRole>(self.db_conn)
                        .map_err(From::from)
                        .map(|user_roles_arg| {
                            user_roles_arg
                                .iter()
                                .any(|user_role_arg| user_role_arg.data.clone().map(|data| data == store_id.0).unwrap_or_default())
                        })
                        .unwrap_or_else(|_: FailureError| false)
                } else {
                    false
                }
            }
        }
    }
}

fn into_expr(search: StoreSubscriptionSearch) -> Option<BoxedExpr> {
    let mut query: Option<BoxedExpr> = None;

    let StoreSubscriptionSearch { id, store_id } = search;

    if let Some(id_filter) = id {
        let new_condition = StoreSubscriptionDsl::id.eq(id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(store_id_filter) = store_id {
        let new_condition = StoreSubscriptionDsl::store_id.eq(store_id_filter);
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
