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

use stq_types::StoreId;

use models::authorization::*;
use models::{NewRussiaBillingInfo, RussiaBillingInfo, RussiaBillingInfoSearch, UpdateRussiaBillingInfo, UserRole};
use repos::legacy_acl::*;

use schema::roles::dsl as UserRolesDsl;
use schema::russia_billing_info::dsl as RussiaBillingInfoDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type RussiaBillingInfoRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, RussiaBillingInfoAccess>>;

type BoxedExpr = Box<BoxableExpression<crate::schema::russia_billing_info::table, Pg, SqlType = Bool>>;

pub struct RussiaBillingInfoRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: RussiaBillingInfoRepoAcl,
}

pub struct RussiaBillingInfoAccess {
    pub store_id: StoreId,
}

pub trait RussiaBillingInfoRepo {
    fn create(&self, new_store_billing_type: NewRussiaBillingInfo) -> RepoResultV2<RussiaBillingInfo>;
    fn get(&self, search: RussiaBillingInfoSearch) -> RepoResultV2<Option<RussiaBillingInfo>>;
    fn search(&self, search: RussiaBillingInfoSearch) -> RepoResultV2<Vec<RussiaBillingInfo>>;
    fn update(&self, search_params: RussiaBillingInfoSearch, payload: UpdateRussiaBillingInfo) -> RepoResultV2<RussiaBillingInfo>;
    fn delete(&self, search_params: RussiaBillingInfoSearch) -> RepoResultV2<Option<RussiaBillingInfo>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> RussiaBillingInfoRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: RussiaBillingInfoRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> RussiaBillingInfoRepo
    for RussiaBillingInfoRepoImpl<'a, T>
{
    fn create(&self, new_russia_billing_info: NewRussiaBillingInfo) -> RepoResultV2<RussiaBillingInfo> {
        debug!("create russia billing info {:?}.", new_russia_billing_info);
        acl::check(
            &*self.acl,
            Resource::BillingInfo,
            Action::Write,
            self,
            Some(&RussiaBillingInfoAccess {
                store_id: new_russia_billing_info.store_id,
            }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(RussiaBillingInfoDsl::russia_billing_info).values(&new_russia_billing_info);

        command.get_result::<RussiaBillingInfo>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn get(&self, search_params: RussiaBillingInfoSearch) -> RepoResultV2<Option<RussiaBillingInfo>> {
        debug!("get russia billing info {:?}.", search_params);
        let query: Option<BoxedExpr> = into_expr(search_params);

        let query = query.ok_or_else(|| {
            let e = format_err!("russia billing info search_params is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let mut billing_info_list = crate::schema::russia_billing_info::table
            .filter(query)
            .get_results::<RussiaBillingInfo>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        if billing_info_list.len() > 1 {
            let e = format_err!("russia billing search returned more than 1 entry");
            return Err(ectx!(err e, ErrorKind::Internal));
        }

        let billing_info = billing_info_list.pop();
        if let Some(ref billing_info) = billing_info {
            let access = RussiaBillingInfoAccess {
                store_id: billing_info.store_id,
            };
            acl::check(&*self.acl, Resource::BillingInfo, Action::Read, self, Some(&access)).map_err(ectx!(try ErrorKind::Forbidden))?;
        }
        Ok(billing_info)
    }

    fn search(&self, search_params: RussiaBillingInfoSearch) -> RepoResultV2<Vec<RussiaBillingInfo>> {
        debug!("search russia billing info {:?}.", search_params);
        let query: Option<BoxedExpr> = into_expr(search_params);

        let query = query.ok_or_else(|| {
            let e = format_err!("russia billing info search_params is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let billing_info = crate::schema::russia_billing_info::table
            .filter(query)
            .get_results::<RussiaBillingInfo>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        let store_ids: HashSet<StoreId> = billing_info.iter().map(|info| info.store_id).collect();

        for store_id in store_ids {
            let access = RussiaBillingInfoAccess { store_id };
            acl::check(&*self.acl, Resource::BillingInfo, Action::Read, self, Some(&access)).map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(billing_info)
    }

    fn update(&self, search_params: RussiaBillingInfoSearch, payload: UpdateRussiaBillingInfo) -> RepoResultV2<RussiaBillingInfo> {
        debug!("update russia billing info {:?}.", search_params);
        let updated_entry = self.get(search_params.clone())?;
        let access = updated_entry
            .as_ref()
            .map(|entry| RussiaBillingInfoAccess { store_id: entry.store_id });
        acl::check(&*self.acl, Resource::BillingInfo, Action::Write, self, access.as_ref()).map_err(ectx!(try ErrorKind::Forbidden))?;
        let query: Option<BoxedExpr> = into_expr(search_params);

        let query = query.ok_or_else(|| {
            let e = format_err!("russia billing info search_params is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let query = diesel::update(crate::schema::russia_billing_info::table.filter(query)).set(&payload);
        query.get_result::<RussiaBillingInfo>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn delete(&self, search_params: RussiaBillingInfoSearch) -> RepoResultV2<Option<RussiaBillingInfo>> {
        debug!("update russia billing info {:?}.", search_params);
        let updated_entry = self.get(search_params.clone())?;
        let access = updated_entry
            .as_ref()
            .map(|entry| RussiaBillingInfoAccess { store_id: entry.store_id });
        if let Some(access) = access {
            acl::check(&*self.acl, Resource::BillingInfo, Action::Write, self, Some(&access)).map_err(ectx!(try ErrorKind::Forbidden))?;
        }
        let query: Option<BoxedExpr> = into_expr(search_params);

        let query = query.ok_or_else(|| {
            let e = format_err!("russia billing info search_params is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let query = diesel::delete(crate::schema::russia_billing_info::table.filter(query));
        query.get_result::<RussiaBillingInfo>(self.db_conn).optional().map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, RussiaBillingInfoAccess>
    for RussiaBillingInfoRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&RussiaBillingInfoAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(RussiaBillingInfoAccess { store_id }) = obj {
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

fn into_expr(search: RussiaBillingInfoSearch) -> Option<BoxedExpr> {
    let mut query: Option<BoxedExpr> = None;

    let RussiaBillingInfoSearch { id, store_id, store_ids } = search;

    if let Some(id_filter) = id {
        let new_condition = RussiaBillingInfoDsl::id.eq(id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(store_id_filter) = store_id {
        let new_condition = RussiaBillingInfoDsl::store_id.eq(store_id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(store_ids_filter) = store_ids {
        let new_condition = RussiaBillingInfoDsl::store_id.eq_any(store_ids_filter);
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
