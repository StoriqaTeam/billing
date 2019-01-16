//! `Context` is a top level module contains static context and dynamic context for each request
use std::sync::Arc;

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};

use stq_http::client::{ClientHandle, HttpClient};
use stq_router::RouteParser;
use stq_types::UserId;

use super::routes::*;
use client::payments::PaymentsClient;
use client::stripe::{StripeClient, StripeClientImpl};
use config::Config;
use repos::repo_factory::*;
use services::accounts::AccountService;

/// Static context for all app
pub struct StaticContext<T, M, F>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
{
    pub db_pool: Pool<M>,
    pub cpu_pool: CpuPool,
    pub config: Arc<Config>,
    pub route_parser: Arc<RouteParser<Route>>,
    pub client_handle: ClientHandle,
    pub repo_factory: F,
    pub stripe_client: Arc<dyn StripeClient>,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > StaticContext<T, M, F>
{
    /// Create a new static context
    pub fn new(db_pool: Pool<M>, cpu_pool: CpuPool, client_handle: ClientHandle, config: Arc<Config>, repo_factory: F) -> Self {
        let route_parser = Arc::new(create_route_parser());
        let stripe_client = Arc::new(StripeClientImpl::create_from_config(&config, cpu_pool.clone()));
        Self {
            route_parser,
            db_pool,
            cpu_pool,
            client_handle,
            config,
            repo_factory,
            stripe_client,
        }
    }
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > Clone for StaticContext<T, M, F>
{
    fn clone(&self) -> Self {
        Self {
            cpu_pool: self.cpu_pool.clone(),
            db_pool: self.db_pool.clone(),
            route_parser: self.route_parser.clone(),
            client_handle: self.client_handle.clone(),
            config: self.config.clone(),
            repo_factory: self.repo_factory.clone(),
            stripe_client: self.stripe_client.clone(),
        }
    }
}

/// Dynamic context for each request
#[derive(Clone)]
pub struct DynamicContext<C, PC, AS>
where
    C: HttpClient + Clone,
    PC: PaymentsClient + Clone,
    AS: AccountService + Clone + 'static,
{
    pub user_id: Option<UserId>,
    pub correlation_token: String,
    pub http_client: C,
    pub payments_client: Option<PC>,
    pub account_service: Option<AS>,
}

impl<C, PC, AS> DynamicContext<C, PC, AS>
where
    C: HttpClient + Clone,
    PC: PaymentsClient + Clone,
    AS: AccountService + Clone + 'static,
{
    /// Create a new dynamic context for each request
    pub fn new(
        user_id: Option<UserId>,
        correlation_token: String,
        http_client: C,
        payments_client: Option<PC>,
        account_service: Option<AS>,
    ) -> Self {
        Self {
            user_id,
            correlation_token,
            http_client,
            payments_client,
            account_service,
        }
    }
}
