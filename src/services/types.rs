use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;
use futures::Future;
use r2d2::{ManageConnection, PooledConnection};

use controller::context::{DynamicContext, StaticContext};
use errors::Error;
use repos::repo_factory::*;

/// Service layer Future
pub type ServiceFuture<T> = Box<Future<Item = T, Error = FailureError>>;

/// Service
pub struct Service<T, M, F>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
{
    pub static_context: StaticContext<T, M, F>,
    pub dynamic_context: DynamicContext,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > Service<T, M, F>
{
    /// Create a new service
    pub fn new(static_context: StaticContext<T, M, F>, dynamic_context: DynamicContext) -> Self {
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

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > Clone for Service<T, M, F>
{
    fn clone(&self) -> Self {
        Self {
            static_context: self.static_context.clone(),
            dynamic_context: self.dynamic_context.clone(),
        }
    }
}
