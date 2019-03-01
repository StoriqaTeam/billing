//! `Controller` is a top layer that handles all http-related
//! stuff like reading bodies, parsing params, forming a response.
//! Basically it provides inputs to `Service` layer and converts outputs
//! of `Service` layer to http responses

pub mod context;
pub mod requests;
pub mod responses;
pub mod routes;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use diesel::{connection::AnsiTransactionManager, pg::Pg, Connection};
use futures::{future, Future, IntoFuture};
use hyper::{header::Authorization, server::Request, Delete, Get, Method, Post, Put};
use r2d2::ManageConnection;

use stq_http::{
    client::TimeLimitedHttpClient,
    controller::{Controller, ControllerFuture},
    errors::ErrorMessageWrapper,
    request_util::{
        self, parse_body, read_body, serialize_future, RequestTimeout as RequestTimeoutHeader, Sign as TureSign,
        StripeSignature as StripeSignatureHeader,
    },
};
use stq_types::UserId;

use self::context::{DynamicContext, StaticContext};
use self::routes::Route;
use client::payments::mock::MockPaymentsClient;
use client::payments::{PaymentsClient, PaymentsClientImpl};
use controller::requests::*;
use errors::Error;
use models::order_v2::OrdersSearch;
use models::*;
use repos::repo_factory::*;
use repos::SearchFee;
use sentry_integration::log_and_capture_error;
use services::accounts::{AccountService, AccountServiceImpl};
use services::billing_info::{BillingInfoService, BillingInfoServiceImpl};
use services::billing_type::{BillingTypeService, BillingTypeServiceImpl};
use services::customer::CustomersService;
use services::customer::CustomersServiceImpl;
use services::fee::{FeesService, FeesServiceImpl};
use services::invoice::InvoiceService;
use services::merchant::MerchantService;
use services::order::OrderService;
use services::order_billing::{OrderBillingService, OrderBillingServiceImpl};
use services::payment_intent::{PaymentIntentService, PaymentIntentServiceImpl};
use services::payout::{CalculatePayoutPayload, GetPayoutsPayload, PayOutToSellerPayload, PayoutService, PayoutServiceImpl};
use services::store_subscription::{StoreSubscriptionService, StoreSubscriptionServiceImpl};
use services::stripe::{StripeService, StripeServiceImpl};
use services::subscription::{SubscriptionService, SubscriptionServiceImpl};
use services::subscription_payment::{SubscriptionPaymentService, SubscriptionPaymentServiceImpl};
use services::user_roles::UserRolesService;
use services::Service;

