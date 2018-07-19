use stq_router::RouteParser;
use stq_types::{InvoiceId, OrderId, RoleId, SagaId, StoreId, UserId};

/// List of all routes with params for the app
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    ExternalBillingCallback,
    Invoices,
    InvoiceBySagaId { id: SagaId },
    InvoiceById { id: InvoiceId },
    InvoiceByOrderId { id: OrderId },
    InvoiceOrdersIds { id: InvoiceId },
    UserMerchants,
    StoreMerchants,
    UserMerchant { user_id: UserId },
    StoreMerchant { store_id: StoreId },
    Roles,
    RoleById { id: RoleId },
    RolesByUserId { user_id: UserId },
}

pub fn create_route_parser() -> RouteParser<Route> {
    let mut route_parser = RouteParser::default();

    route_parser.add_route(r"^/external_billing_callback$", || Route::ExternalBillingCallback);

    route_parser.add_route(r"^/invoices$", || Route::Invoices);
    route_parser.add_route_with_params(r"^/invoices/by-saga-id/(\S+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::InvoiceBySagaId { id })
    });
    route_parser.add_route_with_params(r"^/invoices/by-id/(\S+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::InvoiceById { id })
    });
    route_parser.add_route_with_params(r"^/invoices/by-order-id/(\S+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::InvoiceByOrderId { id })
    });
    route_parser.add_route_with_params(r"^/invoices/by-id/(\S+)/order_ids$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::InvoiceOrdersIds { id })
    });
    route_parser.add_route(r"^/merchants/user$", || Route::UserMerchants);
    route_parser.add_route_with_params(r"^/merchants/user/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|user_id| Route::UserMerchant { user_id })
    });
    route_parser.add_route(r"^/merchants/store$", || Route::StoreMerchants);
    route_parser.add_route_with_params(r"^/merchants/store/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|store_id| Route::StoreMerchant { store_id })
    });

    route_parser.add_route(r"^/roles$", || Route::Roles);
    route_parser.add_route_with_params(r"^/roles/by-user-id/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|user_id| Route::RolesByUserId { user_id })
    });
    route_parser.add_route_with_params(r"^/roles/by-id/(\S+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::RoleById { id })
    });

    route_parser
}
