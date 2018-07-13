//! Merchants Services, presents CRUD operations with order_info

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;
use futures::Future;
use futures_cpupool::CpuPool;
use hyper::{Delete, Get, Post};
use r2d2::{ManageConnection, Pool};
use serde_json;

use stq_http::client::ClientHandle;
use stq_types::{MerchantId, StoreId, UserId};

use super::types::ServiceFuture;
use errors::Error;
use models::*;
use repos::repo_factory::ReposFactory;

pub trait MerchantService {
    /// Creates user merchant
    fn create_user(&self, user: CreateUserMerchantPayload) -> ServiceFuture<Merchant>;
    /// Delete user merchant
    fn delete_user(&self, user_id: UserId) -> ServiceFuture<ExternalBillingMerchant>;
    /// Creates store merchant
    fn create_store(&self, store: CreateStoreMerchantPayload) -> ServiceFuture<Merchant>;
    /// Delete store merchant
    fn delete_store(&self, store_id: StoreId) -> ServiceFuture<ExternalBillingMerchant>;
    /// Get merchant balance by merchant id
    fn get_balance(&self, id: MerchantId) -> ServiceFuture<MerchantBalance>;
}

/// Merchants services, responsible for Merchant-related CRUD operations
pub struct MerchantServiceImpl<
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
> {
    pub db_pool: Pool<M>,
    pub cpu_pool: CpuPool,
    pub http_client: ClientHandle,
    user_id: Option<UserId>,
    pub repo_factory: F,
    pub external_billing_address: String,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > MerchantServiceImpl<T, M, F>
{
    pub fn new(
        db_pool: Pool<M>,
        cpu_pool: CpuPool,
        http_client: ClientHandle,
        user_id: Option<UserId>,
        repo_factory: F,
        external_billing_address: String,
    ) -> Self {
        Self {
            db_pool,
            cpu_pool,
            http_client,
            user_id,
            repo_factory,
            external_billing_address,
        }
    }
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > MerchantService for MerchantServiceImpl<T, M, F>
{
    /// Creates user merchant
    fn create_user(&self, user: CreateUserMerchantPayload) -> ServiceFuture<Merchant> {
        let db_clone = self.db_pool.clone();
        let user_id = self.user_id;
        let repo_factory = self.repo_factory.clone();
        let client = self.http_client.clone();
        let external_billing_address = self.external_billing_address.clone();

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_clone
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let merchant_repo = repo_factory.create_merchant_repo(&conn, user_id);

                            conn.transaction::<Merchant, FailureError, _>(move || {
                                debug!("Creating new user merchant: {:?}", &user);
                                let body = serde_json::to_string(&user)?;
                                let url = format!("{}/merchant", external_billing_address);
                                client
                                    .request::<ExternalBillingMerchant>(Post, url, Some(body), None)
                                    .map_err(|e| {
                                        e.context("Occured an error during user merchant creation in external billing.")
                                            .context(Error::HttpClient)
                                            .into()
                                    })
                                    .wait()
                                    .and_then(|merchant| {
                                        let payload = NewUserMerchant::new(merchant.id, user.id);
                                        merchant_repo.create_user_merchant(payload)
                                    })
                            })
                        })
                })
                .map_err(|e: FailureError| e.context("Service merchant, create user endpoint error occured.").into()),
        )
    }

    /// Delete user merchant
    fn delete_user(&self, user_id_arg: UserId) -> ServiceFuture<ExternalBillingMerchant> {
        let db_clone = self.db_pool.clone();
        let user_id = self.user_id;
        let repo_factory = self.repo_factory.clone();
        let client = self.http_client.clone();
        let external_billing_address = self.external_billing_address.clone();

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_clone
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let merchant_repo = repo_factory.create_merchant_repo(&conn, user_id);

                            conn.transaction::<ExternalBillingMerchant, FailureError, _>(move || {
                                debug!("Deleting user merchant with user id {}", &user_id_arg);
                                merchant_repo.delete_by_user_id(user_id_arg).and_then(|merchant| {
                                    let url = format!("{}/merchant/{}", external_billing_address, merchant.merchant_id);
                                    client
                                        .request::<ExternalBillingMerchant>(Delete, url, None, None)
                                        .map_err(|e| {
                                            e.context("Occured an error during user merchant deletion in external billing.")
                                                .context(Error::HttpClient)
                                                .into()
                                        })
                                        .wait()
                                })
                            })
                        })
                })
                .map_err(|e: FailureError| e.context("Service merchant, delete user endpoint error occured.").into()),
        )
    }

    /// Creates store merchant
    fn create_store(&self, store: CreateStoreMerchantPayload) -> ServiceFuture<Merchant> {
        let db_clone = self.db_pool.clone();
        let user_id = self.user_id;
        let repo_factory = self.repo_factory.clone();
        let client = self.http_client.clone();
        let external_billing_address = self.external_billing_address.clone();

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_clone
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let merchant_repo = repo_factory.create_merchant_repo(&conn, user_id);
                            conn.transaction::<Merchant, FailureError, _>(move || {
                                debug!("Creating new store merchant: {:?}", &store);
                                let body = serde_json::to_string(&store)?;
                                let url = format!("{}/merchant", external_billing_address);
                                client
                                    .request::<ExternalBillingMerchant>(Post, url, Some(body), None)
                                    .map_err(|e| {
                                        e.context("Occured an error during store merchant creation in external billing.")
                                            .context(Error::HttpClient)
                                            .into()
                                    })
                                    .wait()
                                    .and_then(|merchant| {
                                        let payload = NewStoreMerchant::new(merchant.id, store.id);
                                        merchant_repo.create_store_merchant(payload)
                                    })
                            })
                        })
                })
                .map_err(|e: FailureError| e.context("Service merchant, create user endpoint error occured.").into()),
        )
    }

    /// Delete store merchant
    fn delete_store(&self, store_id_arg: StoreId) -> ServiceFuture<ExternalBillingMerchant> {
        let db_clone = self.db_pool.clone();
        let user_id = self.user_id;
        let repo_factory = self.repo_factory.clone();
        let client = self.http_client.clone();
        let external_billing_address = self.external_billing_address.clone();

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_clone
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let merchant_repo = repo_factory.create_merchant_repo(&conn, user_id);

                            conn.transaction::<ExternalBillingMerchant, FailureError, _>(move || {
                                debug!("Deleting store merchant with store id {}", &store_id_arg);
                                merchant_repo.delete_by_store_id(store_id_arg).and_then(|merchant| {
                                    let url = format!("{}/merchant/{}", external_billing_address, merchant.merchant_id);
                                    client
                                        .request::<ExternalBillingMerchant>(Delete, url, None, None)
                                        .map_err(|e| {
                                            e.context("Occured an error during store merchant deletion in external billing.")
                                                .context(Error::HttpClient)
                                                .into()
                                        })
                                        .wait()
                                })
                            })
                        })
                })
                .map_err(|e: FailureError| e.context("Service merchant, delete store endpoint error occured.").into()),
        )
    }

    /// Get merchant balance by merchant id
    fn get_balance(&self, id: MerchantId) -> ServiceFuture<MerchantBalance> {
        let db_clone = self.db_pool.clone();
        let client = self.http_client.clone();
        let external_billing_address = self.external_billing_address.clone();

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_clone
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            conn.transaction::<MerchantBalance, FailureError, _>(move || {
                                debug!("Get merchant balance by merchant id {:?}", &id);
                                let url = format!("{}/merchant/{}", external_billing_address, id);
                                client
                                    .request::<MerchantBalance>(Get, url, None, None)
                                    .map_err(|e| {
                                        e.context("Occured an error during merchant balance receiving from external billing.")
                                            .context(Error::HttpClient)
                                            .into()
                                    })
                                    .wait()
                            })
                        })
                })
                .map_err(|e: FailureError| e.context("Service get_balance, create user endpoint error occured.").into()),
        )
    }
}

