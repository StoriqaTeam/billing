use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;
use futures::Future;
use r2d2::{ManageConnection, PooledConnection};
use stq_http::client::HttpClient;

use client::payments::PaymentsClient;
use controller::context::{DynamicContext, StaticContext};
use errors::Error;
use repos::repo_factory::*;
use services::accounts::AccountService;

use super::{Error as ServiceError, ErrorKind};

/// Service layer Future
pub type ServiceFuture<T> = Box<Future<Item = T, Error = FailureError>>;
pub type ServiceFutureV2<T> = Box<Future<Item = T, Error = ServiceError>>;

/// Service
#[derive(Clone)]
pub struct Service<T, M, F, C, PC, AS>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
    C: HttpClient + Clone,
    PC: PaymentsClient + Clone,
    AS: AccountService + Clone + 'static,
{
    pub static_context: StaticContext<T, M, F>,
    pub dynamic_context: DynamicContext<C, PC, AS>,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone + 'static,
    > Service<T, M, F, C, PC, AS>
{
    /// Create a new service
    pub fn new(static_context: StaticContext<T, M, F>, dynamic_context: DynamicContext<C, PC, AS>) -> Self {
        Self {
            static_context,
            dynamic_context,
        }
    }

    pub fn spawn_on_pool<R, Func>(&self, f: Func) -> ServiceFuture<R>
    where
        Func: FnOnce(PooledConnection<M>) -> Result<R, FailureError> + Send + 'static,
        R: Send + 'static,
    {
        let db_pool = self.static_context.db_pool.clone();
        let cpu_pool = self.static_context.cpu_pool.clone();
        Box::new(cpu_pool.spawn_fn(move || db_pool.get().map_err(|e| e.context(Error::Connection).into()).and_then(f)))
    }
}

pub fn spawn_on_pool<T, M, Func, R>(db_pool: r2d2::Pool<M>, cpu_pool: futures_cpupool::CpuPool, f: Func) -> ServiceFutureV2<R>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    Func: FnOnce(PooledConnection<M>) -> Result<R, ServiceError> + Send + 'static,
    R: Send + 'static,
{
    Box::new(cpu_pool.spawn_fn(move || db_pool.get().map_err(ectx!(ErrorKind::Internal)).and_then(f)))
}
