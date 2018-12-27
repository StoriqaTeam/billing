//! OrderInfos Services, presents CRUD operations with order_info

use bigdecimal::BigDecimal;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::{err_msg, Error as FailureError, Fail};
use futures::{future, stream, Future, IntoFuture, Stream};
use hyper::header::{Authorization, Bearer, ContentType};
use hyper::Headers;
use hyper::Post;
use r2d2::ManageConnection;
use serde_json;
use uuid::Uuid;

use stq_http::client::HttpClient;
use stq_types::{InvoiceId, OrderId, SagaId};

use client::payments::{GetRate, PaymentsClient, Rate, RateRefresh};
use config::ExternalBilling;
use controller::context::DynamicContext;
use errors::Error;
use models::invoice_v2::{calculate_invoice_price, InvoiceDump, InvoiceId as InvoiceV2Id, NewInvoice};
use models::order_v2::{ExchangeId, NewOrder, RawOrder};
use models::*;
use repos::repo_factory::ReposFactory;
use repos::{InvoicesV2Repo, OrderExchangeRatesRepo, OrdersRepo, RepoResult};
use services::accounts::AccountService;
use services::types::spawn_on_pool;
use services::Service;

use super::error::{Error as ServiceError, ErrorKind};
use super::types::{ServiceFuture, ServiceFutureV2};

