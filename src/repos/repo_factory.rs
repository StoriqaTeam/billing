use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Error as FailureError;

use repos::legacy_acl::{Acl, SystemACL, UnauthorizedACL};

use models::*;
use repos::*;

pub trait ReposFactory<C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static>:
    Clone + Send + Sync + 'static
{
    fn create_order_info_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<OrderInfoRepo + 'a>;
    fn create_order_info_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<OrderInfoRepo + 'a>;
    fn create_merchant_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<MerchantRepo + 'a>;
    fn create_merchant_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<MerchantRepo + 'a>;
    fn create_user_roles_repo<'a>(&self, _db_conn: &'a C) -> Box<UserRolesRepo + 'a>;
}

#[derive(Clone)]
pub struct ReposFactoryImpl {
    roles_cache: RolesCacheImpl,
}

impl ReposFactoryImpl {
    pub fn new(roles_cache: RolesCacheImpl) -> Self {
        Self { roles_cache }
    }

    pub fn get_roles<'a, C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static>(
        &self,
        id: UserId,
        db_conn: &'a C,
    ) -> Vec<Role> {
        self.create_user_roles_repo(db_conn).list_for_user(id).ok().unwrap_or_default()
    }

    fn get_acl<'a, T, C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static>(
        &self,
        db_conn: &'a C,
        user_id: Option<UserId>,
    ) -> Box<Acl<Resource, Action, Scope, FailureError, T>> {
        user_id.map_or(
            Box::new(UnauthorizedACL::default()) as Box<Acl<Resource, Action, Scope, FailureError, T>>,
            |id| {
                let roles = self.get_roles(id, db_conn);
                (Box::new(ApplicationAcl::new(roles, id)) as Box<Acl<Resource, Action, Scope, FailureError, T>>)
            },
        )
    }
}

impl<C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> ReposFactory<C> for ReposFactoryImpl {
    fn create_order_info_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<OrderInfoRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(OrderInfoRepoImpl::new(db_conn, acl)) as Box<OrderInfoRepo>
    }

    fn create_order_info_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<OrderInfoRepo + 'a> {
        Box::new(OrderInfoRepoImpl::new(
            db_conn,
            Box::new(SystemACL::default()) as Box<Acl<Resource, Action, Scope, FailureError, OrderInfo>>,
        )) as Box<OrderInfoRepo>
    }

    fn create_merchant_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<MerchantRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(MerchantRepoImpl::new(db_conn, acl)) as Box<MerchantRepo>
    }

    fn create_merchant_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<MerchantRepo + 'a> {
        Box::new(MerchantRepoImpl::new(
            db_conn,
            Box::new(SystemACL::default()) as Box<Acl<Resource, Action, Scope, FailureError, Merchant>>,
        )) as Box<MerchantRepo>
    }

    fn create_user_roles_repo<'a>(&self, db_conn: &'a C) -> Box<UserRolesRepo + 'a> {
        Box::new(UserRolesRepoImpl::new(
            db_conn,
            Box::new(SystemACL::default()) as Box<Acl<Resource, Action, Scope, FailureError, UserRole>>,
            self.roles_cache.clone(),
        )) as Box<UserRolesRepo>
    }
}

#[cfg(test)]
pub mod tests {
    extern crate diesel;
    extern crate futures;
    extern crate futures_cpupool;
    extern crate hyper;
    extern crate r2d2;
    extern crate serde_json;
    extern crate stq_http;
    extern crate tokio_core;
    extern crate uuid;

    use std::error::Error;
    use std::fmt;
    use std::sync::Arc;

    use futures_cpupool::CpuPool;
    use tokio_core::reactor::Handle;

    use r2d2::ManageConnection;

    use diesel::connection::AnsiTransactionManager;
    use diesel::connection::SimpleConnection;
    use diesel::deserialize::QueryableByName;
    use diesel::pg::Pg;
    use diesel::query_builder::AsQuery;
    use diesel::query_builder::QueryFragment;
    use diesel::query_builder::QueryId;
    use diesel::sql_types::HasSqlType;
    use diesel::Connection;
    use diesel::ConnectionResult;
    use diesel::QueryResult;
    use diesel::Queryable;

    use uuid::Uuid;

    use stq_http::client::Config as HttpConfig;

