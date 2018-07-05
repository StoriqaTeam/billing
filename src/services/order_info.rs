//! OrderInfos Services, presents CRUD operations with order_info

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;
use futures::Future;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};

use stq_http::client::ClientHandle;

use super::types::ServiceFuture;
use errors::Error;
use models::{NewOrderInfo, OrderInfo, OrderInfoId, OrderStatus, UpdateOrderInfo};
use repos::repo_factory::ReposFactory;

pub trait OrderInfoService {
    /// Updates specific order_info
    fn set_paid(&self, order_info_id: OrderInfoId) -> ServiceFuture<OrderInfo>;
    /// Creates new order_info
    fn create(&self, payload: NewOrderInfo) -> ServiceFuture<OrderInfo>;
}

/// OrderInfos services, responsible for OrderInfo-related CRUD operations
pub struct OrderInfoServiceImpl<
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
> {
    pub db_pool: Pool<M>,
    pub cpu_pool: CpuPool,
    pub http_client: ClientHandle,
    user_id: Option<i32>,
    pub repo_factory: F,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > OrderInfoServiceImpl<T, M, F>
{
    pub fn new(db_pool: Pool<M>, cpu_pool: CpuPool, http_client: ClientHandle, user_id: Option<i32>, repo_factory: F) -> Self {
        Self {
            db_pool,
            cpu_pool,
            http_client,
            user_id,
            repo_factory,
        }
    }
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > OrderInfoService for OrderInfoServiceImpl<T, M, F>
{
    /// Creates new order_info
    fn create(&self, payload: NewOrderInfo) -> ServiceFuture<OrderInfo> {
        let db_clone = self.db_pool.clone();
        let current_uid = self.user_id;
        let repo_factory = self.repo_factory.clone();

        debug!("Creating new order_info with payload: {:?}", &payload);

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_clone
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let order_info_repo = repo_factory.create_order_info_repo(&conn, current_uid);

                            conn.transaction::<OrderInfo, FailureError, _>(move || order_info_repo.create(payload))
                        })
                })
                .map_err(|e: FailureError| e.context("Service order_info, create endpoint error occured.").into()),
        )
    }

    /// Updates specific order_info
    fn set_paid(&self, order_info_id: OrderInfoId) -> ServiceFuture<OrderInfo> {
        let db_clone = self.db_pool.clone();
        let current_user = self.user_id;
        let repo_factory = self.repo_factory.clone();

        debug!("Seting order with info id {:?} paid", &order_info_id);

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_clone
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let order_info_repo = repo_factory.create_order_info_repo(&conn, current_user);
                            let payload = UpdateOrderInfo {
                                status: OrderStatus::PaimentReceived,
                            };
                            order_info_repo.update(order_info_id, payload)
                        })
                })
                .map_err(|e: FailureError| e.context("Service order_info, update endpoint error occured.").into()),
        )
    }
}

#[cfg(test)]
pub mod tests {

    use std::sync::Arc;
    use tokio_core::reactor::Core;

    use repos::repo_factory::tests::*;

    use models::*;
    use services::order_info::OrderInfoService;

    #[test]
    fn test_create_order_info() {
        let mut core = Core::new().unwrap();
        let handle = Arc::new(core.handle());
        let service = create_order_info_service(Some(1), handle);
        let new_order_info = create_new_order_info();
        let work = service.create(new_order_info);
        let result = core.run(work).unwrap();
        assert_eq!(result.order_id, OrderId::new());
    }

    #[test]
    fn test_update() {
        let mut core = Core::new().unwrap();
        let handle = Arc::new(core.handle());
        let service = create_order_info_service(Some(1), handle);
        let info_id = OrderInfoId::new();
        let work = service.set_paid(info_id);
        let result = core.run(work);
        assert_eq!(result.unwrap().id, info_id);
    }

}