pub trait InvoiceService {
    /// Creates invoice in billing system
    fn create_invoice(&self, create_invoice: CreateInvoice) -> ServiceFuture<Invoice>;
    fn create_invoice_v2(&self, create_invoice: CreateInvoiceV2) -> ServiceFutureV2<InvoiceDump>;
    /// Get invoice by order id
    fn get_invoice_by_order_id(&self, order_id: OrderId) -> ServiceFuture<Option<Invoice>>;
    /// Get invoice by invoice id
    fn get_invoice_by_id(&self, id: InvoiceId) -> ServiceFuture<Option<Invoice>>;
    /// Recalc invoice by invoice id
    fn recalc_invoice(&self, id: InvoiceId) -> ServiceFuture<Invoice>;
    /// Refreshes all rates for the invoice and calculates the total price of the invoice.
    /// Either calculate the current total price of the invoice or get the final price if the invoice has been paid
    fn recalc_invoice_v2(&self, id: InvoiceV2Id) -> ServiceFutureV2<Option<InvoiceDump>>;
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
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone + 'static,
    > InvoiceService for Service<T, M, F, C, PC, AS>
{
    /// Creates orders in billing system, returning url for payment
    fn create_invoice(&self, create_invoice: CreateInvoice) -> ServiceFuture<Invoice> {
        let user_id = self.dynamic_context.user_id;
        let repo_factory = self.static_context.repo_factory.clone();
        let client = self.dynamic_context.http_client.clone();
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
                        let payload = NewOrderInfo::new(order.id, saga_id, customer_id, order.store_id, order.total_amount);
                        order_info_repo.create(payload).and_then(|_| {
                            merchant_repo
                                .get_by_subject_id(SubjectIdentifier::Store(order.store_id))
                                .map(|merchant| BillingOrder::new(&order, merchant.merchant_id))
                        })
                    })
                    .collect::<RepoResult<Vec<BillingOrder>>>()
                    .and_then(|orders| {
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
                            .and_then(|ext_token| {
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
                                    })
                                    .into_future()
                                    .and_then(|body| {
                                        client
                                            .request_json::<ExternalBillingInvoice>(Post, url, Some(body), Some(headers))
                                            .map_err(|e| {
                                                e.context("Occured an error during invoice creation in external billing.")
                                                    .context(Error::HttpClient)
                                                    .into()
                                            })
                                    })
                            })
                            .wait()
                    })
                    .and_then(|invoice| {
                        let payload = Invoice::new(saga_id, invoice);
                        invoice_repo.create(payload)
                    })
            })
            .map_err(|e: FailureError| e.context("Service invoice, create endpoint error occured.").into())
        })
    }

    fn create_invoice_v2(&self, create_invoice: CreateInvoiceV2) -> ServiceFutureV2<InvoiceDump> {
        let repo_factory = self.static_context.repo_factory.clone();
        let DynamicContext {
            user_id,
            payments_client,
            account_service,
            ..
        } = self.dynamic_context.clone();

        let (payments_client, account_service) = if let (Some(payments_client), Some(account_service)) = (payments_client, account_service)
        {
            (payments_client, account_service)
        } else {
            let e = err_msg("payments integration has not been configured");
            return Box::new(future::err::<_, ServiceError>(ectx!(err e, ErrorKind::Internal)));
        };

        let CreateInvoiceV2 {
            orders,
            customer_id: buyer_user_id,
            currency: buyer_currency,
            saga_id: invoice_id,
        } = create_invoice;

        let db_pool = self.static_context.db_pool.clone();
        let cpu_pool = self.static_context.cpu_pool.clone();

        let fut = stream::iter_ok::<_, ServiceError>(orders.into_iter().map(move |order| (payments_client.clone(), order)))
            .and_then(move |(payments_client, create_order)| {
                let CreateOrderV2 {
                    id,
                    store_id,
                    currency: seller_currency,
                    total_amount: seller_total_amount,
                    product_cashback: seller_cashback_percent,
                } = create_order;

                let total_amount = Amount::from_super_unit(seller_currency, seller_total_amount);
                let cashback_amount = match seller_cashback_percent {
                    None => Amount::new(0),
                    Some(cashback_fraction) => Amount::from_super_unit(seller_currency, seller_total_amount * cashback_fraction),
                };

                let new_order = NewOrder {
                    id,
                    seller_currency,
                    total_amount,
                    cashback_amount,
                    invoice_id: invoice_id.clone(),
                    store_id,
                };

                get_rate(&payments_client, buyer_currency, seller_currency, total_amount)
                    .map(|(exchange_id, exchange_rate)| (new_order, exchange_id, exchange_rate))
            })
            .collect()
            .and_then(move |orders| {
                account_service
                    .get_or_create_free_pooled_account(buyer_currency)
                    .map_err(ectx!(convert => buyer_currency))
                    .map(|account| (account.id, orders))
            })
            .and_then(move |(account_id, orders)| {
                cpu_pool.spawn_fn(move || {
                    db_pool.get().map_err(ectx!(ErrorKind::Internal)).and_then(move |conn| {
                        let invoices_repo = repo_factory.create_invoices_v2_repo(&conn, user_id);
                        let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
                        let order_exchange_rates_repo = repo_factory.create_order_exchange_rates_repo(&conn, user_id);

                        conn.transaction::<InvoiceDump, ServiceError, _>(move || {
                            let invoice = NewInvoice {
                                id: invoice_id,
                                account_id: Some(account_id),
                                buyer_currency,
                                amount_captured: Amount::new(0u128),
                                buyer_user_id,
                            };

                            let invoice = invoices_repo.create(invoice.clone()).map_err(ectx!(try convert => invoice))?;

                            let orders_with_rates = orders
                                .into_iter()
                                .map(|(new_order, exchange_id, exchange_rate)| {
                                    let order_id = new_order.id;

                                    let order = orders_repo.create(new_order.clone()).map_err(ectx!(try convert => new_order))?;

                                    let new_rate = NewOrderExchangeRate {
                                        order_id,
                                        exchange_id,
                                        exchange_rate,
                                    };

                                    let rate = order_exchange_rates_repo
                                        .add_new_active_rate(new_rate.clone())
                                        .map_err(ectx!(try convert => new_rate))?;

                                    Ok((order, vec![rate.active_rate]))
                                })
                                .collect::<Result<Vec<_>, ServiceError>>()?;

                            Ok(calculate_invoice_price(invoice, orders_with_rates))
                        })
                    })
                })
            });

        Box::new(fut)
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
                })
                .map_err(|e: FailureError| e.context("Service invoice, get_by_order_id endpoint error occured.").into())
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
        let client = self.dynamic_context.http_client.clone();
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
                    .request_json::<ExternalBillingToken>(Post, url, Some(body), Some(headers))
                    .map_err(|e| {
                        e.context("Occured an error during receiving authorization token in external billing.")
                            .context(Error::HttpClient)
                            .into()
                    })
                    .and_then(|ext_token| {
                        let mut headers = Headers::new();
                        headers.set(Authorization(Bearer { token: ext_token.token }));
                        headers.set(ContentType::json());
                        let url = format!("{}{}/recalc/", invoice_url.to_string(), id);
                        client
                            .request_json::<ExternalBillingInvoice>(Post, url, None, Some(headers))
                            .map_err(|e| {
                                e.context("Occured an error during invoice recalculation in external billing.")
                                    .context(Error::HttpClient)
                                    .into()
                            })
                    })
                    .wait()
                    .and_then(|invoice| invoice_repo.update(id, invoice.into()))
                    .and_then(|invoice| {
                        order_info_repo
                            .update_status(invoice.id, invoice.state)
                            .and_then(|orders| {
                                let body = serde_json::to_string(&orders)?;
                                let url = format!("{}/orders/update_state", saga_url);
                                client
                                    .request_json::<()>(Post, url, Some(body), None)
                                    .map_err(|e| {
                                        e.context("Occured an error during setting orders new status in saga.")
                                            .context(Error::HttpClient)
                                            .into()
                                    })
                                    .wait()
                            })
                            .map(|_| invoice)
                    })
            })
            .map_err(|e: FailureError| e.context("Service invoice, recalc endpoint error occured.").into())
        })
    }

    // TODO: notify saga (/orders/update_state)
    fn recalc_invoice_v2(&self, id: InvoiceV2Id) -> ServiceFutureV2<Option<InvoiceDump>> {
        let payments_client = if let Some(payments_client) = self.dynamic_context.payments_client.clone() {
            payments_client
        } else {
            let e = err_msg("payments integration has not been configured");
            return Box::new(future::err::<_, ServiceError>(ectx!(err e, ErrorKind::Internal)));
        };

        let db_pool = self.static_context.db_pool.clone();
        let cpu_pool = self.static_context.cpu_pool.clone();

        let fut = spawn_on_pool(db_pool, cpu_pool, {
            let user_id = self.dynamic_context.user_id.clone();
            let repo_factory = self.static_context.repo_factory.clone();

            move |conn| {
                let invoices_repo = repo_factory.create_invoices_v2_repo(&conn, user_id);
                let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
                let rates_repo = repo_factory.create_order_exchange_rates_repo(&conn, user_id);

                let id_clone = id.clone();
                let invoice = invoices_repo.get(id_clone.clone()).map_err(ectx!(try convert => id_clone))?;

                let invoice = match invoice {
                    None => {
                        return Ok(None);
                    }
                    Some(invoice) => invoice,
                };

                let current_order_rates = orders_repo
                    .get_many_by_invoice_id(id.clone())
                    .map_err(ectx!(try convert => id_clone))?
                    .into_iter()
                    .map(|order| {
                        let order_id = order.id.clone();
                        rates_repo
                            .get_active_rate_for_order(order_id.clone())
                            .map_err(ectx!(convert => order_id))
                            .map(|rate| (order, rate))
                    })
                    .collect::<Result<Vec<_>, ServiceError>>()?;

                Ok(Some((invoice, current_order_rates)))
            }
        })
        .and_then({
            let db_pool = self.static_context.db_pool.clone();
            let cpu_pool = self.static_context.cpu_pool.clone();
            let repo_factory = self.static_context.repo_factory.clone();
            let user_id = self.dynamic_context.user_id;

            move |invoice_data| match invoice_data {
                None => future::Either::A(future::ok(None)),
                Some((invoice, current_order_rates)) => future::Either::B(Some(future::lazy(move || {
                    refresh_rates(payments_client, invoice.buyer_currency.clone(), current_order_rates)
                        .and_then({
                            let db_pool = db_pool.clone();
                            let cpu_pool = cpu_pool.clone();
                            let repo_factory = repo_factory.clone();

                            move |new_active_rates| {
                                spawn_on_pool(db_pool, cpu_pool, move |conn| {
                                    let rates_repo = repo_factory.create_order_exchange_rates_repo(&conn, user_id);

                                    new_active_rates
                                        .into_iter()
                                        .map(|new_rate| {
                                            rates_repo
                                                .add_new_active_rate(new_rate.clone())
                                                .map_err(ectx!(convert => new_rate))
                                                .map(|_| ())
                                        })
                                        .collect::<Result<Vec<_>, ServiceError>>()
                                })
                            }
                        })
                        .and_then(move |_| {
                            spawn_on_pool(db_pool, cpu_pool, move |conn| {
                                let invoice_id = invoice.id.clone();

                                let invoices_repo = repo_factory.create_invoices_v2_repo(&conn, user_id);
                                let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
                                let rates_repo = repo_factory.create_order_exchange_rates_repo(&conn, user_id);

                                get_invoice_price(&*invoices_repo, &*orders_repo, &*rates_repo, invoice_id)?.ok_or_else(|| {
                                    let e = format_err!("Invoice with ID {} got deleted during recalc", invoice_id);
                                    ectx!(err e, ErrorKind::Internal => invoice_id)
                                })
                            })
                        })
                }))),
            }
        });

        Box::new(fut)
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
                })
                .map_err(|e: FailureError| e.context("Service invoice, get_orders_ids endpoint error occured.").into())
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
            })
            .map_err(|e: FailureError| e.context("Service invoice, delete endpoint error occured.").into())
        })
    }

    /// Updates specific invoice and orders
    fn update_invoice(&self, external_invoice: ExternalBillingInvoice) -> ServiceFuture<()> {
        let current_user = self.dynamic_context.user_id;
        let client = self.dynamic_context.http_client.clone();
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
                            .request_json::<()>(Post, url, Some(body), None)
                            .map_err(|e| {
                                e.context("Occured an error during setting orders new status in saga.")
                                    .context(Error::HttpClient)
                                    .into()
                            })
                            .wait()
                    })
            })
            .map_err(|e: FailureError| e.context("Service invoice, update endpoint error occured.").into())
        })
    }
}

