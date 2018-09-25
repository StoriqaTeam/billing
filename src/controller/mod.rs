//! `Controller` is a top layer that handles all http-related
//! stuff like reading bodies, parsing params, forming a response.
//! Basically it provides inputs to `Service` layer and converts outputs
//! of `Service` layer to http responses

pub mod context;
pub mod routes;

use std::str::FromStr;

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures::future;
use futures::Future;
use hyper::header::Authorization;
use hyper::server::Request;
use hyper::{Delete, Get, Post};
use r2d2::ManageConnection;

use stq_http::controller::Controller;
use stq_http::controller::ControllerFuture;
use stq_http::request_util::parse_body;
use stq_http::request_util::serialize_future;
use stq_types::UserId;

use self::context::Context;
use self::routes::Route;
use errors::Error;
use models::*;
use repos::repo_factory::*;
use services::invoice::{InvoiceService, InvoiceServiceImpl};
use services::merchant::{MerchantService, MerchantServiceImpl};
use services::user_roles::{UserRolesService, UserRolesServiceImpl};

/// Controller handles route parsing and calling `Service` layer
pub struct ControllerImpl<T, M, F>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
{
    pub context: Context<T, M, F>,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > ControllerImpl<T, M, F>
{
    /// Create a new controller based on services
    pub fn new(context: Context<T, M, F>) -> Self {
        Self { context }
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
        let headers = req.headers().clone();
        let auth_header = headers.get::<Authorization<String>>();
        let user_id = auth_header
            .map(move |auth| auth.0.clone())
            .and_then(|id| i32::from_str(&id).ok())
            .map(UserId);

        let invoice_service = InvoiceServiceImpl::new(self.context.clone(), user_id);
        let merchant_service = MerchantServiceImpl::new(self.context.clone(), user_id);
        let user_roles_service = UserRolesServiceImpl::new(self.context.clone(), user_id);

        let path = req.path().to_string();

        match (&req.method().clone(), self.context.route_parser.test(req.path())) {
            (&Post, Some(Route::ExternalBillingCallback)) => serialize_future({
                parse_body::<ExternalBillingInvoice>(req.body()).and_then(move |data| {
                    debug!("Received request to update invoice {:?}", data);
                    invoice_service.update(data)
                })
            }),
            (&Post, Some(Route::UserMerchants)) => serialize_future({
                parse_body::<CreateUserMerchantPayload>(req.body()).and_then(move |data| {
                    debug!("Received request to create user merchant {:?}", data);
                    merchant_service.create_user(data)
                })
            }),
            (Delete, Some(Route::UserMerchant { user_id })) => {
                debug!("Received request to delete merchant by user id {}", user_id);
                serialize_future({ merchant_service.delete_user(user_id) })
            }
            (Get, Some(Route::UserMerchantBalance { user_id })) => {
                debug!("Received request to get merchant balance by user id {}", user_id);
                serialize_future({ merchant_service.get_user_balance(user_id) })
            }
            (&Post, Some(Route::StoreMerchants)) => serialize_future({
                parse_body::<CreateStoreMerchantPayload>(req.body()).and_then(move |data| {
                    debug!("Received request to create store merchant {:?}", data);
                    merchant_service.create_store(data)
                })
            }),
            (Delete, Some(Route::StoreMerchant { store_id })) => {
                debug!("Received request to delete merchant by store id {}", store_id);
                serialize_future({ merchant_service.delete_store(store_id) })
            }
            (Get, Some(Route::StoreMerchantBalance { store_id })) => {
                debug!("Received request to get merchant balance by store id {}", store_id);
                serialize_future({ merchant_service.get_store_balance(store_id) })
            }
            (&Post, Some(Route::Invoices)) => serialize_future({
                parse_body::<CreateInvoice>(req.body()).and_then(move |data| {
                    debug!("Received request to create invoice {}", data);
                    invoice_service.create(data)
                })
            }),
            (Delete, Some(Route::InvoiceBySagaId { id })) => {
                debug!("Received request to delete invoice by saga id {}", id);
                serialize_future({ invoice_service.delete(id) })
            }
            (Get, Some(Route::InvoiceByOrderId { id })) => {
                debug!("Received request to get invoice by order id {}", id);
                serialize_future({ invoice_service.get_by_order_id(id) })
            }
            (Get, Some(Route::InvoiceById { id })) => {
                debug!("Received request to get invoice by id {}", id);
                serialize_future({ invoice_service.get_by_id(id) })
            }
            (Post, Some(Route::InvoiceByIdRecalc { id })) => {
                debug!("Received request to recalc invoice by id {}", id);
                serialize_future({ invoice_service.recalc(id) })
            }
            (Get, Some(Route::InvoiceOrdersIds { id })) => {
                debug!("Received request to get invoice orders ids by id {}", id);
                serialize_future({ invoice_service.get_orders_ids(id) })
            }
            (Get, Some(Route::RolesByUserId { user_id })) => {
                debug!("Received request to get roles by user id {}", user_id);
                serialize_future({ user_roles_service.get_roles(user_id) })
            }
            (Post, Some(Route::Roles)) => serialize_future({
                parse_body::<NewUserRole>(req.body()).and_then(move |data| {
                    debug!("Received request to create role {:?}", data);
                    user_roles_service.create(data)
                })
            }),
            (Delete, Some(Route::RolesByUserId { user_id })) => {
                debug!("Received request to delete role by user id {}", user_id);
                serialize_future({ user_roles_service.delete_by_user_id(user_id) })
            }
            (Delete, Some(Route::RoleById { id })) => {
                debug!("Received request to delete role by id {}", id);
                serialize_future({ user_roles_service.delete_by_id(id) })
            }

            // Fallback
            (m, _) => Box::new(future::err(
                format_err!("Request to non existing endpoint in billing microservice! {:?} {:?}", m, path)
                    .context(Error::NotFound)
                    .into(),
            )),
        }
    }
}