    use config::Config;
    use models::*;
    use repos::merchant::MerchantRepo;
    use repos::order_info::*;
    use repos::repo_factory::ReposFactory;
    use repos::types::RepoResult;
    use repos::user_roles::UserRolesRepo;
    use services::order_info::OrderInfoServiceImpl;

    #[derive(Default, Copy, Clone)]
    pub struct ReposFactoryMock;

    impl<C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> ReposFactory<C> for ReposFactoryMock {
        fn create_order_info_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<OrderInfoRepo + 'a> {
            Box::new(OrderInfoRepoMock::default()) as Box<OrderInfoRepo>
        }

        fn create_order_info_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<OrderInfoRepo + 'a> {
            Box::new(OrderInfoRepoMock::default()) as Box<OrderInfoRepo>
        }

        fn create_merchant_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<MerchantRepo + 'a> {
            Box::new(MerchantRepoMock::default()) as Box<MerchantRepo>
        }

        fn create_merchant_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<MerchantRepo + 'a> {
            Box::new(MerchantRepoMock::default()) as Box<MerchantRepo>
        }

        fn create_user_roles_repo<'a>(&self, _db_conn: &'a C) -> Box<UserRolesRepo + 'a> {
            Box::new(UserRolesRepoMock::default()) as Box<UserRolesRepo>
        }
    }

    #[derive(Clone, Default)]
    pub struct OrderInfoRepoMock;

    impl OrderInfoRepo for OrderInfoRepoMock {
        /// Find specific order_info by ID
        fn find(&self, _order_info_id: OrderInfoId) -> RepoResult<Option<OrderInfo>> {
            Ok(Some(create_order_info()))
        }

        /// Creates new order_info
        fn create(&self, _payload: NewOrderInfo) -> RepoResult<OrderInfo> {
            Ok(create_order_info())
        }

        /// Updates specific order_info
        fn set_paid(&self, callback_id_arg: CallbackId) -> RepoResult<Vec<OrderInfo>> {
            let mut order_info = create_order_info();
            order_info.callback_id = callback_id_arg;
            Ok(vec![order_info])
        }
    }

    #[derive(Clone, Default)]
    pub struct MerchantRepoMock;

    impl MerchantRepo for MerchantRepoMock {
        /// Returns merchant by subject identifier
        fn get_by_subject_id(&self, id: SubjectIdentifier) -> RepoResult<Merchant> {
            Ok(match id {
                SubjectIdentifier::Store(store_ident) => Merchant {
                    merchant_id: MerchantId(Uuid::new_v4()),
                    user_id: None,
                    store_id: Some(store_ident),
                    merchant_type: MerchantType::Store,
                },
                SubjectIdentifier::User(user_ident) => Merchant {
                    merchant_id: MerchantId(Uuid::new_v4()),
                    user_id: Some(user_ident),
                    store_id: None,
                    merchant_type: MerchantType::User,
                },
            })
        }

        /// Returns merchant by merchant identifier
        fn get_by_merchant_id(&self, merchant_id: MerchantId) -> RepoResult<Merchant> {
            Ok(Merchant {
                merchant_id,
                user_id: Some(UserId(1)),
                store_id: None,
                merchant_type: MerchantType::User,
            })
        }

        /// Create a new store merchant
        fn create_store_merchant(&self, payload: NewStoreMerchant) -> RepoResult<Merchant> {
            Ok(Merchant {
                merchant_id: payload.merchant_id().clone(),
                user_id: payload.user_id().clone(),
                store_id: payload.store_id().clone(),
                merchant_type: payload.merchant_type().clone(),
            })
        }

        /// Create a new user merchant
        fn create_user_merchant(&self, payload: NewUserMerchant) -> RepoResult<Merchant> {
            Ok(Merchant {
                merchant_id: payload.merchant_id().clone(),
                user_id: payload.user_id().clone(),
                store_id: payload.store_id().clone(),
                merchant_type: payload.merchant_type().clone(),
            })
        }
    }

    #[derive(Clone, Default)]
    pub struct UserRolesRepoMock;

    impl UserRolesRepo for UserRolesRepoMock {
        fn list_for_user(&self, user_id_value: UserId) -> RepoResult<Vec<Role>> {
            Ok(match user_id_value.0 {
                1 => vec![Role::Superuser],
                _ => vec![Role::User],
            })
        }

        fn create(&self, payload: NewUserRole) -> RepoResult<UserRole> {
            Ok(UserRole {
                id: RoleId::new(),
                user_id: payload.user_id,
                role: payload.role,
                data: None,
            })
        }

        fn delete_by_user_id(&self, user_id_arg: UserId) -> RepoResult<Vec<UserRole>> {
            Ok(vec![UserRole {
                id: RoleId::new(),
                user_id: user_id_arg,
                role: Role::User,
                data: None,
            }])
        }

        fn delete_by_id(&self, id: RoleId) -> RepoResult<UserRole> {
            Ok(UserRole {
                id: id,
                user_id: UserId(1),
                role: Role::User,
                data: None,
            })
        }
    }

    pub fn create_order_info_service(
        user_id: Option<UserId>,
        handle: Arc<Handle>,
    ) -> OrderInfoServiceImpl<MockConnection, MockConnectionManager, ReposFactoryMock> {
        let manager = MockConnectionManager::default();
        let db_pool = r2d2::Pool::builder().build(manager).expect("Failed to create connection pool");
        let cpu_pool = CpuPool::new(1);

        let config = Config::new().unwrap();
        let http_config = HttpConfig {
            http_client_retries: config.client.http_client_retries,
            http_client_buffer_size: config.client.http_client_buffer_size,
        };
        let client = stq_http::client::Client::new(&http_config, &handle);
        let client_handle = client.handle();

        OrderInfoServiceImpl::new(db_pool, cpu_pool, client_handle, user_id, MOCK_REPO_FACTORY, "".to_string(), "".to_string())
    }

    pub fn create_order_info() -> OrderInfo {
        OrderInfo {
            id: OrderInfoId::new(),
            order_id: OrderId::new(),
            callback_id: CallbackId::new(),
            status: OrderStatus::PaimentAwaited,
        }
    }

    #[derive(Default)]
    pub struct MockConnection {
        tr: AnsiTransactionManager,
    }

    impl Connection for MockConnection {
        type Backend = Pg;
        type TransactionManager = AnsiTransactionManager;

        fn establish(_database_url: &str) -> ConnectionResult<MockConnection> {
            Ok(MockConnection::default())
        }

        fn execute(&self, _query: &str) -> QueryResult<usize> {
            unimplemented!()
        }

        fn query_by_index<T, U>(&self, _source: T) -> QueryResult<Vec<U>>
        where
            T: AsQuery,
            T::Query: QueryFragment<Pg> + QueryId,
            Pg: HasSqlType<T::SqlType>,
            U: Queryable<T::SqlType, Pg>,
        {
            unimplemented!()
        }

        fn query_by_name<T, U>(&self, _source: &T) -> QueryResult<Vec<U>>
        where
            T: QueryFragment<Pg> + QueryId,
            U: QueryableByName<Pg>,
        {
            unimplemented!()
        }

        fn execute_returning_count<T>(&self, _source: &T) -> QueryResult<usize>
        where
            T: QueryFragment<Pg> + QueryId,
        {
            unimplemented!()
        }

        fn transaction_manager(&self) -> &Self::TransactionManager {
            &self.tr
        }
    }

    impl SimpleConnection for MockConnection {
        fn batch_execute(&self, _query: &str) -> QueryResult<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    pub struct MockConnectionManager;

    impl ManageConnection for MockConnectionManager {
        type Connection = MockConnection;
        type Error = MockError;

        fn connect(&self) -> Result<MockConnection, MockError> {
            Ok(MockConnection::default())
        }

        fn is_valid(&self, _conn: &mut MockConnection) -> Result<(), MockError> {
            Ok(())
        }

        fn has_broken(&self, _conn: &mut MockConnection) -> bool {
            false
        }
    }

    #[derive(Debug)]
    pub struct MockError {}

    impl fmt::Display for MockError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "SuperError is here!")
        }
    }

    impl Error for MockError {
        fn description(&self) -> &str {
            "I'm the superhero of errors"
        }

        fn cause(&self) -> Option<&Error> {
            None
        }
    }

    pub const MOCK_REPO_FACTORY: ReposFactoryMock = ReposFactoryMock {};

}
