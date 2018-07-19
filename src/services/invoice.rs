//! OrderInfos Services, presents CRUD operations with order_info

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;
use futures::Future;
use futures_cpupool::CpuPool;
use hyper::header::{Authorization, Bearer, ContentType};
use hyper::Headers;
use hyper::Post;
use r2d2::{ManageConnection, Pool};
use serde_json;

use stq_http::client::ClientHandle;
use stq_types::{InvoiceId, OrderId, SagaId, UserId};

use super::types::ServiceFuture;
use config::Config;
use errors::Error;
use models::*;
use repos::repo_factory::ReposFactory;
use repos::RepoResult;

pub trait InvoiceService {
    /// Creates invoice in billing system
    fn create(&self, create_invoice: CreateInvoice) -> ServiceFuture<Invoice>;
    /// Get invoice by order id
    fn get_by_order_id(&self, order_id: OrderId) -> ServiceFuture<Option<Invoice>>;
    /// Get invoice by invoice id
    fn get_by_id(&self, id: InvoiceId) -> ServiceFuture<Option<Invoice>>;
    /// Get orders ids by invoice id
    fn get_orders_ids(&self, id: InvoiceId) -> ServiceFuture<Vec<OrderId>>;
    /// Delete invoice merchant
    fn delete(&self, id: SagaId) -> ServiceFuture<SagaId>;
    /// Creates orders in billing system, returning url for payment
    fn update(&self, invoice: ExternalBillingInvoice) -> ServiceFuture<String>;
}

