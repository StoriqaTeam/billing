//! Merchants Services, presents CRUD operations with order_info

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;
use futures::Future;
use hyper::header::{Authorization, Bearer, ContentType};
use hyper::Headers;
use hyper::{Get, Post};
use r2d2::ManageConnection;
use serde_json;

use stq_http::client::HttpClient;
use stq_types::{BillingType, MerchantId, StoreId, UserId};

use super::types::ServiceFuture;
use client::payments::PaymentsClient;
use config::ExternalBilling;
use errors::Error;
use models::*;
use repos::repo_factory::ReposFactory;
use services::accounts::AccountService;
use services::Service;

pub trait MerchantService {
    /// Creates user merchant
    fn create_user(&self, user: CreateUserMerchantPayload) -> ServiceFuture<Merchant>;
    /// Creates user merchant v1
    fn create_user_tugush(&self, user: CreateUserMerchantPayload) -> ServiceFuture<Merchant>;
    /// Creates user merchant v2
    fn create_user_ture(&self, user: CreateUserMerchantPayload) -> ServiceFuture<Merchant>;
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
    fn create_user(&self, user: CreateUserMerchantPayload) -> ServiceFuture<Merchant> {
        if !self.payments_v2_enabled() {
            self.create_user_tugush(user)
        } else {
            self.create_user_ture(user)
        }
    }

    /// Creates user merchant in ture
    fn create_user_ture(&self, user: CreateUserMerchantPayload) -> ServiceFuture<Merchant> {
        let user_id = self.dynamic_context.user_id;
        let repo_factory = self.static_context.repo_factory.clone();

        self.spawn_on_pool(move |conn| {
            let merchant_repo = repo_factory.create_merchant_repo(&conn, user_id);
            conn.transaction::<Merchant, FailureError, _>(move || {
                debug!("Creating new user merchant: {:?}", &user);
                let merchant_id = MerchantId::new();
                let payload = NewUserMerchant::new(merchant_id, user.id);
                merchant_repo.create_user_merchant(payload)
            })
            .map_err(|e: FailureError| e.context("Service merchant, create user endpoint error occured.").into())
        })
    }

    /// Creates user merchant in tugush
    fn create_user_tugush(&self, user: CreateUserMerchantPayload) -> ServiceFuture<Merchant> {
        let user_id = self.dynamic_context.user_id;
        let repo_factory = self.static_context.repo_factory.clone();

        let client = self.dynamic_context.http_client.clone();
        let ExternalBilling {
            merchant_url,
            login_url,
            username,
            password,
            ..
        } = self.static_context.config.external_billing.clone();
        let credentials = ExternalBillingCredentials::new(username, password);

        self.spawn_on_pool(move |conn| {
            let merchant_repo = repo_factory.create_merchant_repo(&conn, user_id);
            conn.transaction::<Merchant, FailureError, _>(move || {
                debug!("Creating new user merchant: {:?}", &user);

                let body = serde_json::to_string(&credentials)?;
                let url = login_url.to_string();
                let mut headers = Headers::new();
                headers.set(ContentType::json());
                client
                    .request_json::<ExternalBillingToken>(Post, url, Some(body), Some(headers))
                    .map_err(|e| {
                        e.context("Occured an error during receiving authorization token in external billing.")
                            .context(Error::HttpClient)
                            .into()
                    })
                    .wait()
                    .and_then(|ext_token| {
                        let body = serde_json::to_string(&user)?;
                        let url = merchant_url.to_string();
                        let mut headers = Headers::new();
                        headers.set(Authorization(Bearer { token: ext_token.token }));
                        headers.set(ContentType::json());
                        client
                            .request_json::<ExternalBillingMerchant>(Post, url, Some(body), Some(headers))
                            .map_err(|e| {
                                e.context("Occured an error during user merchant creation in external billing.")
                                    .context(Error::HttpClient)
                                    .into()
                            })
                            .wait()
                    })
                    .and_then(|merchant| {
                        let payload = NewUserMerchant::new(merchant.id, user.id);
                        merchant_repo.create_user_merchant(payload)
                    })
            })
            .map_err(|e: FailureError| e.context("Service merchant, create user endpoint error occured.").into())
        })
    }

    /// Delete user merchant
    fn delete_user(&self, user_id_arg: UserId) -> ServiceFuture<MerchantId> {
        let user_id = self.dynamic_context.user_id;
        let repo_factory = self.static_context.repo_factory.clone();

        self.spawn_on_pool(move |conn| {
            let merchant_repo = repo_factory.create_merchant_repo(&conn, user_id);

            conn.transaction::<MerchantId, FailureError, _>(move || {
                debug!("Deleting user merchant with user id {}", &user_id_arg);
                merchant_repo.delete_by_user_id(user_id_arg).map(|merchant| merchant.merchant_id)
            })
            .map_err(|e: FailureError| e.context("Service merchant, delete user endpoint error occured.").into())
        })
    }

