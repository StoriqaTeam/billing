//! OrderInfos Services, presents CRUD operations with order_info

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;
use futures::{Future, IntoFuture};
use hyper::header::{Authorization, Bearer, ContentType};
use hyper::Headers;
use hyper::Post;
use r2d2::ManageConnection;
use serde_json;

use stq_types::{InvoiceId, OrderId, SagaId};

use super::types::ServiceFuture;
use config::ExternalBilling;
use errors::Error;
use models::*;
use repos::repo_factory::ReposFactory;
use repos::RepoResult;
use services::Service;

pub trait InvoiceService {
    /// Creates invoice in billing system
    fn create_invoice(&self, create_invoice: CreateInvoice) -> ServiceFuture<Invoice>;
    /// Get invoice by order id
    fn get_invoice_by_order_id(&self, order_id: OrderId) -> ServiceFuture<Option<Invoice>>;
    /// Get invoice by invoice id
    fn get_invoice_by_id(&self, id: InvoiceId) -> ServiceFuture<Option<Invoice>>;
    /// Recalc invoice by invoice id
    fn recalc_invoice(&self, id: InvoiceId) -> ServiceFuture<Invoice>;
    /// Get orders ids by invoice id
    fn get_invoice_orders_ids(&self, id: InvoiceId) -> ServiceFuture<Vec<OrderId>>;
    /// Delete invoice merchant
    fn delete_invoice_by_saga_id(&self, id: SagaId) -> ServiceFuture<SagaId>;
    /// Creates orders in billing system, returning url for payment
    fn update_invoice(&self, invoice: ExternalBillingInvoice) -> ServiceFuture<()>;
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > InvoiceService for Service<T, M, F>
{
    /// Creates orders in billing system, returning url for payment
    fn create_invoice(&self, create_invoice: CreateInvoice) -> ServiceFuture<Invoice> {
        let user_id = self.dynamic_context.user_id;
        let repo_factory = self.static_context.repo_factory.clone();
        let client = self.static_context.client_handle.clone();
        let callback_url = self.static_context.config.callback.url.clone();
        let ExternalBilling {
            invoice_url,
            login_url,
            username,
            password,
            amount_recalculate_timeout_sec,
            ..
        } = self.static_context.config.external_billing.clone();
        let credentials = ExternalBillingCredentials::new(username, password);

        self.spawn_on_pool(move |conn| {
            let order_info_repo = repo_factory.create_order_info_repo(&conn, user_id);
            let merchant_repo = repo_factory.create_merchant_repo(&conn, user_id);
            let invoice_repo = repo_factory.create_invoice_repo(&conn, user_id);

            conn.transaction::<Invoice, FailureError, _>(move || {
                debug!("Creating new invoice: {}", &create_invoice);
                let saga_id = create_invoice.saga_id;
                let customer_id = create_invoice.customer_id;
                create_invoice
                    .orders
                    .iter()
                    .map(|order| {
                        let payload = NewOrderInfo::new(order.id, saga_id, customer_id, order.store_id);
                        order_info_repo.create(payload).and_then(|_| {
                            merchant_repo
                                .get_by_subject_id(SubjectIdentifier::Store(order.store_id))
                                .map(|merchant| BillingOrder::new(&order, merchant.merchant_id))
                        })
                    }).collect::<RepoResult<Vec<BillingOrder>>>()
                    .and_then(|orders| {
                        let body = serde_json::to_string(&credentials)?;
                        let url = login_url.to_string();
                        let mut headers = Headers::new();
                        headers.set(ContentType::json());
                        client
                            .request::<ExternalBillingToken>(Post, url, Some(body), Some(headers))
                            .map_err(|e| {
                                e.context("Occured an error during receiving authorization token in external billing.")
                                    .context(Error::HttpClient)
                                    .into()
                            }).and_then(|ext_token| {
                                let mut headers = Headers::new();
                                headers.set(Authorization(Bearer { token: ext_token.token }));
                                headers.set(ContentType::json());
                                let callback = callback_url.to_string();
                                let billing_payload = CreateInvoicePayload::new(
                                    orders,
                                    callback,
                                    create_invoice.currency.to_string(),
                                    amount_recalculate_timeout_sec,
                                );
                                let url = invoice_url.to_string();
                                serde_json::to_string(&billing_payload)
                                    .map_err(|e| {
                                        e.context("Occured an error during invoice creation payload serialization.")
                                            .context(Error::Parse)
                                            .into()
                                    }).into_future()
                                    .and_then(|body| {
                                        client
                                            .request::<ExternalBillingInvoice>(Post, url, Some(body), Some(headers))
                                            .map_err(|e| {
                                                e.context("Occured an error during invoice creation in external billing.")
                                                    .context(Error::HttpClient)
                                                    .into()
                                            })
                                    })
                            }).wait()
                    }).and_then(|invoice| {
                        let payload = Invoice::new(saga_id, invoice);
                        invoice_repo.create(payload)
                    })
            }).map_err(|e: FailureError| e.context("Service invoice, create endpoint error occured.").into())
        })
    }

    /// Get invoice by order id
    fn get_invoice_by_order_id(&self, order_id: OrderId) -> ServiceFuture<Option<Invoice>> {
        let user_id = self.dynamic_context.user_id;
        let repo_factory = self.static_context.repo_factory.clone();

        self.spawn_on_pool(move |conn| {
            let invoice_repo = repo_factory.create_invoice_repo(&conn, user_id);
            let order_info_repo = repo_factory.create_order_info_repo(&conn, user_id);
            debug!("Requesting invoice by order id: {}", &order_id);

            order_info_repo
                .find_by_order_id(order_id)
                .and_then(|order_info| {
                    if let Some(order_info) = order_info {
                        invoice_repo.find_by_saga_id(order_info.saga_id)
                    } else {
                        Ok(None)
                    }
                }).map_err(|e: FailureError| e.context("Service invoice, get_by_order_id endpoint error occured.").into())
        })
    }
    /// Get invoice by invoice id
    fn get_invoice_by_id(&self, id: InvoiceId) -> ServiceFuture<Option<Invoice>> {
        let repo_factory = self.static_context.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;
        self.spawn_on_pool(move |conn| {
            let invoice_repo = repo_factory.create_invoice_repo(&conn, user_id);
            debug!("Requesting invoice by invoice id: {}", &id);
            invoice_repo
                .find(id)
                .map_err(|e: FailureError| e.context("Service invoice, get_by_id endpoint error occured.").into())
        })
    }

    /// Recalc invoice by invoice id
    fn recalc_invoice(&self, id: InvoiceId) -> ServiceFuture<Invoice> {
        let user_id = self.dynamic_context.user_id;
        let repo_factory = self.static_context.repo_factory.clone();
        let client = self.static_context.client_handle.clone();
        let ExternalBilling {
            invoice_url,
            login_url,
            username,
            password,
            ..
        } = self.static_context.config.external_billing.clone();
        let credentials = ExternalBillingCredentials::new(username, password);
        let saga_url = self.static_context.config.saga_addr.url.clone();

        self.spawn_on_pool(move |conn| {
            let invoice_repo = repo_factory.create_invoice_repo(&conn, user_id);
            let order_info_repo = repo_factory.create_order_info_repo(&conn, user_id);

            conn.transaction::<Invoice, FailureError, _>(move || {
                debug!("Recalculating invoice with id: {}", &id);
                let body = serde_json::to_string(&credentials)?;
                let url = login_url.to_string();
                let mut headers = Headers::new();
                headers.set(ContentType::json());
                client
                    .request::<ExternalBillingToken>(Post, url, Some(body), Some(headers))
                    .map_err(|e| {
                        e.context("Occured an error during receiving authorization token in external billing.")
                            .context(Error::HttpClient)
                            .into()
                    }).and_then(|ext_token| {
                        let mut headers = Headers::new();
                        headers.set(Authorization(Bearer { token: ext_token.token }));
                        headers.set(ContentType::json());
                        let url = format!("{}{}/recalc/", invoice_url.to_string(), id);
                        client
                            .request::<ExternalBillingInvoice>(Post, url, None, Some(headers))
                            .map_err(|e| {
                                e.context("Occured an error during invoice recalculation in external billing.")
                                    .context(Error::HttpClient)
                                    .into()
                            })
                    }).wait()
                    .and_then(|invoice| invoice_repo.update(id, invoice.into()))
                    .and_then(|invoice| {
                        order_info_repo
                            .update_status(invoice.id, invoice.state)
                            .and_then(|orders| {
                                let body = serde_json::to_string(&orders)?;
                                let url = format!("{}/orders/update_state", saga_url);
                                client
                                    .request::<()>(Post, url, Some(body), None)
                                    .map_err(|e| {
                                        e.context("Occured an error during setting orders new status in saga.")
                                            .context(Error::HttpClient)
                                            .into()
                                    }).wait()
                            }).map(|_| invoice)
                    })
            }).map_err(|e: FailureError| e.context("Service invoice, recalc endpoint error occured.").into())
        })
    }

    /// Get orders ids by invoice id
    fn get_invoice_orders_ids(&self, id: InvoiceId) -> ServiceFuture<Vec<OrderId>> {
        let user_id = self.dynamic_context.user_id;
        let repo_factory = self.static_context.repo_factory.clone();
        self.spawn_on_pool(move |conn| {
            let invoice_repo = repo_factory.create_invoice_repo(&conn, user_id);
            let order_info_repo = repo_factory.create_order_info_repo(&conn, user_id);
            debug!("Requesting vec order ids by invoice id: {}", &id);

            invoice_repo
                .find(id)
                .and_then(|invoice| {
                    if let Some(invoice) = invoice {
                        order_info_repo
                            .find_by_saga_id(invoice.id)
                            .map(|order_infos| order_infos.into_iter().map(|order_info| order_info.order_id).collect())
                    } else {
                        Ok(vec![])
                    }
                }).map_err(|e: FailureError| e.context("Service invoice, get_orders_ids endpoint error occured.").into())
        })
    }

    /// Delete invoice
    fn delete_invoice_by_saga_id(&self, id: SagaId) -> ServiceFuture<SagaId> {
        let user_id = self.dynamic_context.user_id;
        let repo_factory = self.static_context.repo_factory.clone();

        self.spawn_on_pool(move |conn| {
            let invoice_repo = repo_factory.create_invoice_repo(&conn, user_id);
            let order_info_repo = repo_factory.create_order_info_repo(&conn, user_id);
            conn.transaction::<SagaId, FailureError, _>(move || {
                debug!("Deleting invoice: {}", &id);
                invoice_repo
                    .delete(id)
                    .and_then(|invoice| order_info_repo.delete_by_saga_id(invoice.id).map(|_| invoice.id))
            }).map_err(|e: FailureError| e.context("Service invoice, delete endpoint error occured.").into())
        })
    }

    /// Updates specific invoice and orders
    fn update_invoice(&self, external_invoice: ExternalBillingInvoice) -> ServiceFuture<()> {
        let current_user = self.dynamic_context.user_id;
        let client = self.static_context.client_handle.clone();
        let repo_factory = self.static_context.repo_factory.clone();
        let saga_url = self.static_context.config.saga_addr.url.clone();

        debug!("Updating by external invoice {:?}.", &external_invoice);

        self.spawn_on_pool(move |conn| {
            let order_info_repo = repo_factory.create_order_info_repo(&conn, current_user);
            let invoice_repo = repo_factory.create_invoice_repo(&conn, current_user);
            let invoice_id = external_invoice.id;
            let update_payload = external_invoice.into();
            conn.transaction::<(), FailureError, _>(move || {
                invoice_repo
                    .update(invoice_id, update_payload)
                    .and_then(|invoice| order_info_repo.update_status(invoice.id, invoice.state))
                    .and_then(|orders| {
                        let body = serde_json::to_string(&orders)?;
                        let url = format!("{}/orders/update_state", saga_url);
                        client
                            .request::<()>(Post, url, Some(body), None)
                            .map_err(|e| {
                                e.context("Occured an error during setting orders new status in saga.")
                                    .context(Error::HttpClient)
                                    .into()
                            }).wait()
                    })
            }).map_err(|e: FailureError| e.context("Service invoice, update endpoint error occured.").into())
        })
    }
}

#[cfg(test)]
pub mod tests {

    use std::sync::Arc;
    use std::time::SystemTime;
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
        let service = create_service(Some(UserId(1)), handle);
        let order = Order {
            id: OrderId::new(),
            store_id: StoreId(1),
            price: ProductPrice(3232.32),
            quantity: Quantity(1),
            currency: Currency::STQ,
        };
        let create_order = CreateInvoice {
            saga_id: SagaId::new(),
            customer_id: UserId(1),
            orders: vec![order],
            currency: Currency::STQ,
        };
        let work = service.create_invoice(create_order);
        let _result = core.run(work).unwrap();
    }

    #[test]
    fn test_set_paid() {
        let mut core = Core::new().unwrap();
        let handle = Arc::new(core.handle());
        let service = create_service(Some(UserId(1)), handle);
        let invoice = ExternalBillingInvoice {
            id: InvoiceId::new(),
            amount: "0.000000000".to_string(),
            status: ExternalBillingStatus::New,
            wallet: Some("wallet".to_string()),
            amount_captured: "0.000000000".to_string(),
            transactions: None,
            currency: Currency::STQ,
            expired: SystemTime::now().into(),
        };
        let work = service.update_invoice(invoice);
        let _result = core.run(work).unwrap();
    }

}