/// OrderInfos services, responsible for OrderInfo-related CRUD operations
pub struct InvoiceServiceImpl<
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
> {
    pub db_pool: Pool<M>,
    pub cpu_pool: CpuPool,
    pub http_client: ClientHandle,
    user_id: Option<UserId>,
    pub repo_factory: F,
    pub invoice_url: String,
    pub callback_url: String,
    pub saga_url: String,
    pub login_url: String,
    pub credentials: ExternalBillingCredentials,
    pub timeout_s: i32,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > InvoiceServiceImpl<T, M, F>
{
    pub fn new(
        db_pool: Pool<M>,
        cpu_pool: CpuPool,
        http_client: ClientHandle,
        user_id: Option<UserId>,
        repo_factory: F,
        config: Config,
    ) -> Self {
        let credentials = ExternalBillingCredentials::new(config.external_billing.username, config.external_billing.password);
        Self {
            db_pool,
            cpu_pool,
            http_client,
            user_id,
            repo_factory,
            invoice_url: config.external_billing.invoice_url,
            callback_url: config.callback.url,
            saga_url: config.saga_addr.url,
            login_url: config.external_billing.login_url,
            credentials,
            timeout_s: config.external_billing.amount_recalculate_timeout_sec,
        }
    }
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > InvoiceService for InvoiceServiceImpl<T, M, F>
{
    /// Creates orders in billing system, returning url for payment
    fn create(&self, create_invoice: CreateInvoice) -> ServiceFuture<Invoice> {
        let db_clone = self.db_pool.clone();
        let user_id = self.user_id;
        let repo_factory = self.repo_factory.clone();
        let client = self.http_client.clone();
        let invoice_url = self.invoice_url.clone();
        let callback_url = self.callback_url.clone();
        let login_url = self.login_url.clone();
        let credentials = self.credentials.clone();
        let timeout_s = self.timeout_s.clone();

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_clone
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let order_info_repo = repo_factory.create_order_info_repo(&conn, user_id);
                            let merchant_repo = repo_factory.create_merchant_repo(&conn, user_id);
                            let invoice_repo = repo_factory.create_invoice_repo(&conn, user_id);

                            conn.transaction::<Invoice, FailureError, _>(move || {
                                debug!("Creating new invoice: {}", &create_invoice);
                                let saga_id = create_invoice.saga_id.clone();
                                let customer_id = create_invoice.customer_id.clone();
                                create_invoice
                                    .orders
                                    .iter()
                                    .map(|order| {
                                        let payload = NewOrderInfo::new(
                                            order.id.clone(),
                                            saga_id.clone(),
                                            customer_id.clone(),
                                            order.store_id.clone(),
                                        );
                                        order_info_repo.create(payload).and_then(|_| {
                                            merchant_repo
                                                .get_by_subject_id(SubjectIdentifier::Store(order.store_id.clone()))
                                                .map(|merchant| BillingOrder::new(order.clone(), merchant.merchant_id))
                                        })
                                    })
                                    .collect::<RepoResult<Vec<BillingOrder>>>()
                                    .and_then(|orders| {
                                        let body = serde_json::to_string(&credentials)?;
                                        let url = format!("{}", login_url);
                                        let mut headers = Headers::new();
                                        headers.set(ContentType::json());
                                        client
                                            .request::<ExternalBillingToken>(Post, url, Some(body), Some(headers))
                                            .map_err(|e| {
                                                e.context("Occured an error during receiving authorization token in external billing.")
                                                    .context(Error::HttpClient)
                                                    .into()
                                            })
                                            .wait()
                                            .and_then(|ext_token| {
                                                let mut headers = Headers::new();
                                                headers.set(Authorization(Bearer { token: ext_token.token }));
                                                headers.set(ContentType::json());
                                                let callback = format!("{}", callback_url);
                                                let billing_payload = CreateInvoicePayload::new(
                                                    orders,
                                                    callback,
                                                    create_invoice.currency_id.to_string(),
                                                    timeout_s,
                                                );
                                                let body = serde_json::to_string(&billing_payload)?;
                                                let url = format!("{}", invoice_url);
                                                client
                                                    .request::<ExternalBillingInvoice>(Post, url, Some(body), Some(headers))
                                                    .map_err(|e| {
                                                        e.context("Occured an error during invoice creation in external billing.")
                                                            .context(Error::HttpClient)
                                                            .into()
                                                    })
                                                    .wait()
                                            })
                                    })
                                    .and_then(|invoice| {
                                        let payload = Invoice::new(saga_id, invoice);
                                        invoice_repo.create(payload)
                                    })
                            })
                        })
                })
                .map_err(|e: FailureError| e.context("Service invoice, create endpoint error occured.").into()),
        )
    }

    /// Delete invoice
    fn delete(&self, id: SagaId) -> ServiceFuture<SagaId> {
        let db_clone = self.db_pool.clone();
        let user_id = self.user_id;
        let repo_factory = self.repo_factory.clone();

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_clone
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let invoice_repo = repo_factory.create_invoice_repo(&conn, user_id);
                            let order_info_repo = repo_factory.create_order_info_repo(&conn, user_id);
                            conn.transaction::<SagaId, FailureError, _>(move || {
                                debug!("Deleting invoice: {}", &id);
                                invoice_repo
                                    .delete(id)
                                    .and_then(|invoice| order_info_repo.delete_by_saga_id(invoice.id).map(|_| invoice.id))
                            })
                        })
                })
                .map_err(|e: FailureError| e.context("Service invoice, delete endpoint error occured.").into()),
        )
    }

    /// Updates specific invoice and orders
    fn update(&self, external_invoice: ExternalBillingInvoice) -> ServiceFuture<String> {
        let db_clone = self.db_pool.clone();
        let current_user = self.user_id;
        let client = self.http_client.clone();
        let repo_factory = self.repo_factory.clone();
        let saga_url = self.saga_url.clone();

        debug!("Update invoice by external invoice {:?}.", &external_invoice);

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_clone
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let order_info_repo = repo_factory.create_order_info_repo(&conn, current_user);
                            let invoice_repo = repo_factory.create_invoice_repo(&conn, current_user);
                            let invoice_id = external_invoice.id;
                            let update_payload = external_invoice.into();
                            invoice_repo
                                .update(invoice_id, update_payload)
                                .and_then(|invoice| order_info_repo.update_status(invoice.id, invoice.state))
                        })
                        .and_then(|orders| {
                            let body = serde_json::to_string(&orders)?;
                            let url = format!("{}/orders/update_state", saga_url);
                            client
                                .request::<String>(Post, url, Some(body), None)
                                .map_err(|e| {
                                    e.context("Occured an error during setting orders paid in saga.")
                                        .context(Error::HttpClient)
                                        .into()
                                })
                                .wait()
                        })
                })
                .map_err(|e: FailureError| e.context("Service invoice, update endpoint error occured.").into()),
        )
    }

    /// Get invoice by order id
    fn get_by_order_id(&self, order_id: OrderId) -> ServiceFuture<Option<Invoice>> {
        let db_clone = self.db_pool.clone();
        let user_id = self.user_id;
        let repo_factory = self.repo_factory.clone();

        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_clone
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let invoice_repo = repo_factory.create_invoice_repo(&conn, user_id);
                            let order_info_repo = repo_factory.create_order_info_repo(&conn, user_id);
                            debug!("Requesting invoice by order id: {}", &order_id);

                            order_info_repo.find_by_order_id(order_id).and_then(|order_info| {
                                if let Some(order_info) = order_info {
                                    invoice_repo.find_by_saga_id(order_info.saga_id)
                                } else {
                                    Ok(None)
                                }
                            })
                        })
                })
                .map_err(|e: FailureError| e.context("Service invoice, get_by_order_id endpoint error occured.").into()),
        )
    }
    /// Get invoice by invoice id
    fn get_by_id(&self, id: InvoiceId) -> ServiceFuture<Option<Invoice>> {
        let db_clone = self.db_pool.clone();
        let user_id = self.user_id;
        let repo_factory = self.repo_factory.clone();
        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_clone
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let invoice_repo = repo_factory.create_invoice_repo(&conn, user_id);
                            debug!("Requesting invoice by invoice id: {}", &id);
                            invoice_repo.find(id)
                        })
                })
                .map_err(|e: FailureError| e.context("Service invoice, get_by_id endpoint error occured.").into()),
        )
    }

    /// Get orders ids by invoice id
    fn get_orders_ids(&self, id: InvoiceId) -> ServiceFuture<Vec<OrderId>> {
        let db_clone = self.db_pool.clone();
        let user_id = self.user_id;
        let repo_factory = self.repo_factory.clone();
        Box::new(
            self.cpu_pool
                .spawn_fn(move || {
                    db_clone
                        .get()
                        .map_err(|e| e.context(Error::Connection).into())
                        .and_then(move |conn| {
                            let invoice_repo = repo_factory.create_invoice_repo(&conn, user_id);
                            let order_info_repo = repo_factory.create_order_info_repo(&conn, user_id);
                            debug!("Requesting vec order ids by invoice id: {}", &id);

                            invoice_repo.find(id).and_then(|invoice| {
                                if let Some(invoice) = invoice {
                                    order_info_repo
                                        .find_by_saga_id(invoice.id)
                                        .map(|order_infos| order_infos.into_iter().map(|order_info| order_info.order_id).collect())
                                } else {
                                    Ok(vec![])
                                }
                            })
                        })
                })
                .map_err(|e: FailureError| e.context("Service invoice, get_orders_ids endpoint error occured.").into()),
        )
    }
}