pub fn get_rate<PC: PaymentsClient + Send + Clone + 'static>(
    payments_client: &PC,
    buyer_currency: Currency,
    seller_currency: Currency,
    total_amount: Amount,
) -> Box<Future<Item = (Option<ExchangeId>, BigDecimal), Error = ServiceError>> {
    Box::new(if buyer_currency == seller_currency {
        future::Either::A(future::ok((None, BigDecimal::from(1))))
    } else {
        let input = GetRate {
            id: Uuid::new_v4(),
            from: buyer_currency,
            to: seller_currency,
            amount_currency: seller_currency,
            amount: total_amount,
        };

        future::Either::B(
            payments_client
                .get_rate(input.clone())
                .map(|Rate { id, rate, .. }| (Some(ExchangeId::new(id)), rate))
                .map_err(ectx!(ErrorKind::Internal => input)),
        )
    })
}

pub fn get_invoice_price(
    invoices_repo: &InvoicesV2Repo,
    orders_repo: &OrdersRepo,
    rates_repo: &OrderExchangeRatesRepo,
    invoice_id: InvoiceV2Id,
) -> Result<Option<InvoiceDump>, ServiceError> {
    let invoice = invoices_repo.get(invoice_id.clone()).map_err(ectx!(try convert => invoice_id))?;

    let invoice = match invoice {
        None => return Ok(None),
        Some(invoice) => invoice,
    };

    let orders_with_rates = orders_repo
        .get_many_by_invoice_id(invoice_id.clone())
        .map_err(ectx!(try convert => invoice_id))?
        .into_iter()
        .map(|order| {
            let order_id = order.id.clone();
            rates_repo
                .get_all_rates_for_order(order_id.clone())
                .map_err(ectx!(convert => order_id))
                .map(|rates| (order, rates))
        })
        .collect::<Result<Vec<_>, ServiceError>>()?;

    Ok(Some(calculate_invoice_price(invoice, orders_with_rates)))
}

