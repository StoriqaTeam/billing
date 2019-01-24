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
use models::{InternationalBillingInfo, InternationalBillingInfoSearch, NewInternationalBillingInfo, UpdateInternationalBillingInfo};
use repos::legacy_acl::*;

use schema::international_billing_info::dsl as InternationalBillingInfoDsl;
use schema::merchants::dsl as MerchantDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type InternationalBillingInfoRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, InternationalBillingInfoAccess>>;

type BoxedExpr = Box<BoxableExpression<crate::schema::international_billing_info::table, Pg, SqlType = Bool>>;

pub struct InternationalBillingInfoRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: InternationalBillingInfoRepoAcl,
}

pub struct InternationalBillingInfoAccess {
    pub store_id: StoreId,
}

pub trait InternationalBillingInfoRepo {
    fn create(&self, new_store_billing_type: NewInternationalBillingInfo) -> RepoResultV2<InternationalBillingInfo>;
    fn get(&self, search: InternationalBillingInfoSearch) -> RepoResultV2<Option<InternationalBillingInfo>>;
    fn search(&self, search: InternationalBillingInfoSearch) -> RepoResultV2<Vec<InternationalBillingInfo>>;
    fn update(
        &self,
        search_params: InternationalBillingInfoSearch,
        payload: UpdateInternationalBillingInfo,
    ) -> RepoResultV2<InternationalBillingInfo>;
    fn delete(&self, search_params: InternationalBillingInfoSearch) -> RepoResultV2<Option<InternationalBillingInfo>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> InternationalBillingInfoRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: InternationalBillingInfoRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> InternationalBillingInfoRepo
    for InternationalBillingInfoRepoImpl<'a, T>
{
    fn create(&self, new_international_billing_info: NewInternationalBillingInfo) -> RepoResultV2<InternationalBillingInfo> {
        debug!("create international billing info {:?}.", new_international_billing_info);
        acl::check(
            &*self.acl,
            Resource::BillingInfo,
            Action::Write,
            self,
            Some(&InternationalBillingInfoAccess {
                store_id: new_international_billing_info.store_id,
            }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(InternationalBillingInfoDsl::international_billing_info).values(&new_international_billing_info);

        command.get_result::<InternationalBillingInfo>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn get(&self, search_params: InternationalBillingInfoSearch) -> RepoResultV2<Option<InternationalBillingInfo>> {
        debug!("get international billing info {:?}.", search_params);
        let query: Option<BoxedExpr> = into_expr(search_params);

        let query = query.ok_or_else(|| {
            let e = format_err!("international billing info search_params is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let mut billing_info_list = crate::schema::international_billing_info::table
            .filter(query)
            .get_results::<InternationalBillingInfo>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        if billing_info_list.len() > 1 {
            let e = format_err!("store international billing search returned more than 1 entry");
            return Err(ectx!(err e, ErrorKind::Internal));
        }

        let billing_info = billing_info_list.pop();
        let access = billing_info
            .as_ref()
            .map(|info| InternationalBillingInfoAccess { store_id: info.store_id });
        acl::check(&*self.acl, Resource::BillingInfo, Action::Read, self, access.as_ref()).map_err(ectx!(try ErrorKind::Forbidden))?;
        Ok(billing_info)
    }

    fn search(&self, search_params: InternationalBillingInfoSearch) -> RepoResultV2<Vec<InternationalBillingInfo>> {
        debug!("get international billing info {:?}.", search_params);
        let query: Option<BoxedExpr> = into_expr(search_params);

        let query = query.ok_or_else(|| {
            let e = format_err!("international billing info search_params is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let billing_info = crate::schema::international_billing_info::table
            .filter(query)
            .get_results::<InternationalBillingInfo>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        let store_ids: HashSet<StoreId> = billing_info.iter().map(|info| info.store_id).collect();

        for store_id in store_ids {
            let access = InternationalBillingInfoAccess { store_id };
            acl::check(&*self.acl, Resource::BillingInfo, Action::Read, self, Some(&access)).map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(billing_info)
    }

    fn update(
        &self,
        search_params: InternationalBillingInfoSearch,
        payload: UpdateInternationalBillingInfo,
    ) -> RepoResultV2<InternationalBillingInfo> {
        debug!("update international billing info {:?}.", search_params);
        let updated_entry = self.get(search_params.clone())?;
        let access = updated_entry
            .as_ref()
            .map(|entry| InternationalBillingInfoAccess { store_id: entry.store_id });
        acl::check(&*self.acl, Resource::BillingInfo, Action::Write, self, access.as_ref()).map_err(ectx!(try ErrorKind::Forbidden))?;
        let query: Option<BoxedExpr> = into_expr(search_params);

        let query = query.ok_or_else(|| {
            let e = format_err!("international billing info search_params is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let query = diesel::update(crate::schema::international_billing_info::table.filter(query)).set(&payload);
        query.get_result::<InternationalBillingInfo>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn delete(&self, search_params: InternationalBillingInfoSearch) -> RepoResultV2<Option<InternationalBillingInfo>> {
        debug!("delete international billing info {:?}.", search_params);
        let deleted_entry = self.get(search_params.clone())?;
        let access = deleted_entry
            .as_ref()
            .map(|entry| InternationalBillingInfoAccess { store_id: entry.store_id });
        acl::check(&*self.acl, Resource::BillingInfo, Action::Write, self, access.as_ref()).map_err(ectx!(try ErrorKind::Forbidden))?;
        let query: Option<BoxedExpr> = into_expr(search_params);

        let query = query.ok_or_else(|| {
            let e = format_err!("international billing info search_params is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let query = diesel::delete(crate::schema::international_billing_info::table.filter(query));
        query.get_result::<InternationalBillingInfo>(self.db_conn).optional().map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static>
    CheckScope<Scope, InternationalBillingInfoAccess> for InternationalBillingInfoRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&InternationalBillingInfoAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(InternationalBillingInfoAccess { store_id }) = obj {
                    let query = MerchantDsl::merchants
                        .filter(MerchantDsl::store_id.eq(store_id))
                        .select(MerchantDsl::user_id);

                    match query.get_result::<Option<UserId>>(self.db_conn) {
                        Ok(None) => false,
                        Ok(Some(store_owner_id)) => store_owner_id == user_id,
                        Err(_) => false,
                    }
                } else {
                    false
                }
            }
        }
    }
}

fn into_expr(search: InternationalBillingInfoSearch) -> Option<BoxedExpr> {
    let mut query: Option<BoxedExpr> = None;

    if let Some(id_filter) = search.id {
        let new_condition = InternationalBillingInfoDsl::id.eq(id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(store_id_filter) = search.store_id {
        let new_condition = InternationalBillingInfoDsl::store_id.eq(store_id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(store_ids_filter) = search.store_ids {
        let new_condition = InternationalBillingInfoDsl::store_id.eq_any(store_ids_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(swift_bic_filter) = search.swift_bic {
        let new_condition = InternationalBillingInfoDsl::swift_bic.eq(swift_bic_filter);
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
