//! `Controller` is a top layer that handles all http-related
//! stuff like reading bodies, parsing params, forming a response.
//! Basically it provides inputs to `Service` layer and converts outputs
//! of `Service` layer to http responses

pub mod context;
pub mod routes;

use std::str::FromStr;
use std::time::Duration;

use diesel::{connection::AnsiTransactionManager, pg::Pg, Connection};
use futures::{future, Future};
use hyper::{header::Authorization, server::Request, Delete, Get, Method, Post};
use r2d2::ManageConnection;

use stq_http::{
    client::TimeLimitedHttpClient,
    controller::{Controller, ControllerFuture},
    errors::ErrorMessageWrapper,
    request_util::{self, parse_body, serialize_future, RequestTimeout as RequestTimeoutHeader},
};
use stq_types::UserId;

use self::context::{DynamicContext, StaticContext};
use self::routes::Route;
use client::payments::PaymentsClientImpl;
use errors::Error;
use models::*;
use repos::repo_factory::*;
use sentry_integration::log_and_capture_error;
use services::accounts::AccountServiceImpl;
use services::invoice::InvoiceService;
use services::merchant::MerchantService;
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

        let (payments_client, account_service) = match self.static_context.config.payments.clone() {
            None => (None, None),
            Some(payments_config) => {
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
                        (Some(payments_client), Some(account_service))
                    })
                    .unwrap_or((None, None))
            }
        };

        let dynamic_context = DynamicContext::new(
            user_id,
            correlation_token,
            time_limited_http_client,
            payments_client,
            account_service,
        );

        let service = Service::new(self.static_context.clone(), dynamic_context);

        let path = req.path().to_string();

        let fut = match (&req.method().clone(), self.static_context.route_parser.test(req.path())) {
            (&Post, Some(Route::ExternalBillingCallback)) => {
                serialize_future({ parse_body::<ExternalBillingInvoice>(req.body()).and_then(move |data| service.update_invoice(data)) })
            }
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
            (Delete, Some(Route::RolesByUserId { user_id })) => serialize_future({ service.delete_user_role_by_user_id(user_id) }),
            (Delete, Some(Route::RoleById { id })) => serialize_future({ service.delete_user_role_by_id(id) }),

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