    /// Creates store merchant
    fn create_store(&self, store: CreateStoreMerchantPayload) -> ServiceFuture<Merchant> {
        let user_id = self.dynamic_context.user_id;
        let store_id = store.id;
        let country = store.country_code.clone();
        let repo_factory = self.static_context.repo_factory.clone();
        let client = self.dynamic_context.http_client.clone();
        let ExternalBilling {
            merchant_url,
            login_url,
            username,
            password,
            ..
        } = self.static_context.config.external_billing.clone();
        let credentials = ExternalBillingCredentials::new(username, password);

        self.spawn_on_pool(move |conn| {
            let merchant_repo = repo_factory.create_merchant_repo(&conn, user_id);
            let store_billing_type_repo = repo_factory.create_store_billing_type_repo(&conn, user_id);
            conn.transaction::<Merchant, FailureError, _>(move || {
                debug!("Creating new store merchant: {:?}", &store);
                let body = serde_json::to_string(&credentials)?;
                let url = login_url.to_string();
                let mut headers = Headers::new();
                headers.set(ContentType::json());
                client
                    .request_json::<ExternalBillingToken>(Post, url, Some(body), Some(headers))
                    .map_err(|e| {
                        e.context("Occured an error during receiving authorization token in external billing.")
                            .context(Error::HttpClient)
                            .into()
                    })
                    .wait()
                    .and_then(|ext_token| {
                        let body = serde_json::to_string(&store)?;
                        let url = merchant_url.to_string();
                        let mut headers = Headers::new();
                        headers.set(Authorization(Bearer { token: ext_token.token }));
                        headers.set(ContentType::json());
                        client
                            .request_json::<ExternalBillingMerchant>(Post, url, Some(body), Some(headers))
                            .map_err(|e| {
                                e.context("Occured an error during store merchant creation in external billing.")
                                    .context(Error::HttpClient)
                                    .into()
                            })
                            .wait()
                    })
                    .and_then(|merchant| {
                        let payload = NewStoreMerchant::new(merchant.id, store.id);
                        merchant_repo.create_store_merchant(payload)
                    })
                    .and_then(move |new_merchant| {
                        store_billing_type_repo
                            .create(NewStoreBillingType {
                                store_id,
                                billing_type: country.as_ref().map(country_to_billing_type).unwrap_or(BillingType::International),
                            })
                            .map_err(FailureError::from)
                            .map(|_| new_merchant)
                    })
            })
            .map_err(|e: FailureError| e.context("Service merchant, create_store endpoint error occured.").into())
        })
    }

    /// Delete store merchant
    fn delete_store(&self, store_id_arg: StoreId) -> ServiceFuture<MerchantId> {
        let user_id = self.dynamic_context.user_id;
        let repo_factory = self.static_context.repo_factory.clone();

        self.spawn_on_pool(move |conn| {
            let merchant_repo = repo_factory.create_merchant_repo(&conn, user_id);
            let store_billing_type_repo = repo_factory.create_store_billing_type_repo(&conn, user_id);
            conn.transaction::<MerchantId, FailureError, _>(move || {
                debug!("Deleting store merchant with store id {}", &store_id_arg);
                if store_billing_type_repo
                    .delete(StoreBillingTypeSearch::by_store_id(store_id_arg))
                    .is_err()
                {
                    warn!("Could not delete store billing type {}", store_id_arg);
                }
                merchant_repo.delete_by_store_id(store_id_arg).map(|merchant| merchant.merchant_id)
            })
            .map_err(|e: FailureError| e.context("Service merchant, delete store endpoint error occured.").into())
        })
    }