/// Controller handles route parsing and calling `Service` layer
pub struct ControllerImpl<T, M, F>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
{
    pub static_context: StaticContext<T, M, F>,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > ControllerImpl<T, M, F>
{
    /// Create a new controller based on services
    pub fn new(static_context: StaticContext<T, M, F>) -> Self {
        Self { static_context }
    }
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > Controller for ControllerImpl<T, M, F>
{
    /// Handle a request and get future response
    fn call(&self, req: Request) -> ControllerFuture {
        let user_id = get_user_id(&req);
        let correlation_token = request_util::get_correlation_token(&req);

        let request_timeout = req
            .headers()
            .get::<RequestTimeoutHeader>()
            .and_then(|h| h.0.parse::<u64>().ok())
            .unwrap_or(self.static_context.config.client.http_timeout_ms)
            .checked_sub(self.static_context.config.server.processing_timeout_ms as u64)
            .map(Duration::from_millis)
            .unwrap_or(Duration::new(0, 0));

        let time_limited_http_client = TimeLimitedHttpClient::new(self.static_context.client_handle.clone(), request_timeout);

        let payments_mock_cfg = &self.static_context.config.payments_mock;
        let (payments_client, account_service) = match (payments_mock_cfg.use_mock, self.static_context.config.payments.clone()) {
            (true, _) => {
                let payments_client = MockPaymentsClient::default();
                let account_service = AccountServiceImpl::new(
                    self.static_context.db_pool.clone(),
                    self.static_context.cpu_pool.clone(),
                    self.static_context.repo_factory.clone(),
                    payments_mock_cfg.min_pooled_accounts,
                    payments_client.clone(),
                    format!(
                        "{}{}",
                        self.static_context.config.callback.url.clone(),
                        routes::PAYMENTS_CALLBACK_ENDPOINT
                    ),
                    payments_mock_cfg.clone().accounts.into(),
                );

                let payments_client = Arc::new(payments_client) as Arc<dyn PaymentsClient>;
                let account_service = Arc::new(account_service) as Arc<dyn AccountService + Send + Sync>;

                (Some(payments_client), Some(account_service))
            }
            (_, None) => (None, None),
            (_, Some(payments_config)) => {
                PaymentsClientImpl::create_from_config(time_limited_http_client.clone(), payments_config.clone().into())
                    .ok()
                    .map(|payments_client| {
                        let account_service = AccountServiceImpl::new(
                            self.static_context.db_pool.clone(),
                            self.static_context.cpu_pool.clone(),
                            self.static_context.repo_factory.clone(),
                            payments_config.min_pooled_accounts,
                            payments_client.clone(),
                            format!(
                                "{}{}",
                                self.static_context.config.callback.url.clone(),
                                routes::PAYMENTS_CALLBACK_ENDPOINT
                            ),
                            payments_config.accounts.into(),
                        );

                        let payments_client = Arc::new(payments_client) as Arc<dyn PaymentsClient>;
                        let account_service = Arc::new(account_service) as Arc<dyn AccountService + Send + Sync>;

                        (Some(payments_client), Some(account_service))
                    })
                    .unwrap_or((None, None))
            }
        };

        let dynamic_context = DynamicContext::new(
            user_id,
            correlation_token,
            time_limited_http_client,
            payments_client.clone(),
            account_service,
        );

        let service = Service::new(self.static_context.clone(), dynamic_context.clone());

        let customer_service = Arc::new(CustomersServiceImpl {
            db_pool: self.static_context.db_pool.clone(),
            cpu_pool: self.static_context.cpu_pool.clone(),
            repo_factory: self.static_context.repo_factory.clone(),
            stripe_client: self.static_context.stripe_client.clone(),
            dynamic_context: dynamic_context.clone(),
        });

        let order_billing_service = Arc::new(OrderBillingServiceImpl {
            db_pool: self.static_context.db_pool.clone(),
            cpu_pool: self.static_context.cpu_pool.clone(),
            repo_factory: self.static_context.repo_factory.clone(),
            dynamic_context: dynamic_context.clone(),
        });

        let billing_info_service = Arc::new(BillingInfoServiceImpl {
            db_pool: self.static_context.db_pool.clone(),
            cpu_pool: self.static_context.cpu_pool.clone(),
            repo_factory: self.static_context.repo_factory.clone(),
            dynamic_context: dynamic_context.clone(),
        });

        let fees_service = Arc::new(FeesServiceImpl {
            db_pool: self.static_context.db_pool.clone(),
            cpu_pool: self.static_context.cpu_pool.clone(),
            repo_factory: self.static_context.repo_factory.clone(),
            stripe_client: self.static_context.stripe_client.clone(),
            dynamic_context: dynamic_context.clone(),
        });

        let billing_type_service = Arc::new(BillingTypeServiceImpl {
            db_pool: self.static_context.db_pool.clone(),
            cpu_pool: self.static_context.cpu_pool.clone(),
            repo_factory: self.static_context.repo_factory.clone(),
            dynamic_context: dynamic_context.clone(),
        });

        let payment_intent_service = Arc::new(PaymentIntentServiceImpl {
            db_pool: self.static_context.db_pool.clone(),
            cpu_pool: self.static_context.cpu_pool.clone(),
            repo_factory: self.static_context.repo_factory.clone(),
            dynamic_context: dynamic_context.clone(),
            stripe_client: self.static_context.stripe_client.clone(),
        });

        let stripe_service = Arc::new(StripeServiceImpl {
            db_pool: self.static_context.db_pool.clone(),
            cpu_pool: self.static_context.cpu_pool.clone(),
            repo_factory: self.static_context.repo_factory.clone(),
            stripe_client: self.static_context.stripe_client.clone(),
            dynamic_context: dynamic_context.clone(),
            static_context: self.static_context.clone(),
        });

        let payout_service = Arc::new(PayoutServiceImpl {
            db_pool: self.static_context.db_pool.clone(),
            cpu_pool: self.static_context.cpu_pool.clone(),
            repo_factory: self.static_context.repo_factory.clone(),
            user_id: dynamic_context.user_id.clone(),
            payments_client: payments_client.clone(),
        });

        let subscription_service = Arc::new(SubscriptionServiceImpl {
            db_pool: self.static_context.db_pool.clone(),
            cpu_pool: self.static_context.cpu_pool.clone(),
            repo_factory: self.static_context.repo_factory.clone(),
            dynamic_context: dynamic_context.clone(),
            config: self.static_context.config.subscription.clone(),
        });

        let subscription_payment_service = Arc::new(SubscriptionPaymentServiceImpl {
            db_pool: self.static_context.db_pool.clone(),
            cpu_pool: self.static_context.cpu_pool.clone(),
            repo_factory: self.static_context.repo_factory.clone(),
            dynamic_context: dynamic_context.clone(),
            stripe_client: self.static_context.stripe_client.clone(),
            config: self.static_context.config.subscription.clone(),
        });

        let store_subscription_service = Arc::new(StoreSubscriptionServiceImpl {
            db_pool: self.static_context.db_pool.clone(),
            cpu_pool: self.static_context.cpu_pool.clone(),
            repo_factory: self.static_context.repo_factory.clone(),
            dynamic_context: dynamic_context.clone(),
        });

        let path = req.path().to_string();

        let fut = match (&req.method().clone(), self.static_context.route_parser.test(req.path())) {
            (&Post, Some(Route::StripeWebhook)) => serialize_future(
                req.headers()
                    .get::<StripeSignatureHeader>()
                    .cloned()
                    .ok_or(format_err!("Stripe-Signature header not provided"))
                    .into_future()
                    .and_then(|signature_header| {
                        info!("stripe controller signature_header: {}", signature_header);
                        read_body(req.body())
                            .map(move |data| (signature_header, data))
                            .map_err(failure::Error::from)
                    })
                    .and_then(move |(signature_header, data)| {
                        stripe_service
                            .handle_stripe_event(signature_header, data)
                            .map_err(Error::from)
                            .map_err(failure::Error::from)
                    }),
            ),
            (&Post, Some(Route::ExternalBillingCallback)) => {
                serialize_future({ parse_body::<ExternalBillingInvoice>(req.body()).and_then(move |data| service.update_invoice(data)) })
            }
            (&Post, Some(Route::PaymentsInboundTx)) => serialize_future(
                req.headers()
                    .get::<TureSign>()
                    .cloned()
                    .ok_or(format_err!("Sign header not provided"))
                    .into_future()
                    .and_then(|signature_header| {
                        read_body(req.body()).map_err(failure::Error::from).and_then(|body| {
                            serde_json::from_str(&body)
                                .map(|data| (signature_header, data, body))
                                .map_err(failure::Error::from)
                        })
                    })
                    .and_then(move |(signature_header, data, body)| {
                        service
                            .handle_inbound_tx(signature_header, data, body)
                            .map_err(Error::from)
                            .map_err(failure::Error::from)
                    }),
            ),
            (&Post, Some(Route::UserMerchants)) => {
                serialize_future({ parse_body::<CreateUserMerchantPayload>(req.body()).and_then(move |data| service.create_user(data)) })
            }
            (Delete, Some(Route::UserMerchant { user_id })) => serialize_future({ service.delete_user(user_id) }),
            (Get, Some(Route::UserMerchantBalance { user_id })) => serialize_future({ service.get_user_balance(user_id) }),
            (&Post, Some(Route::StoreMerchants)) => {
                serialize_future({ parse_body::<CreateStoreMerchantPayload>(req.body()).and_then(move |data| service.create_store(data)) })
            }
            (Delete, Some(Route::StoreMerchant { store_id })) => serialize_future({ service.delete_store(store_id) }),
            (Get, Some(Route::StoreMerchantBalance { store_id })) => serialize_future({ service.get_store_balance(store_id) }),
            (&Post, Some(Route::Invoices)) => {
                serialize_future({ parse_body::<CreateInvoice>(req.body()).and_then(move |data| service.create_invoice(data)) })
            }
            (&Post, Some(Route::InvoicesV2)) => serialize_future(
                parse_body::<CreateInvoiceV2>(req.body())
                    .and_then(move |data| service.create_invoice_v2(data).map_err(Error::from).map_err(failure::Error::from)),
            ),
            (Delete, Some(Route::InvoiceBySagaId { id })) => serialize_future({ service.delete_invoice_by_saga_id(id) }),
            (Get, Some(Route::InvoiceByOrderId { id })) => serialize_future({ service.get_invoice_by_order_id(id) }),
            (Get, Some(Route::InvoiceById { id })) => serialize_future({ service.get_invoice_by_id(id) }),
            (Get, Some(Route::InvoiceByIdV2 { id })) => {
                serialize_future(service.recalc_invoice_v2(id).map_err(Error::from).map_err(failure::Error::from))
            }
            (Post, Some(Route::InvoiceByIdRecalc { id })) => serialize_future({ service.recalc_invoice(id) }),
            (Get, Some(Route::InvoiceOrdersIds { id })) => serialize_future({ service.get_invoice_orders_ids(id) }),
            (Get, Some(Route::RolesByUserId { user_id })) => serialize_future({ service.get_roles(user_id) }),
            (Post, Some(Route::Roles)) => {
                serialize_future({ parse_body::<NewUserRole>(req.body()).and_then(move |data| service.create_user_role(data)) })
            }
            (Delete, Some(Route::Roles)) => {
                serialize_future({ parse_body::<RemoveUserRole>(req.body()).and_then(move |data| service.delete_user_role(data)) })
            }
            (Delete, Some(Route::RolesByUserId { user_id })) => serialize_future({ service.delete_user_role_by_user_id(user_id) }),
            (Delete, Some(Route::RoleById { id })) => serialize_future({ service.delete_user_role_by_id(id) }),

            (Get, Some(Route::PaymentIntentByInvoice { invoice_id })) => {
                serialize_future({ payment_intent_service.get_by_invoice(invoice_id) })
            }
            (Post, Some(Route::PaymentIntentByFee { fee_id })) => serialize_future({ payment_intent_service.create_by_fee(fee_id) }),
            (Post, Some(Route::OrdersByIdCapture { id })) => serialize_future({ service.order_capture(id) }),
            (Post, Some(Route::OrdersByIdDecline { id })) => serialize_future({ service.order_decline(id) }),

            (Post, Some(Route::OrdersSetPaymentState { order_id })) => serialize_future({
                parse_body::<OrderPaymentStateRequest>(req.body())
                    .map_err(failure::Error::from)
                    .and_then(move |payload| service.update_order_state(order_id, payload.state).map_err(failure::Error::from))
            }),

            (Post, Some(Route::CustomersWithSource)) => serialize_future({
                parse_body::<NewCustomerWithSourceRequest>(req.body())
                    .and_then(move |data| customer_service.create_customer_with_source(data).map_err(failure::Error::from))
            }),
            (Get, Some(Route::Customers)) => serialize_future({ customer_service.get_customer() }),
            (Delete, Some(Route::Customers)) => serialize_future({
                parse_body::<DeleteCustomerRequest>(req.body())
                    .and_then(move |payload| customer_service.delete(payload.customer_id).map_err(failure::Error::from))
            }),
            (Put, Some(Route::Customers)) => serialize_future({
                parse_body::<UpdateCustomerRequest>(req.body())
                    .and_then(move |payload| customer_service.update(payload).map_err(failure::Error::from))
            }),
            (Post, Some(Route::OrderBillingInfo)) => {
                let (skip_opt, count_opt) = parse_query!(
                    req.query().unwrap_or_default(),
                    "skip" => i64, "count" => i64
                );

                let skip = skip_opt.unwrap_or(0);
                let count = count_opt.unwrap_or(0);

                serialize_future(parse_body::<OrderBillingSearchTerms>(req.body()).and_then(move |payload| {
                    order_billing_service
                        .search(skip, count, payload)
                        .map_err(Error::from)
                        .map_err(failure::Error::from)
                }))
            }
            (Post, Some(Route::OrderSearch)) => {
                let (skip_opt, count_opt) = parse_query!(
                    req.query().unwrap_or_default(),
                    "skip" => i64, "count" => i64
                );

                let skip = skip_opt.unwrap_or(0);
                let count = count_opt.unwrap_or(0);

                serialize_future(parse_body::<OrdersSearch>(req.body()).and_then(move |payload| {
                    service
                        .search_orders(skip, count, payload)
                        .map_err(Error::from)
                        .map_err(failure::Error::from)
                }))
            }

            (Post, Some(Route::InternationalBillingInfos)) => serialize_future({
                parse_body::<NewInternationalBillingInfo>(req.body()).and_then(move |payload| {
                    billing_info_service
                        .create_international_billing_info(payload)
                        .map_err(failure::Error::from)
                })
            }),

            (Put, Some(Route::InternationalBillingInfo { id })) => serialize_future({
                parse_body::<UpdateInternationalBillingInfo>(req.body()).and_then(move |payload| {
                    billing_info_service
                        .update_international_billing_info(id, payload)
                        .map_err(failure::Error::from)
                })
            }),
            (Post, Some(Route::RussiaBillingInfos)) => serialize_future({
                parse_body::<NewRussiaBillingInfo>(req.body()).and_then(move |payload| {
                    billing_info_service
                        .create_russia_billing_info(payload)
                        .map_err(failure::Error::from)
                })
            }),
            (Put, Some(Route::RussiaBillingInfo { id })) => serialize_future({
                parse_body::<UpdateRussiaBillingInfo>(req.body()).and_then(move |payload| {
                    billing_info_service
                        .update_russia_billing_info(id, payload)
                        .map_err(failure::Error::from)
                })
            }),

            (Get, Some(Route::FeesByOrder { id })) => serialize_future({ fees_service.get_by_order_id(id).map_err(failure::Error::from) }),
            (Post, Some(Route::FeesPay { id })) => serialize_future({ fees_service.create_charge(SearchFee::Id(id)) }),
            (Post, Some(Route::FeesPayByOrder { id })) => serialize_future({ fees_service.create_charge(SearchFee::OrderId(id)) }),
            (Post, Some(Route::FeesPayByOrders)) => serialize_future({
                parse_body::<FeesPayByOrdersRequest>(req.body())
                    .and_then(move |payload| fees_service.create_charge_for_several_fees(payload).map_err(failure::Error::from))
            }),
            (Get, Some(Route::RussiaBillingInfoByStore { id })) => serialize_future({
                billing_info_service
                    .get_russia_billing_info_by_store(id)
                    .map_err(failure::Error::from)
            }),
            (Get, Some(Route::InternationalBillingInfoByStore { id })) => serialize_future({
                billing_info_service
                    .get_international_billing_info_by_store(id)
                    .map_err(failure::Error::from)
            }),
            (Get, Some(Route::BillingTypeByStore { id })) => {
                serialize_future({ billing_type_service.get_billing_type_by_store(id).map_err(failure::Error::from) })
            }
            (Post, Some(Route::Payouts)) => serialize_future({
                parse_body::<PayOutToSellerPayload>(req.body()).and_then(move |payload| {
                    payout_service
                        .pay_out_to_seller(payload)
                        .map_err(Error::from)
                        .map_err(failure::Error::from)
                })
            }),
            (Get, Some(Route::PayoutsByStoreId { id })) => serialize_future(
                payout_service
                    .get_payouts_by_store_id(id)
                    .map_err(Error::from)
                    .map_err(failure::Error::from),
            ),
            (Get, Some(Route::PayoutById { id })) => {
                serialize_future(payout_service.get_payout(id).map_err(Error::from).map_err(failure::Error::from))
            }
            (Post, Some(Route::PayoutsByOrderIds)) => serialize_future({
                parse_body::<GetPayoutsPayload>(req.body()).and_then(move |payload| {
                    payout_service
                        .get_payouts_by_order_ids(payload)
                        .map_err(Error::from)
                        .map_err(failure::Error::from)
                })
            }),
            (Get, Some(Route::StoreBalance { store_id })) => serialize_future(
                payout_service
                    .get_balance(store_id)
                    .map_err(Error::from)
                    .map_err(failure::Error::from),
            ),
            (Post, Some(Route::PayoutsCalculate)) => serialize_future({
                parse_body::<CalculatePayoutPayload>(req.body()).and_then(move |payload| {
                    payout_service
                        .calculate_payout(payload)
                        .map_err(Error::from)
                        .map_err(failure::Error::from)
                })
            }),
            (Post, Some(Route::Subscriptions)) => serialize_future({
                parse_body::<CreateSubscriptionsRequest>(req.body()).and_then(move |payload| {
                    subscription_service
                        .create_all(payload)
                        .map_err(Error::from)
                        .map_err(failure::Error::from)
                })
            }),
            (Post, Some(Route::SubscriptionBySubscriptionPaymentId { id })) => serialize_future(
                subscription_service
                    .get_by_subscription_payment_id(id)
                    .map_err(Error::from)
                    .map_err(failure::Error::from),
            ),
            (Post, Some(Route::SubscriptionPayment)) => serialize_future(
                subscription_payment_service
                    .pay_subscriptions()
                    .map_err(Error::from)
                    .map_err(failure::Error::from),
            ),
            (Post, Some(Route::SubscriptionPaymentSearch)) => {
                let (skip_opt, count_opt) = parse_query!(
                    req.query().unwrap_or_default(),
                    "skip" => i64, "count" => i64
                );

                let skip = skip_opt.unwrap_or(0);
                let count = count_opt.unwrap_or(0);

                serialize_future(parse_body::<SubscriptionPaymentSearch>(req.body()).and_then(move |payload| {
                    subscription_payment_service
                        .search(skip, count, payload)
                        .map_err(Error::from)
                        .map_err(failure::Error::from)
                }))
            }

            (Post, Some(Route::StoreSubscriptionByStoreId { store_id })) => {
                serialize_future(parse_body::<CreateStoreSubscriptionRequest>(req.body()).and_then(move |payload| {
                    store_subscription_service
                        .create(store_id, payload)
                        .map_err(Error::from)
                        .map_err(failure::Error::from)
                }))
            }
            (Get, Some(Route::StoreSubscriptionByStoreId { store_id })) => {
                serialize_future({ store_subscription_service.get(store_id).map_err(failure::Error::from) })
            }
            (Put, Some(Route::StoreSubscriptionByStoreId { store_id })) => {
                serialize_future(parse_body::<UpdateStoreSubscriptionRequest>(req.body()).and_then(move |payload| {
                    store_subscription_service
                        .update(store_id, payload)
                        .map_err(Error::from)
                        .map_err(failure::Error::from)
                }))
            }

            // Fallback
            (m, _) => not_found(m, path),
        }
        .map_err(|err| {
            let wrapper = ErrorMessageWrapper::<Error>::from(&err);
            if wrapper.inner.code == 500 {
                log_and_capture_error(&err);
            }
            err
        });

        Box::new(fut)
    }
}

fn not_found(method: &Method, path: String) -> Box<Future<Item = String, Error = failure::Error>> {
    Box::new(future::err(
        format_err!("Request to non existing endpoint in billing microservice! {:?} {:?}", method, path)
            .context(Error::NotFound)
            .into(),
    ))
}

fn get_user_id(req: &Request) -> Option<UserId> {
    req.headers()
        .get::<Authorization<String>>()
        .map(|auth| auth.0.clone())
        .and_then(|id| i32::from_str(&id).ok())
        .map(UserId)
}
