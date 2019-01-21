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
use models::{NewStoreBillingType, StoreBillingType, StoreBillingTypeSearch};

use schema::store_billing_type::dsl as StoreBillingTypeDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type StoreBillingTypeRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, StoreBillingType>>;

type BoxedExpr = Box<BoxableExpression<crate::schema::store_billing_type::table, Pg, SqlType = Bool>>;

pub struct StoreBillingTypeRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: StoreBillingTypeRepoAcl,
}

pub trait StoreBillingTypeRepo {
    fn create(&self, new_store_billing_type: NewStoreBillingType) -> RepoResultV2<StoreBillingType>;
    fn get(&self, search: StoreBillingTypeSearch) -> RepoResultV2<Option<StoreBillingType>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> StoreBillingTypeRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: StoreBillingTypeRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> StoreBillingTypeRepo
    for StoreBillingTypeRepoImpl<'a, T>
{
    fn create(&self, new_store_billing_type: NewStoreBillingType) -> RepoResultV2<StoreBillingType> {
        debug!("create store billing type {:?}.", new_store_billing_type);
        acl::check(&*self.acl, Resource::StoreBillingType, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(StoreBillingTypeDsl::store_billing_type).values(&new_store_billing_type);

        command.get_result::<StoreBillingType>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn get(&self, search: StoreBillingTypeSearch) -> RepoResultV2<Option<StoreBillingType>> {
        debug!("get store billing type {:?}.", search);
        acl::check(&*self.acl, Resource::StoreBillingType, Action::Read, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let query: Option<BoxedExpr> = into_expr(search);

        let query = query.ok_or_else(|| {
            let e = format_err!("store billing type search is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        crate::schema::store_billing_type::table
            .filter(query)
            .get_result(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, StoreBillingType>
    for StoreBillingTypeRepoImpl<'a, T>
{
    fn is_in_scope(&self, _user_id: stq_types::UserId, _scope: &Scope, _obj: Option<&StoreBillingType>) -> bool {
        true
    }
}

fn into_expr(search: StoreBillingTypeSearch) -> Option<BoxedExpr> {
    let mut query: Option<BoxedExpr> = None;

    if let Some(id_filter) = search.id {
        let new_condition = StoreBillingTypeDsl::id.eq(id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(store_id_filter) = search.store_id {
        let new_condition = StoreBillingTypeDsl::store_id.eq(store_id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(billing_type_filter) = search.billing_type {
        let new_condition = StoreBillingTypeDsl::billing_type.eq(billing_type_filter);
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
