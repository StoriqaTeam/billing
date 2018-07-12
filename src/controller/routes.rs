use stq_router::RouteParser;
use stq_types::{CallbackId, RoleId, SagaId, StoreId, UserId};

/// List of all routes with params for the app
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    ExternalBillingCallback { id: CallbackId },
    Invoices,
    Invoice { id: SagaId },
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

    route_parser.add_route_with_params(r"^/external_billing_callback/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::ExternalBillingCallback { id })
    });

    route_parser.add_route(r"^/invoices", || Route::Invoices);
    route_parser.add_route_with_params(r"^/invoices/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::Invoice { id })
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
