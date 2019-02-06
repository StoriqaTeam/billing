//! Repo for user_roles table. UserRole is an entity that connects
//! users and roles. I.e. this table is for user has-many roles
//! relationship

use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;
use std::sync::Arc;
use stq_cache::cache::Cache;
use stq_types::{BillingRole, RoleId, StoreId, UserId};

use super::acl;
use models::authorization::*;
use models::{NewUserRole, RemoveUserRole, UserRole};
use repos::error::*;
use repos::legacy_acl::*;
use repos::types::{RepoResult, RepoResultV2};
use repos::RolesCacheImpl;
use schema::roles::dsl::*;

/// UserRoles repository for handling UserRoles
pub trait UserRolesRepo {
    /// Returns user role by storeId
    fn get_by_store_id(&self, store_id: StoreId) -> RepoResultV2<Option<UserRole>>;

    /// Returns list of user_roles for a specific user
    fn list_for_user(&self, user_id: UserId) -> RepoResult<Vec<BillingRole>>;

    /// Create a new user role
    fn create(&self, payload: NewUserRole) -> RepoResult<UserRole>;

    /// Delete existing user role
    fn delete(&self, payload: RemoveUserRole) -> RepoResult<UserRole>;

    /// Delete roles of a user
    fn delete_by_user_id(&self, user_id: UserId) -> RepoResult<Vec<UserRole>>;

    /// Delete user roles by id
    fn delete_by_id(&self, id: RoleId) -> RepoResult<UserRole>;
}

/// Implementation of UserRoles trait
pub struct UserRolesRepoImpl<'a, C, T>
where
    C: Cache<Vec<BillingRole>>,
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
{
    pub db_conn: &'a T,
    pub acl: Box<Acl<Resource, Action, Scope, FailureError, UserRole>>,
    pub cached_roles: Arc<RolesCacheImpl<C>>,
}

impl<'a, C, T> UserRolesRepoImpl<'a, C, T>
where
    C: Cache<Vec<BillingRole>>,
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
{
    pub fn new(
        db_conn: &'a T,
        acl: Box<Acl<Resource, Action, Scope, FailureError, UserRole>>,
        cached_roles: Arc<RolesCacheImpl<C>>,
    ) -> Self {
        Self {
            db_conn,
            acl,
            cached_roles,
        }
    }
}

impl<'a, C, T> UserRolesRepo for UserRolesRepoImpl<'a, C, T>
where
    C: Cache<Vec<BillingRole>>,
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
{
    /// Returns user role by storeId
    fn get_by_store_id(&self, store_id: StoreId) -> RepoResultV2<Option<UserRole>> {
        debug!("Getting user role by store id {}", store_id);
        let query = roles.filter(data.eq(serde_json::json!(store_id.0)));
        let role = query.get_result::<UserRole>(self.db_conn).optional().map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(try err e, ErrorSource::Diesel, error_kind)
        })?;

        Ok(role)
    }

    /// Returns list of user_roles for a specific user
    fn list_for_user(&self, user_id_value: UserId) -> RepoResult<Vec<BillingRole>> {
        debug!("list user roles for id {}.", user_id_value);
        if let Some(user_roles) = self.cached_roles.get(user_id_value) {
            Ok(user_roles)
        } else {
            let query = roles.filter(user_id.eq(user_id_value));
            query
                .get_results::<UserRole>(self.db_conn)
                .map(|user_roles_arg| {
                    let user_roles = user_roles_arg
                        .into_iter()
                        .map(|user_role| user_role.name)
                        .collect::<Vec<BillingRole>>();

                    if !user_roles.is_empty() {
                        self.cached_roles.set(user_id_value, user_roles.clone());
                    }

                    user_roles
                })
                .map_err(|e| {
                    e.context(format!("List user roles for user {} error occurred.", user_id_value))
                        .into()
                })
        }
    }

    /// Create a new user role
    fn create(&self, payload: NewUserRole) -> RepoResult<UserRole> {
        debug!("create new user role {:?}.", payload);
        acl::check(&*self.acl, Resource::UserRoles, Action::Write, self, None)?;

        self.cached_roles.remove(payload.user_id);
        let query = diesel::insert_into(roles).values(&payload);
        query
            .get_result(self.db_conn)
            .map_err(|e| e.context(format!("Create a new user role {:?} error occurred", payload)).into())
    }

    /// Delete existing user role
    fn delete(&self, payload: RemoveUserRole) -> RepoResult<UserRole> {
        debug!("delete user role {:?}.", payload);
        self.cached_roles.remove(payload.user_id);
        let filtered = roles.filter(user_id.eq(payload.user_id).and(name.eq(payload.name)));
        let query = diesel::delete(filtered);
        let deleted_role = query
            .get_result(self.db_conn)
            .map_err(|e| e.context(format!("Delete user {} roles error occurred", payload.user_id)))?;

        acl::check(&*self.acl, Resource::UserRoles, Action::Write, self, Some(&deleted_role))?;

        Ok(deleted_role)
    }

    /// Delete roles of a user
    fn delete_by_user_id(&self, user_id_arg: UserId) -> RepoResult<Vec<UserRole>> {
        debug!("delete user {} role.", user_id_arg);
        self.cached_roles.remove(user_id_arg);
        let filtered = roles.filter(user_id.eq(user_id_arg));
        let query = diesel::delete(filtered);
        query
            .get_results(self.db_conn)
            .map_err(|e| e.context(format!("Delete user {} roles error occurred", user_id_arg)).into())
    }

    /// Delete user roles by id
    fn delete_by_id(&self, id_arg: RoleId) -> RepoResult<UserRole> {
        debug!("delete user role by id {}.", id_arg);
        let filtered = roles.filter(id.eq(id_arg));
        let query = diesel::delete(filtered);
        query
            .get_result(self.db_conn)
            .map_err(|e| e.context(format!("Delete role {} error occurred", id_arg)).into())
            .map(|user_role: UserRole| {
                self.cached_roles.remove(user_role.user_id);
                user_role
            })
    }
}

impl<'a, C, T> CheckScope<Scope, UserRole> for UserRolesRepoImpl<'a, C, T>
where
    C: Cache<Vec<BillingRole>>,
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
{
    fn is_in_scope(&self, user_id_arg: UserId, scope: &Scope, obj: Option<&UserRole>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(user_role) = obj {
                    user_role.user_id == user_id_arg
                } else {
                    false
                }
            }
        }
    }
}

pub fn user_is_store_manager<T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static>(
    conn: &T,
    user_id_arg: stq_types::UserId,
    store_id_arg: ::models::order_v2::StoreId,
) -> bool {
    roles
        .filter(user_id.eq(user_id_arg))
        .get_results::<UserRole>(conn)
        .map_err(From::from)
        .map(|user_roles_arg| {
            user_roles_arg.iter().any(|user_role_arg| {
                user_role_arg
                    .data
                    .clone()
                    .map(|data_arg| data_arg == store_id_arg.inner())
                    .unwrap_or_default()
            })
        })
        .unwrap_or_else(|_: FailureError| false)
}
