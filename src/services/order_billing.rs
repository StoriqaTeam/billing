use std::collections::{HashMap, HashSet};

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};

use failure::Fail;

use stq_http::client::HttpClient;
use stq_types::{Alpha3, BillingType, StoreId};

use super::types::ServiceFutureV2;
use client::payments::PaymentsClient;
use controller::context::DynamicContext;
use models::order_v2::OrdersSearch;
use models::order_v2::StoreId as StoreIdV2;
use models::{
    InternationalBillingInfoSearch, OrderBillingInfo, OrderBillingInfoSearchResults, OrderBillingSearchTerms,
    ProxyCompanyBillingInfoSearch, RussiaBillingInfoSearch, StoreBillingTypeSearch,
};
use repos::repo_factory::ReposFactory;
use services::accounts::AccountService;
use services::types::spawn_on_pool;

pub trait OrderBillingService {
    fn search(&self, skip: i64, count: i64, payload: OrderBillingSearchTerms) -> ServiceFutureV2<OrderBillingInfoSearchResults>;
}

pub struct OrderBillingServiceImpl<
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
    > OrderBillingService for OrderBillingServiceImpl<T, M, F, C, PC, AS>
{
    fn search(&self, skip: i64, count: i64, payload: OrderBillingSearchTerms) -> ServiceFutureV2<OrderBillingInfoSearchResults> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
            let store_billing_type_repo = repo_factory.create_store_billing_type_repo(&conn, user_id);
            let international_billing_info_repo = repo_factory.create_international_billing_info_repo(&conn, user_id);
            let russia_billing_info_repo = repo_factory.create_russia_billing_info_repo(&conn, user_id);
            let proxy_companies_billing_info_repo = repo_factory.create_proxy_companies_billing_info_repo(&conn, user_id);
            debug!("Requesting order billing {:?}", payload);
            let orders_search_result = orders_repo
                .search(
                    skip,
                    count,
                    OrdersSearch {
                        store_id: payload.store_id.map(|id| StoreIdV2::new(id.0)),
                        state: payload.payment_state,
                    },
                )
                .map_err(ectx!(try convert))?;

            let store_ids: Vec<StoreId> = orders_search_result
                .orders
                .iter()
                .map(|order| order.store_id)
                .collect::<HashSet<_>>()
                .into_iter()
                .map(|id| StoreId(id.inner()))
                .collect();

            let store_billing_types: HashMap<_, _> = store_billing_type_repo
                .search(StoreBillingTypeSearch::by_store_ids(store_ids.clone()))
                .map_err(ectx!(try convert))?
                .into_iter()
                .map(|billing_type| (billing_type.store_id, billing_type))
                .collect();

            let international_billings: HashMap<_, _> = international_billing_info_repo
                .search(InternationalBillingInfoSearch::by_store_ids(store_ids.clone()))
                .map_err(ectx!(try convert))?
                .into_iter()
                .map(|billing| (billing.store_id, billing))
                .collect();

            let russia_billings: HashMap<_, _> = russia_billing_info_repo
                .search(RussiaBillingInfoSearch::by_store_ids(store_ids.clone()))
                .map_err(ectx!(try convert))?
                .into_iter()
                .map(|billing| (billing.store_id, billing))
                .collect();

            let russia = Alpha3("RUS".to_string());
            let proxy_company_billing_info = proxy_companies_billing_info_repo
                .get(ProxyCompanyBillingInfoSearch::by_country(russia))
                .map_err(ectx!(try convert))?;

            let total_count = orders_search_result.total_count;
            let orders = orders_search_result
                .orders
                .into_iter()
                .map(|order| {
                    let store_id = StoreId(order.store_id.inner());
                    let billing_type = store_billing_types
                        .get(&store_id)
                        .map(|store_billing| store_billing.billing_type)
                        .unwrap_or(BillingType::International);
                    OrderBillingInfo {
                        russia_billing_info: russia_billings.get(&store_id).cloned(),
                        international_billing_info: international_billings.get(&store_id).cloned(),
                        billing_type,
                        proxy_company_billing_info: proxy_company_billing_info
                            .clone()
                            .filter(move |_| billing_type == BillingType::Russia),
                        order,
                    }
                })
                .collect();

            Ok(OrderBillingInfoSearchResults { total_count, orders })
        })
    }
}
