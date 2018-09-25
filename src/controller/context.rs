//! `Context` is a top level struct containg static resources
use std::sync::Arc;

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool, PooledConnection};

use stq_http::client::ClientHandle;
use stq_router::RouteParser;

use super::routes::*;
use config::Config;
use errors::Error;
use repos::repo_factory::*;
use services::types::ServiceFuture;

/// Static context for each request
pub struct Context<T, M, F>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
{
    pub db_pool: Pool<M>,
    pub cpu_pool: CpuPool,
    pub config: Config,
    pub route_parser: Arc<RouteParser<Route>>,
    pub client_handle: ClientHandle,
    pub repo_factory: F,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > Context<T, M, F>
{
    /// Create a new static context
    pub fn new(db_pool: Pool<M>, cpu_pool: CpuPool, client_handle: ClientHandle, config: Config, repo_factory: F) -> Self {
        let route_parser = Arc::new(create_route_parser());
        Self {
            route_parser,
            db_pool,
            cpu_pool,
            client_handle,
            config,
            repo_factory,
        }
    }

    pub fn spawn_on_pool<R, Func>(&self, f: Func) -> ServiceFuture<R>
    where
        Func: FnOnce(PooledConnection<M>) -> Result<R, FailureError> + Send + 'static,
        R: Send + 'static,
    {
        let db_pool = self.db_pool.clone();
        Box::new(
            self.cpu_pool
                .spawn_fn(move || db_pool.get().map_err(|e| e.context(Error::Connection).into()).and_then(f)),
        )
    }
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > Clone for Context<T, M, F>
{
    fn clone(&self) -> Self {
        Self {
            cpu_pool: self.cpu_pool.clone(),
            db_pool: self.db_pool.clone(),
            route_parser: self.route_parser.clone(),
            client_handle: self.client_handle.clone(),
            config: self.config.clone(),
            repo_factory: self.repo_factory.clone(),
        }
    }
}
