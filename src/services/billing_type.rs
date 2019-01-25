//! BillingInfo Service, presents operations with billing info resource
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};

use failure::Fail;

use stq_http::client::HttpClient;
use stq_types::{BillingType, StoreId};

use client::payments::PaymentsClient;
use services::accounts::AccountService;

use models::*;
use repos::ReposFactory;

use super::types::ServiceFutureV2;
use controller::context::DynamicContext;

use services::types::spawn_on_pool;

pub trait BillingTypeService {
    fn get_billing_type_by_store(&self, store_id: StoreId) -> ServiceFutureV2<Option<BillingType>>;
}

pub struct BillingTypeServiceImpl<
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
    > BillingTypeService for BillingTypeServiceImpl<T, M, F, C, PC, AS>
{
    fn get_billing_type_by_store(&self, store_id: StoreId) -> ServiceFutureV2<Option<BillingType>> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let store_billing_type_repo = repo_factory.create_store_billing_type_repo(&conn, user_id);

            store_billing_type_repo
                .get(StoreBillingTypeSearch::by_store_id(store_id))
                .map(|store_billing_type| store_billing_type.map(|s| s.billing_type))
                .map_err(ectx!(convert))
        })
    }
}
