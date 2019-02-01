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
use models::order_v2::OrderId;
use models::{Fee, FeeId, NewFee, UpdateFee, UserRole};

use schema::fees::dsl as FeesDsl;
use schema::orders::dsl as OrdersDsl;
use schema::roles::dsl as UserRolesDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type FeeRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, Fee>>;

#[derive(Debug, Clone)]
pub enum SearchFee {
    Id(FeeId),
    OrderId(OrderId),
}

pub struct FeeRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: FeeRepoAcl,
}

pub trait FeeRepo {
    fn get(&self, search: SearchFee) -> RepoResultV2<Option<Fee>>;
    fn create(&self, payload: NewFee) -> RepoResultV2<Fee>;
    fn update(&self, fee_id: FeeId, payload: UpdateFee) -> RepoResultV2<Fee>;
    fn delete(&self, fee_id: FeeId) -> RepoResultV2<()>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> FeeRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: FeeRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> FeeRepo for FeeRepoImpl<'a, T> {
    fn get(&self, search: SearchFee) -> RepoResultV2<Option<Fee>> {
        debug!("Getting a fee by search term: {:?}", search);

        let search_exp: Box<BoxableExpression<FeesDsl::fees, _, SqlType = Bool>> = match search {
            SearchFee::Id(fee_id) => Box::new(FeesDsl::id.eq(fee_id)),
            SearchFee::OrderId(order_id) => Box::new(FeesDsl::order_id.eq(order_id)),
        };

        let query = FeesDsl::fees.filter(search_exp);

        query
            .get_result(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|fee: Option<Fee>| {
                if let Some(ref fee) = fee {
                    acl::check(&*self.acl, Resource::Fee, Action::Read, self, Some(&fee)).map_err(ectx!(try ErrorKind::Forbidden))?;
                };
                Ok(fee)
            })
    }

    fn create(&self, payload: NewFee) -> RepoResultV2<Fee> {
        debug!("Create a fee with ID: {:?}", payload);
        acl::check(&*self.acl, Resource::Fee, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(FeesDsl::fees).values(&payload);

        command.get_result::<Fee>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn update(&self, fee_id: FeeId, payload: UpdateFee) -> RepoResultV2<Fee> {
        debug!("Updating a fee with ID: {}", fee_id);

        FeesDsl::fees
            .filter(FeesDsl::id.eq(&fee_id))
            .get_result(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|fee: Fee| {
                acl::check(&*self.acl, Resource::Fee, Action::Write, self, Some(&fee)).map_err(ectx!(try ErrorKind::Forbidden))?;

                let filter = FeesDsl::fees.filter(FeesDsl::id.eq(&fee_id));

                let query = diesel::update(filter).set(&payload);
                query.get_result::<Fee>(self.db_conn).map_err(|e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, ErrorSource::Diesel, error_kind)
                })
            })
    }

    fn delete(&self, fee_id: FeeId) -> RepoResultV2<()> {
        debug!("Deleting a fee with ID: {}", fee_id);

        FeesDsl::fees
            .filter(FeesDsl::id.eq(&fee_id))
            .get_result::<Fee>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|fee: Fee| {
                acl::check(&*self.acl, Resource::Fee, Action::Write, self, Some(&fee)).map_err(ectx!(try ErrorKind::Forbidden))?;

                let command = diesel::delete(FeesDsl::fees.filter(FeesDsl::id.eq(&fee_id)));

                command
                    .get_result::<Fee>(self.db_conn)
                    .map_err(|e| {
                        let error_kind = ErrorKind::from(&e);
                        ectx!(err e, ErrorSource::Diesel, error_kind)
                    })
                    .map(|_| ())
            })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, Fee> for FeeRepoImpl<'a, T> {
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&Fee>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(Fee { order_id, .. }) = obj {
                    let store_id = match OrdersDsl::orders
                        .filter(OrdersDsl::id.eq(order_id))
                        .select(OrdersDsl::store_id)
                        .get_result::<stq_types::StoreId>(self.db_conn)
                    {
                        Ok(store_id) => store_id,
                        Err(_) => return false,
                    };

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