    /// Get user merchant balance by user id
    fn get_user_balance(&self, id: UserId) -> ServiceFuture<Vec<MerchantBalance>> {
        let user_id = self.dynamic_context.user_id;
        let repo_factory = self.static_context.repo_factory.clone();
        let client = self.dynamic_context.http_client.clone();
        let ExternalBilling {
            merchant_url,
            login_url,
            username,
            password,
            ..
        } = self.static_context.config.external_billing.clone();
        let credentials = ExternalBillingCredentials::new(username, password);

        self.spawn_on_pool(move |conn| {
            let merchant_repo = repo_factory.create_merchant_repo(&conn, user_id);
            conn.transaction::<Vec<MerchantBalance>, FailureError, _>(move || {
                debug!("Get merchant balance by user id {:?}", &id);
                merchant_repo.get_by_subject_id(SubjectIdentifier::User(id)).and_then(|merchant| {
                    let body = serde_json::to_string(&credentials)?;
                    let url = login_url.to_string();
                    let mut headers = Headers::new();
                    headers.set(ContentType::json());
                    client
                        .request_json::<ExternalBillingToken>(Post, url, Some(body), Some(headers))
                        .map_err(|e| {
                            e.context("Occured an error during receiving authorization token in external billing.")
                                .context(Error::HttpClient)
                                .into()
                        })
                        .wait()
                        .and_then(|ext_token| {
                            let url = format!("{}/{}/", merchant_url, merchant.merchant_id);
                            let mut headers = Headers::new();
                            headers.set(Authorization(Bearer { token: ext_token.token }));
                            headers.set(ContentType::json());
                            client
                                .request_json::<ExternalBillingMerchant>(Get, url, None, Some(headers))
                                .map(|ex_merchant| ex_merchant.balance.unwrap_or_default())
                                .map_err(|e| {
                                    e.context("Occured an error during user merchant get balance in external billing.")
                                        .context(Error::HttpClient)
                                        .into()
                                })
                                .wait()
                        })
                })
            })
            .map_err(|e: FailureError| e.context("Service merchant, get_user_balance endpoint error occured.").into())
        })
    }

    /// Get store merchant balance by store id
    fn get_store_balance(&self, id: StoreId) -> ServiceFuture<Vec<MerchantBalance>> {
        let user_id = self.dynamic_context.user_id;
        let repo_factory = self.static_context.repo_factory.clone();
        let client = self.dynamic_context.http_client.clone();
        let ExternalBilling {
            merchant_url,
            login_url,
            username,
            password,
            ..
        } = self.static_context.config.external_billing.clone();
        let credentials = ExternalBillingCredentials::new(username, password);

        self.spawn_on_pool(move |conn| {
            let merchant_repo = repo_factory.create_merchant_repo(&conn, user_id);
            conn.transaction::<Vec<MerchantBalance>, FailureError, _>(move || {
                debug!("Get merchant balance by store id {:?}", &id);
                merchant_repo.get_by_subject_id(SubjectIdentifier::Store(id)).and_then(|merchant| {
                    let body = serde_json::to_string(&credentials)?;
                    let url = login_url.to_string();
                    let mut headers = Headers::new();
                    headers.set(ContentType::json());
                    client
                        .request_json::<ExternalBillingToken>(Post, url, Some(body), Some(headers))
                        .map_err(|e| {
                            e.context("Occured an error during receiving authorization token in external billing.")
                                .context(Error::HttpClient)
                                .into()
                        })
                        .wait()
                        .and_then(|ext_token| {
                            let url = format!("{}/{}/", merchant_url, merchant.merchant_id);
                            let mut headers = Headers::new();
                            headers.set(Authorization(Bearer { token: ext_token.token }));
                            headers.set(ContentType::json());
                            client
                                .request_json::<ExternalBillingMerchant>(Get, url, None, Some(headers))
                                .map(|ex_merchant| ex_merchant.balance.unwrap_or_default())
                                .map_err(|e| {
                                    e.context("Occured an error during store merchant get balance in external billing.")
                                        .context(Error::HttpClient)
                                        .into()
                                })
                                .wait()
                        })
                })
            })
            .map_err(|e: FailureError| e.context("Service merchant, get_store_balance endpoint error occured.").into())
        })
    }
}

#[cfg(test)]
pub mod tests {

    use std::sync::Arc;
    use tokio_core::reactor::Core;

    use stq_types::{StoreId, UserId};

    use models::*;
    use repos::repo_factory::tests::*;
    use services::merchant::MerchantService;

    #[test]
    #[ignore]
    fn test_create_user_merchant() {
        let mut core = Core::new().unwrap();
        let handle = Arc::new(core.handle());
        let service = create_service(Some(UserId(1)), handle);
        let create_user = CreateUserMerchantPayload { id: UserId(1) };
        let work = service.create_user(create_user);
        let _result = core.run(work).unwrap();
    }

    #[test]
    #[ignore]
    fn test_create_store_merchant() {
        let mut core = Core::new().unwrap();
        let handle = Arc::new(core.handle());
        let service = create_service(Some(UserId(1)), handle);
        let create_store = CreateStoreMerchantPayload {
            id: StoreId(1),
            country_code: None,
        };
        let work = service.create_store(create_store);
        let _result = core.run(work).unwrap();
    }

    #[test]
    #[ignore]
    fn test_get_user_balance() {
        let id = UserId(1);
        let mut core = Core::new().unwrap();
        let handle = Arc::new(core.handle());
        let service = create_service(Some(id), handle);

        let create_user = CreateUserMerchantPayload { id };
        let work = service.create_user(create_user);
        let _merchant = core.run(work).unwrap();

        let work = service.get_user_balance(id);
        let _result = core.run(work).unwrap();
    }

}
