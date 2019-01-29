use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::sql_types::Bool;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;

use models::authorization::*;
use models::{NewProxyCompanyBillingInfo, ProxyCompanyBillingInfo, ProxyCompanyBillingInfoSearch, UpdateProxyCompanyBillingInfo};
use repos::legacy_acl::*;

use schema::proxy_companies_billing_info::dsl as ProxyCompanyBillingInfoDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type ProxyCompanyBillingInfoRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, ProxyCompanyBillingInfoAccess>>;

type BoxedExpr = Box<BoxableExpression<crate::schema::proxy_companies_billing_info::table, Pg, SqlType = Bool>>;

pub struct ProxyCompanyBillingInfoRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: ProxyCompanyBillingInfoRepoAcl,
}

pub struct ProxyCompanyBillingInfoAccess {}

pub trait ProxyCompanyBillingInfoRepo {
    fn create(&self, new_proxy_companies_billing_info: NewProxyCompanyBillingInfo) -> RepoResultV2<ProxyCompanyBillingInfo>;
    fn get(&self, search: ProxyCompanyBillingInfoSearch) -> RepoResultV2<Option<ProxyCompanyBillingInfo>>;
    fn update(
        &self,
        search_params: ProxyCompanyBillingInfoSearch,
        payload: UpdateProxyCompanyBillingInfo,
    ) -> RepoResultV2<ProxyCompanyBillingInfo>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> ProxyCompanyBillingInfoRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: ProxyCompanyBillingInfoRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> ProxyCompanyBillingInfoRepo
    for ProxyCompanyBillingInfoRepoImpl<'a, T>
{
    fn create(&self, new_proxy_companies_billing_info: NewProxyCompanyBillingInfo) -> RepoResultV2<ProxyCompanyBillingInfo> {
        debug!("create proxy company billing info {:?}.", new_proxy_companies_billing_info);
        acl::check(&*self.acl, Resource::ProxyCompanyBillingInfo, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let command =
            diesel::insert_into(ProxyCompanyBillingInfoDsl::proxy_companies_billing_info).values(&new_proxy_companies_billing_info);

        command.get_result::<ProxyCompanyBillingInfo>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn get(&self, search_params: ProxyCompanyBillingInfoSearch) -> RepoResultV2<Option<ProxyCompanyBillingInfo>> {
        debug!("get proxy company billing info {:?}.", search_params);
        acl::check(&*self.acl, Resource::ProxyCompanyBillingInfo, Action::Read, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let query: Option<BoxedExpr> = into_expr(search_params);

        let query = query.ok_or_else(|| {
            let e = format_err!("store billing info search_params is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let mut billing_info_list = crate::schema::proxy_companies_billing_info::table
            .filter(query)
            .get_results::<ProxyCompanyBillingInfo>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        if billing_info_list.len() > 1 {
            let e = format_err!("store international billing search returned more than 1 entry");
            return Err(ectx!(err e, ErrorKind::Internal));
        }

        Ok(billing_info_list.pop())
    }

    fn update(
        &self,
        search_params: ProxyCompanyBillingInfoSearch,
        payload: UpdateProxyCompanyBillingInfo,
    ) -> RepoResultV2<ProxyCompanyBillingInfo> {
        debug!("update proxy company billing info {:?}.", search_params);
        acl::check(&*self.acl, Resource::ProxyCompanyBillingInfo, Action::Read, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let _updated_entry = self.get(search_params.clone())?;

        let query: Option<BoxedExpr> = into_expr(search_params);

        let query = query.ok_or_else(|| {
            let e = format_err!("proxy company billing info search_params is empty");
            ectx!(try err e, ErrorKind::Internal)
        })?;

        let query = diesel::update(crate::schema::proxy_companies_billing_info::table.filter(query)).set(&payload);
        query.get_result::<ProxyCompanyBillingInfo>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static>
    CheckScope<Scope, ProxyCompanyBillingInfoAccess> for ProxyCompanyBillingInfoRepoImpl<'a, T>
{
    fn is_in_scope(&self, _user_id: stq_types::UserId, _scope: &Scope, _obj: Option<&ProxyCompanyBillingInfoAccess>) -> bool {
        true
    }
}

fn into_expr(search: ProxyCompanyBillingInfoSearch) -> Option<BoxedExpr> {
    let mut query: Option<BoxedExpr> = None;

    let ProxyCompanyBillingInfoSearch { id, country_alpha3 } = search;

    if let Some(id_filter) = id {
        let new_condition = ProxyCompanyBillingInfoDsl::id.eq(id_filter);
        query = Some(and(query, Box::new(new_condition)));
    }

    if let Some(country_alpha3_filter) = country_alpha3 {
        let new_condition = ProxyCompanyBillingInfoDsl::country_alpha3.eq(country_alpha3_filter);
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
