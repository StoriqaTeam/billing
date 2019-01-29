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
use models::{NewStoreBillingType, StoreBillingType, StoreBillingTypeSearch, UpdateStoreBillingType, UserRole};
use repos::legacy_acl::*;

use schema::roles::dsl as UserRolesDsl;
use schema::store_billing_type::dsl as StoreBillingTypeDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type StoreBillingTypeRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, StoreBillingTypeAccess>>;

type BoxedExpr = Box<BoxableExpression<crate::schema::store_billing_type::table, Pg, SqlType = Bool>>;

pub struct StoreBillingTypeRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: StoreBillingTypeRepoAcl,
}

pub struct StoreBillingTypeAccess {
    store_id: StoreId,
}

pub trait StoreBillingTypeRepo {
    fn create(&self, new_store_billing_type: NewStoreBillingType) -> RepoResultV2<StoreBillingType>;
    fn get(&self, search: StoreBillingTypeSearch) -> RepoResultV2<Option<StoreBillingType>>;
    fn search(&self, search: StoreBillingTypeSearch) -> RepoResultV2<Vec<StoreBillingType>>;
    fn update(&self, search: StoreBillingTypeSearch, payload: UpdateStoreBillingType) -> RepoResultV2<StoreBillingType>;
    fn delete(&self, search: StoreBillingTypeSearch) -> RepoResultV2<StoreBillingType>;
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
        acl::check(
            &*self.acl,
            Resource::StoreBillingType,
            Action::Write,
            self,
            Some(&StoreBillingTypeAccess {
                store_id: new_store_billing_type.store_id,
            }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(StoreBillingTypeDsl::store_billing_type).values(&new_store_billing_type);

        command.get_result::<StoreBillingType>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn get(&self, search: StoreBillingTypeSearch) -> RepoResultV2<Option<StoreBillingType>> {
        debug!("get store billing type {:?}.", search);

        let query: Option<BoxedExpr> = into_expr(search);

        let query = query.ok_or_else(|| {
            let e = format_err!("store billing type search is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let billing_type = crate::schema::store_billing_type::table
            .filter(query)
            .get_result::<StoreBillingType>(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        if let Some(ref billing_type) = billing_type {
            acl::check(
                &*self.acl,
                Resource::StoreBillingType,
                Action::Read,
                self,
                Some(&StoreBillingTypeAccess {
                    store_id: billing_type.store_id,
                }),
            )
            .map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(billing_type)
    }

    fn search(&self, search: StoreBillingTypeSearch) -> RepoResultV2<Vec<StoreBillingType>> {
        debug!("search store billing type {:?}.", search);

        acl::check(&*self.acl, Resource::StoreBillingType, Action::Read, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let query: Option<BoxedExpr> = into_expr(search);

        let query = query.ok_or_else(|| {
            let e = format_err!("store billing type search is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let entries = crate::schema::store_billing_type::table
            .filter(query)
            .get_results::<StoreBillingType>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        let store_ids: HashSet<StoreId> = entries.iter().map(|billing_type| billing_type.store_id).collect();

        for store_id in store_ids {
            let access = StoreBillingTypeAccess { store_id };
            acl::check(&*self.acl, Resource::StoreBillingType, Action::Read, self, Some(&access))
                .map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(entries)
    }

    fn update(&self, search_params: StoreBillingTypeSearch, payload: UpdateStoreBillingType) -> RepoResultV2<StoreBillingType> {
        debug!("update store billing type {:?}.", search_params);
        let updated_entry = self.get(search_params.clone())?;
        let access = updated_entry
            .as_ref()
            .map(|entry| StoreBillingTypeAccess { store_id: entry.store_id });
        acl::check(&*self.acl, Resource::StoreBillingType, Action::Write, self, access.as_ref())
            .map_err(ectx!(try ErrorKind::Forbidden))?;
        let query: Option<BoxedExpr> = into_expr(search_params);

        let query = query.ok_or_else(|| {
            let e = format_err!("store billing type search_params is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let query = diesel::update(crate::schema::store_billing_type::table.filter(query)).set(&payload);
        query.get_result::<StoreBillingType>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn delete(&self, search_params: StoreBillingTypeSearch) -> RepoResultV2<StoreBillingType> {
        debug!("delete store billing type {:?}.", search_params);
        let updated_entry = self.get(search_params.clone())?;
        let access = updated_entry
            .as_ref()
            .map(|entry| StoreBillingTypeAccess { store_id: entry.store_id });
        acl::check(&*self.acl, Resource::StoreBillingType, Action::Write, self, access.as_ref())
            .map_err(ectx!(try ErrorKind::Forbidden))?;
        let query: Option<BoxedExpr> = into_expr(search_params);

        let query = query.ok_or_else(|| {
            let e = format_err!("store billing type search_params is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let query = diesel::delete(crate::schema::store_billing_type::table.filter(query));
        query.get_result::<StoreBillingType>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, StoreBillingTypeAccess>
    for StoreBillingTypeRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: UserId, scope: &Scope, obj: Option<&StoreBillingTypeAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(StoreBillingTypeAccess { store_id }) = obj {
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

    if let Some(store_ids_filter) = search.store_ids {
        let new_condition = StoreBillingTypeDsl::store_id.eq_any(store_ids_filter);
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
