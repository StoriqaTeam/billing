//! Repo for merchants table.

use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::Connection;
use failure::Error as FailureError;

use stq_types::{MerchantId, StoreId, UserId};

use repos::legacy_acl::*;

use super::acl;
use super::types::RepoResult;
use models::authorization::*;
use models::merchant::merchants::dsl::*;
use models::role::roles::dsl as Roles;
use models::{Merchant, NewStoreMerchant, NewUserMerchant, SubjectIdentifier, UserRole};

/// Merchant repository for handling Merchant
pub trait MerchantRepo {
    /// Returns merchant by subject identifier
    fn get_by_subject_id(&self, id: SubjectIdentifier) -> RepoResult<Merchant>;

    /// Returns merchant by merchant identifier
    fn get_by_merchant_id(&self, id: MerchantId) -> RepoResult<Merchant>;

    /// Create a new store merchant
    fn create_store_merchant(&self, payload: NewStoreMerchant) -> RepoResult<Merchant>;

    /// Delete store merchant
    fn delete_by_store_id(&self, store_id: StoreId) -> RepoResult<Merchant>;

    /// Create a new user merchant
    fn create_user_merchant(&self, payload: NewUserMerchant) -> RepoResult<Merchant>;

    /// Delete user merchant
    fn delete_by_user_id(&self, user_id: UserId) -> RepoResult<Merchant>;
}

/// Implementation of Merchant trait
pub struct MerchantRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: Box<Acl<Resource, Action, Scope, FailureError, Merchant>>,
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> MerchantRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: Box<Acl<Resource, Action, Scope, FailureError, Merchant>>) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> MerchantRepo for MerchantRepoImpl<'a, T> {
    /// Returns merchant by subject identifier
    fn get_by_subject_id(&self, id: SubjectIdentifier) -> RepoResult<Merchant> {
        debug!("Returns merchant by id {:?} from db.", id);
        let query = match id {
            SubjectIdentifier::Store(store_ident) => merchants
                .filter(store_id.eq(Some(store_ident)))
                .get_result::<Merchant>(self.db_conn),
            SubjectIdentifier::User(user_ident) => merchants.filter(user_id.eq(Some(user_ident))).get_result::<Merchant>(self.db_conn),
        };
        query
            .map_err(From::from)
            .and_then(|merch| {
                acl::check(&*self.acl, Resource::Merchant, Action::Read, self, Some(&merch))?;
                Ok(merch)
            })
            .map_err(|e: FailureError| e.context(format!("get by subject id {:?} error occured.", id)).into())
    }

    /// Returns merchant by merchant identifier
    fn get_by_merchant_id(&self, id: MerchantId) -> RepoResult<Merchant> {
        debug!("Returns merchant by merchant id {} from db.", id);
        let query = merchants.filter(merchant_id.eq(id)).get_result::<Merchant>(self.db_conn);
        query
            .map_err(From::from)
            .and_then(|merch| {
                acl::check(&*self.acl, Resource::Merchant, Action::Read, self, Some(&merch))?;
                Ok(merch)
            })
            .map_err(|e: FailureError| e.context(format!("get by merchant id {} error occured.", id)).into())
    }

    /// Create a new store merchant
    fn create_store_merchant(&self, payload: NewStoreMerchant) -> RepoResult<Merchant> {
        debug!("create new store merchant {} in db.", payload);
        let query = diesel::insert_into(merchants).values(&payload);
        query
            .get_result(self.db_conn)
            .map_err(From::from)
            .and_then(|merch| {
                acl::check(&*self.acl, Resource::Merchant, Action::Write, self, Some(&merch))?;
                Ok(merch)
            })
            .map_err(|e: FailureError| e.context(format!("Create a new store merchant {:?} error occured", payload)).into())
    }

    /// Delete store merchant
    fn delete_by_store_id(&self, store_id_arg: StoreId) -> RepoResult<Merchant> {
        debug!("Delete store {} merchant from db.", store_id_arg);
        let filtered = merchants.filter(store_id.eq(Some(store_id_arg)));

        let query = diesel::delete(filtered);
        query
            .get_result(self.db_conn)
            .map_err(From::from)
            .and_then(|merch| {
                acl::check(&*self.acl, Resource::Merchant, Action::Write, self, Some(&merch))?;
                Ok(merch)
            })
            .map_err(|e: FailureError| e.context(format!("Delete store {} merchant error occured", store_id_arg)).into())
    }

    /// Create a new user merchant
    fn create_user_merchant(&self, payload: NewUserMerchant) -> RepoResult<Merchant> {
        debug!("Create new user merchant {} in db.", payload);
        let query = diesel::insert_into(merchants).values(&payload);
        query
            .get_result(self.db_conn)
            .map_err(From::from)
            .and_then(|merch| {
                acl::check(&*self.acl, Resource::Merchant, Action::Write, self, Some(&merch))?;
                Ok(merch)
            })
            .map_err(|e: FailureError| e.context(format!("Create a new user merchant {:?} error occured", payload)).into())
    }

    /// Delete user merchant
    fn delete_by_user_id(&self, user_id_arg: UserId) -> RepoResult<Merchant> {
        debug!("Delete user {} merchant in db.", user_id_arg);
        let filtered = merchants.filter(user_id.eq(Some(user_id_arg)));

        let query = diesel::delete(filtered);
        query
            .get_result(self.db_conn)
            .map_err(From::from)
            .and_then(|merch| {
                acl::check(&*self.acl, Resource::Merchant, Action::Write, self, Some(&merch))?;
                Ok(merch)
            })
            .map_err(|e: FailureError| e.context(format!("Delete user {} merchant error occured", user_id_arg)).into())
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, Merchant>
    for MerchantRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id_arg: UserId, scope: &Scope, obj: Option<&Merchant>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(obj) = obj {
                    if let Some(obj_user_id) = obj.user_id {
                        user_id_arg == obj_user_id
                    } else if let Some(obj_store_id) = obj.store_id {
                        let res = Roles::roles
                            .filter(Roles::user_id.eq(user_id_arg))
                            .get_results::<UserRole>(self.db_conn)
                            .map_err(From::from)
                            .map(|user_roles_arg| {
                                user_roles_arg.iter().any(|user_role_arg| {
                                    if let Some(data) = user_role_arg.data.clone() {
                                        data == json!(obj_store_id)
                                    } else {
                                        false
                                    }
                                })
                            })
                            .unwrap_or_else(|_: FailureError| false);
                        res
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        }
    }
}
