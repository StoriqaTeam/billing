//! Merchants Services, presents CRUD operations with order_info

use std::time::SystemTime;

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures::future;
use r2d2::ManageConnection;
use uuid::Uuid;

use stq_http::client::HttpClient;
use stq_types::{MerchantId, MerchantType, StoreId, UserId};

use super::types::ServiceFuture;
use client::payments::PaymentsClient;
use models::*;
use repos::repo_factory::ReposFactory;
use services::accounts::AccountService;
use services::Service;

// TODO: remove completely if nothing depends on it
pub trait MerchantService {
    /// Creates user merchant
    fn create_user(&self, user: CreateUserMerchantPayload) -> ServiceFuture<Merchant>;
    /// Delete user merchant
    fn delete_user(&self, user_id: UserId) -> ServiceFuture<MerchantId>;
    /// Creates store merchant
    fn create_store(&self, store: CreateStoreMerchantPayload) -> ServiceFuture<Merchant>;
    /// Delete store merchant
    fn delete_store(&self, store_id: StoreId) -> ServiceFuture<MerchantId>;
    /// Get user merchant balance by user id
    fn get_user_balance(&self, id: UserId) -> ServiceFuture<Vec<MerchantBalance>>;
    /// Get store merchant balance by store id
    fn get_store_balance(&self, id: StoreId) -> ServiceFuture<Vec<MerchantBalance>>;
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > MerchantService for Service<T, M, F, C, PC, AS>
{
    /// Creates user merchant
    fn create_user(&self, _user: CreateUserMerchantPayload) -> ServiceFuture<Merchant> {
        error!("Merchant service `create_user` called");
        Box::new(future::ok(Merchant {
            merchant_id: MerchantId(Uuid::default()),
            user_id: Some(UserId(0)),
            store_id: None,
            created_at: SystemTime::UNIX_EPOCH,
            updated_at: SystemTime::UNIX_EPOCH,
            merchant_type: MerchantType::User,
        }))
    }

    /// Delete user merchant
    fn delete_user(&self, _user_id_arg: UserId) -> ServiceFuture<MerchantId> {
        error!("Merchant service `delete_user` called");
        Box::new(future::ok(MerchantId(Uuid::default())))
    }

    /// Creates store merchant
    fn create_store(&self, _store: CreateStoreMerchantPayload) -> ServiceFuture<Merchant> {
        error!("Merchant service `create_store` called");
        Box::new(future::ok(Merchant {
            merchant_id: MerchantId(Uuid::default()),
            user_id: None,
            store_id: Some(StoreId(0)),
            created_at: SystemTime::UNIX_EPOCH,
            updated_at: SystemTime::UNIX_EPOCH,
            merchant_type: MerchantType::Store,
        }))
    }

    /// Delete store merchant
    fn delete_store(&self, _store_id_arg: StoreId) -> ServiceFuture<MerchantId> {
        error!("Merchant service `delete_store` called");
        Box::new(future::ok(MerchantId(Uuid::default())))
    }

    /// Get user merchant balance by user id
    fn get_user_balance(&self, _id: UserId) -> ServiceFuture<Vec<MerchantBalance>> {
        error!("Merchant service `get_user_balance` called");
        Box::new(future::ok(vec![]))
    }

    /// Get store merchant balance by store id
    fn get_store_balance(&self, _id: StoreId) -> ServiceFuture<Vec<MerchantBalance>> {
        error!("Merchant service `get_store_balance` called");
        Box::new(future::ok(vec![]))
    }
}
