use std::collections::HashSet;

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
use models::{NewSubscription, Subscription, SubscriptionSearch, UpdateSubscription, UserRole};
use repos::legacy_acl::*;

use schema::roles::dsl as UserRolesDsl;
use schema::subscription::dsl as SubscriptionDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

pub type SubscriptionRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, SubscriptionAccess>>;

type BoxedExpr = Box<BoxableExpression<crate::schema::subscription::table, Pg, SqlType = Bool>>;

pub struct SubscriptionAccess {
    store_id: StoreId,
}

pub struct SubscriptionRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: SubscriptionRepoAcl,
}

pub trait SubscriptionRepo {
    fn create(&self, new_subscription: NewSubscription) -> RepoResultV2<Subscription>;
    fn get(&self, search: SubscriptionSearch) -> RepoResultV2<Option<Subscription>>;
    fn get_unpaid(&self) -> RepoResultV2<Vec<Subscription>>;
    fn search(&self, search: SubscriptionSearch) -> RepoResultV2<Vec<Subscription>>;
    fn update(&self, search: SubscriptionSearch, payload: UpdateSubscription) -> RepoResultV2<Subscription>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> SubscriptionRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: SubscriptionRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> SubscriptionRepo
    for SubscriptionRepoImpl<'a, T>
{
    fn create(&self, new_subscription: NewSubscription) -> RepoResultV2<Subscription> {
        debug!("create subscription {:?}.", new_subscription);
        acl::check(
            &*self.acl,
            Resource::Subscription,
            Action::Write,
            self,
            Some(&SubscriptionAccess {
                store_id: new_subscription.store_id,
            }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(SubscriptionDsl::subscription).values(&new_subscription);

        command.get_result::<Subscription>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn get(&self, search: SubscriptionSearch) -> RepoResultV2<Option<Subscription>> {
        debug!("get subscription {:?}.", search);

        let query: Option<BoxedExpr> = into_expr(search);

        let query = query.ok_or_else(|| {
            let e = format_err!("subscription search is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let subscription = crate::schema::subscription::table
            .filter(query)
            .get_result::<Subscription>(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        if let Some(ref subscription) = subscription {
            acl::check(
                &*self.acl,
                Resource::Subscription,
                Action::Read,
                self,
                Some(&SubscriptionAccess {
                    store_id: subscription.store_id,
                }),
            )
            .map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(subscription)
    }

    fn get_unpaid(&self) -> RepoResultV2<Vec<Subscription>> {
        debug!("get unpaid subscriptions.");

        self.search(SubscriptionSearch {
            paid: Some(false),
            ..Default::default()
        })
    }

    fn search(&self, search: SubscriptionSearch) -> RepoResultV2<Vec<Subscription>> {
        debug!("search subscription {:?}.", search);

        let query: Option<BoxedExpr> = into_expr(search);

        let query = query.ok_or_else(|| {
            let e = format_err!("subscription search is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let subscriptions = crate::schema::subscription::table
            .filter(query)
            .get_results::<Subscription>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        let store_ids: HashSet<StoreId> = subscriptions.iter().map(|s| s.store_id).collect();

        for store_id in store_ids {
            acl::check(
                &*self.acl,
                Resource::Subscription,
                Action::Read,
                self,
                Some(&SubscriptionAccess { store_id }),
            )
            .map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(subscriptions)
    }

    fn update(&self, search_params: SubscriptionSearch, payload: UpdateSubscription) -> RepoResultV2<Subscription> {
        debug!("update subscription {:?}.", search_params);
        let updated_entry = self.get(search_params.clone())?;
        let access = updated_entry.as_ref().map(|entry| SubscriptionAccess { store_id: entry.store_id });
        acl::check(&*self.acl, Resource::Subscription, Action::Write, self, access.as_ref()).map_err(ectx!(try ErrorKind::Forbidden))?;
        let query: Option<BoxedExpr> = into_expr(search_params);

        let query = query.ok_or_else(|| {
            let e = format_err!("subscription search_params is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let query = diesel::update(crate::schema::subscription::table.filter(query)).set(&payload);
        query.get_result::<Subscription>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, SubscriptionAccess>
    for SubscriptionRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: UserId, scope: &Scope, obj: Option<&SubscriptionAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(SubscriptionAccess { store_id }) = obj {
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

fn into_expr(search: SubscriptionSearch) -> Option<BoxedExpr> {
    let mut query: Option<BoxedExpr> = None;

    let SubscriptionSearch {
        id,
        store_id,
        paid,
        subscription_payment_id,
    } = search;

    if let Some(id_filter) = id {
        let new_condition = SubscriptionDsl::id.eq(id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(store_id_filter) = store_id {
        let new_condition = SubscriptionDsl::store_id.eq(store_id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(paid_filter) = paid {
        let new_condition = if paid_filter {
            Box::new(SubscriptionDsl::subscription_payment_id.is_not_null()) as BoxedExpr
        } else {
            Box::new(SubscriptionDsl::subscription_payment_id.is_null()) as BoxedExpr
        };

        query = Some(and(query, new_condition));
    }

    if let Some(subscription_payment_id_filter) = subscription_payment_id {
        let new_condition = SubscriptionDsl::subscription_payment_id.eq(subscription_payment_id_filter);
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