#[cfg(test)]
pub mod tests {

    use std::sync::Arc;
    use tokio_core::reactor::Core;

    use stq_static_resources::*;
    use stq_types::*;

    use models::*;
    use repos::repo_factory::tests::*;
    use services::invoice::InvoiceService;

    #[test]
    fn test_create_order_info() {
        let mut core = Core::new().unwrap();
        let handle = Arc::new(core.handle());
        let service = create_invoice_service(Some(UserId(1)), handle);
        let order = Order {
            id: OrderId::new(),
            store_id: StoreId(1),
            price: ProductPrice(3232.32),
            currency_id: CurrencyId(1),
        };
        let create_order = CreateInvoice {
            saga_id: SagaId::new(),
            customer_id: UserId(1),
            orders: vec![order],
            currency_id: CurrencyId(Currency::Stq as i32),
        };
        let work = service.create(create_order);
        let _result = core.run(work).unwrap();
    }

    #[test]
    fn test_set_paid() {
        let mut core = Core::new().unwrap();
        let handle = Arc::new(core.handle());
        let service = create_invoice_service(Some(UserId(1)), handle);
        let invoice = ExternalBillingInvoice {
            id: InvoiceId::new(),
            amount: "0.000000000".to_string(),
            status: ExternalBillingStatus::New,
            wallet: Some("wallet".to_string()),
            amount_captured: "0.000000000".to_string(),
            transactions: None,
            currency: "stq".to_string(),
        };
        let work = service.update(invoice);
        let _result = core.run(work).unwrap();
    }

}