#[cfg(test)]
pub mod tests {

    use std::sync::Arc;
    use tokio_core::reactor::Core;

    use stq_types::{MerchantId, StoreId, UserId};

    use models::*;
    use repos::repo_factory::tests::*;
    use services::merchant::MerchantService;

    #[test]
    fn test_create_user_merchant() {
        let mut core = Core::new().unwrap();
        let handle = Arc::new(core.handle());
        let service = create_merchant_service(Some(UserId(1)), handle);
        let create_user = CreateUserMerchantPayload { id: UserId(1) };
        let work = service.create_user(create_user);
        let _result = core.run(work).unwrap();
    }

    #[test]
    fn test_create_store_merchant() {
        let mut core = Core::new().unwrap();
        let handle = Arc::new(core.handle());
        let service = create_merchant_service(Some(UserId(1)), handle);
        let create_store = CreateStoreMerchantPayload { id: StoreId(1) };
        let work = service.create_store(create_store);
        let _result = core.run(work).unwrap();
    }

    #[test]
    fn test_get_balance() {
        let mut core = Core::new().unwrap();
        let handle = Arc::new(core.handle());
        let service = create_merchant_service(Some(UserId(1)), handle);
        let id = MerchantId::new();
        let work = service.get_balance(id);
        let _result = core.run(work).unwrap();
    }

}
