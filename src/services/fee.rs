//! FeesService Services, presents CRUD operations with fee table

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};

use failure::Fail;

use stq_http::client::HttpClient;

use client::payments::PaymentsClient;
use services::accounts::AccountService;

use models::{order_v2::OrderId, Fee};
use repos::{ReposFactory, SearchFee};

use super::types::ServiceFutureV2;
use controller::context::DynamicContext;

use services::types::spawn_on_pool;

pub trait FeesService {
    /// Getting fee by order id
    fn get_by_order_id(&self, order_id: OrderId) -> ServiceFutureV2<Option<Fee>>;
}

pub struct FeesServiceImpl<
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
    C: HttpClient + Clone,
    PC: PaymentsClient + Clone,
    AS: AccountService + Clone,
> {
    pub db_pool: Pool<M>,
    pub cpu_pool: CpuPool,
    pub repo_factory: F,
    pub dynamic_context: DynamicContext<C, PC, AS>,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > FeesService for FeesServiceImpl<T, M, F, C, PC, AS>
{
    fn get_by_order_id(&self, order_id: OrderId) -> ServiceFutureV2<Option<Fee>> {
        debug!("Requesting fee record by order id: {}", order_id);

        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let customers_repo = repo_factory.create_fees_repo(&conn, user_id);

            customers_repo.get(SearchFee::OrderId(order_id)).map_err(ectx!(convert => order_id))
        })
    }
}
