use models::*;

use stq_router::RouteParser;

/// List of all routes with params for the app
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    ExternalBillingCallback { id: CallbackId },
    OrderInfo,
    UserMerchant,
    StoreMerchant,
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

    route_parser.add_route(r"^/order_info$", || Route::OrderInfo);
    route_parser.add_route(r"^/merchants/user$", || Route::UserMerchant);
    route_parser.add_route(r"^/merchants/store$", || Route::StoreMerchant);

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
