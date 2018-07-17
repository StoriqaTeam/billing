//! UserRoles Services, presents CRUD operations with user_roles

use futures_cpupool::CpuPool;

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;
use futures::Future;
use r2d2::{ManageConnection, Pool};

use stq_types::{RoleId, StoresRole, UserId};

use super::types::ServiceFuture;
use errors::Error;
use models::{NewUserRole, UserRole};
use repos::roles_cache::RolesCacheImpl;
use repos::ReposFactory;

pub trait UserRolesService {
    /// Returns role by user ID
    fn get_roles(&self, user_id: UserId) -> ServiceFuture<Vec<StoresRole>>;
    /// Creates new user_role
    fn create(&self, payload: NewUserRole) -> ServiceFuture<UserRole>;
    /// Deletes roles for user
    fn delete_by_user_id(&self, user_id_arg: UserId) -> ServiceFuture<Vec<UserRole>>;
    /// Deletes role for user by id
    fn delete_by_id(&self, id_arg: RoleId) -> ServiceFuture<UserRole>;
}

/// UserRoles services, responsible for UserRole-related CRUD operations
pub struct UserRolesServiceImpl<
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
> {
    pub db_pool: Pool<M>,
    pub cpu_pool: CpuPool,
    pub cached_roles: RolesCacheImpl,
    pub repo_factory: F,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > UserRolesServiceImpl<T, M, F>
{
    pub fn new(db_pool: Pool<M>, cpu_pool: CpuPool, cached_roles: RolesCacheImpl, repo_factory: F) -> Self {
        Self {
            db_pool,
            cpu_pool,
            cached_roles,
            repo_factory,
        }
    }
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > UserRolesService for UserRolesServiceImpl<T, M, F>
{
    /// Returns role by user ID
    fn get_roles(&self, user_id: UserId) -> ServiceFuture<Vec<StoresRole>> {
        let db_pool = self.db_pool.clone();
        let repo_factory = self.repo_factory.clone();

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_pool
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let user_roles_repo = repo_factory.create_user_roles_repo(&*conn);
                            user_roles_repo.list_for_user(user_id)
                        })
                })
                .map_err(|e: FailureError| e.context("Service user_roles, get_roles endpoint error occured.").into()),
        )
    }

    /// Deletes roles for user
    fn delete_by_user_id(&self, user_id_arg: UserId) -> ServiceFuture<Vec<UserRole>> {
        let db_pool = self.db_pool.clone();
        let repo_factory = self.repo_factory.clone();

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_pool
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let user_roles_repo = repo_factory.create_user_roles_repo(&*conn);
                            user_roles_repo.delete_by_user_id(user_id_arg)
                        })
                })
                .map_err(|e: FailureError| e.context("Service user_roles, delete endpoint error occured.").into()),
        )
    }

    /// Creates new user_role
    fn create(&self, new_user_role: NewUserRole) -> ServiceFuture<UserRole> {
        let db_pool = self.db_pool.clone();
        let repo_factory = self.repo_factory.clone();

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_pool
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let user_roles_repo = repo_factory.create_user_roles_repo(&*conn);
                            user_roles_repo.create(new_user_role)
                        })
                })
                .map_err(|e: FailureError| e.context("Service user_roles, create endpoint error occured.").into()),
        )
    }

    /// Deletes role for user by id
    fn delete_by_id(&self, id_arg: RoleId) -> ServiceFuture<UserRole> {
        let db_pool = self.db_pool.clone();
        let repo_factory = self.repo_factory.clone();

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_pool
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let user_roles_repo = repo_factory.create_user_roles_repo(&*conn);
                            user_roles_repo.delete_by_id(id_arg)
                        })
                })
                .map_err(|e: FailureError| e.context("Service user_roles, delete_by_id endpoint error occured.").into()),
        )
    }
}