pub fn refresh_rates<PC: PaymentsClient + Send + Clone + 'static>(
    payments_client: PC,
    buyer_currency: Currency,
    current_order_rates: Vec<(RawOrder, Option<RawOrderExchangeRate>)>,
) -> Box<Future<Item = Vec<NewOrderExchangeRate>, Error = ServiceError>> {
    Box::new(
        stream::iter_ok(
            current_order_rates
                .into_iter()
                .map(move |(order, current_rate)| (payments_client.clone(), buyer_currency.clone(), order, current_rate)),
        )
        .and_then(|(pc, buyer_currency, order, current_rate)| reserve_or_refresh_rate(pc, buyer_currency, order, current_rate))
        .filter_map(|x| x)
        .collect(),
    )
}

pub fn reserve_or_refresh_rate<PC: PaymentsClient + Send + Clone + 'static>(
    payments_client: PC,
    buyer_currency: Currency,
    order: RawOrder,
    current_rate: Option<RawOrderExchangeRate>,
) -> Box<Future<Item = Option<NewOrderExchangeRate>, Error = ServiceError>> {
    let RawOrder {
        id: order_id,
        seller_currency,
        total_amount,
        ..
    } = order;
    let fut = match current_rate {
        None => future::Either::A(get_rate(&payments_client, buyer_currency, seller_currency, total_amount).map(
            move |(exchange_id, exchange_rate)| {
                Some(NewOrderExchangeRate {
                    order_id,
                    exchange_id,
                    exchange_rate,
                })
            },
        )),
        Some(RawOrderExchangeRate { exchange_id, .. }) => future::Either::B(match exchange_id {
            None => future::Either::A(future::ok(Some(NewOrderExchangeRate {
                order_id,
                exchange_id: None,
                exchange_rate: BigDecimal::from(1),
            }))),
            Some(id) => future::Either::B(future::lazy(move || {
                payments_client
                    .refresh_rate(id.inner().clone())
                    .map_err(ectx!(convert ErrorKind::Internal => exchange_id))
                    .map(move |RateRefresh { rate, is_new_rate }| {
                        if is_new_rate {
                            let Rate {
                                id, rate: exchange_rate, ..
                            } = rate;
                            Some(NewOrderExchangeRate {
                                order_id,
                                exchange_id: Some(ExchangeId::new(id)),
                                exchange_rate,
                            })
                        } else {
                            None
                        }
                    })
            })),
        }),
    };
    Box::new(fut)
}

#[cfg(test)]
pub mod tests {

    use std::sync::Arc;
    use std::time::SystemTime;
    use tokio_core::reactor::Core;

    use stq_static_resources::Currency;
    use stq_types::*;

    use models::*;
    use repos::repo_factory::tests::*;
    use services::invoice::InvoiceService;
    use services::merchant::MerchantService;

    #[test]
    #[ignore]
    fn test_create_order_info() {
        let id = UserId(1);
        let mut core = Core::new().unwrap();
        let handle = Arc::new(core.handle());
        let service = create_service(Some(id), handle);

        let create_user = CreateUserMerchantPayload { id };
        let work = service.create_user(create_user);
        let _merchant = core.run(work).unwrap();

        let create_store = CreateStoreMerchantPayload { id: StoreId(1) };
        let work = service.create_store(create_store);
        let _store_merchant = core.run(work).unwrap();

        let order = Order {
            id: OrderId::new(),
            store_id: StoreId(1),
            price: ProductPrice(3232.32),
            quantity: Quantity(1),
            currency: Currency::STQ,
            total_amount: ProductPrice(3232.32),
            product_cashback: None,
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
    #[ignore]
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
