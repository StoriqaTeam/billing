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
use models::{NewSubscriptionPayment, SubscriptionPayment, SubscriptionPaymentSearch, SubscriptionPaymentSearchResults, UserRole};
use repos::legacy_acl::*;

use schema::roles::dsl as UserRolesDsl;
use schema::subscription_payment::dsl as SubscriptionPaymentDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

pub type SubscriptionPaymentRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, SubscriptionPaymentAccess>>;

type BoxedExpr = Box<BoxableExpression<crate::schema::subscription_payment::table, Pg, SqlType = Bool>>;

pub struct SubscriptionPaymentAccess {
    store_id: StoreId,
}

pub struct SubscriptionPaymentRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: SubscriptionPaymentRepoAcl,
}

pub trait SubscriptionPaymentRepo {
    fn create(&self, new_store_subscription: NewSubscriptionPayment) -> RepoResultV2<SubscriptionPayment>;
    fn get(&self, search: SubscriptionPaymentSearch) -> RepoResultV2<Option<SubscriptionPayment>>;
    fn search(&self, skip: i64, count: i64, search_params: SubscriptionPaymentSearch) -> RepoResultV2<SubscriptionPaymentSearchResults>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> SubscriptionPaymentRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: SubscriptionPaymentRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> SubscriptionPaymentRepo
    for SubscriptionPaymentRepoImpl<'a, T>
{
    fn create(&self, new_subscription_payment: NewSubscriptionPayment) -> RepoResultV2<SubscriptionPayment> {
        debug!("create subscription payment {:?}.", new_subscription_payment);
        acl::check(
            &*self.acl,
            Resource::SubscriptionPayment,
            Action::Write,
            self,
            Some(&SubscriptionPaymentAccess {
                store_id: new_subscription_payment.store_id,
            }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(SubscriptionPaymentDsl::subscription_payment).values(&new_subscription_payment);

        command.get_result::<SubscriptionPayment>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn get(&self, search: SubscriptionPaymentSearch) -> RepoResultV2<Option<SubscriptionPayment>> {
        debug!("get subscription payment {:?}.", search);

        let query: Option<BoxedExpr> = into_expr(search);

        let query = query.ok_or_else(|| {
            let e = format_err!("subscription payment search is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let subscription_payment = crate::schema::subscription_payment::table
            .filter(query)
            .get_result::<SubscriptionPayment>(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        if let Some(ref subscription_payment) = subscription_payment {
            acl::check(
                &*self.acl,
                Resource::SubscriptionPayment,
                Action::Read,
                self,
                Some(&SubscriptionPaymentAccess {
                    store_id: subscription_payment.store_id,
                }),
            )
            .map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(subscription_payment)
    }

    fn search(&self, skip: i64, count: i64, search_params: SubscriptionPaymentSearch) -> RepoResultV2<SubscriptionPaymentSearchResults> {
        debug!(
            "Searching subscription payments, skip={}, count={}, search {:?}",
            skip, count, search_params
        );
        let query: BoxedExpr = into_expr(search_params).unwrap_or(Box::new(true.into_sql::<Bool>()));

        let subscription_payments = crate::schema::subscription_payment::table
            .filter(&query)
            .offset(skip)
            .limit(count)
            .order_by(SubscriptionPaymentDsl::created_at.desc())
            .get_results::<SubscriptionPayment>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        let total_count = SubscriptionPaymentDsl::subscription_payment
            .filter(&query)
            .count()
            .get_result::<i64>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        let store_ids: HashSet<StoreId> = subscription_payments.iter().map(|s| s.store_id).collect();

        for store_id in store_ids {
            acl::check(
                &*self.acl,
                Resource::SubscriptionPayment,
                Action::Read,
                self,
                Some(&SubscriptionPaymentAccess { store_id }),
            )
            .map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(SubscriptionPaymentSearchResults {
            total_count,
            subscription_payments,
        })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, SubscriptionPaymentAccess>
    for SubscriptionPaymentRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: UserId, scope: &Scope, obj: Option<&SubscriptionPaymentAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(SubscriptionPaymentAccess { store_id }) = obj {
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

fn into_expr(search: SubscriptionPaymentSearch) -> Option<BoxedExpr> {
    let mut query: Option<BoxedExpr> = None;

    let SubscriptionPaymentSearch { id, store_id, status } = search;

    if let Some(id_filter) = id {
        let new_condition = SubscriptionPaymentDsl::id.eq(id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(store_id_filter) = store_id {
        let new_condition = SubscriptionPaymentDsl::store_id.eq(store_id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(status_filter) = status {
        let new_condition = SubscriptionPaymentDsl::status.eq(status_filter);
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
