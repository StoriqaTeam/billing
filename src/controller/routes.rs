use stq_router::RouteParser;
use stq_types::{InternationalBillingId, InvoiceId, OrderId, RoleId, RussiaBillingId, SagaId, StoreId, UserId};

use models::invoice_v2;
use models::order_v2::{OrderId as Orderv2Id, StoreId as BillingStoreId};
use models::{FeeId, PayoutId};

pub const PAYMENTS_CALLBACK_ENDPOINT: &'static str = "/v2/callback/payments/inbound_tx";

/// List of all routes with params for the app
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    StripeWebhook,
    ExternalBillingCallback,
    PaymentsInboundTx,
    Invoices,
    InvoicesV2,
    InvoiceBySagaId { id: SagaId },
    InvoiceById { id: InvoiceId },
    InvoiceByIdV2 { id: invoice_v2::InvoiceId },
    InvoiceByOrderId { id: OrderId },
    InvoiceOrdersIds { id: InvoiceId },
    InvoiceByIdRecalc { id: InvoiceId },
    OrdersByIdCapture { id: Orderv2Id },
    OrdersByIdDecline { id: Orderv2Id },
    UserMerchants,
    StoreMerchants,
    UserMerchant { user_id: UserId },
    UserMerchantBalance { user_id: UserId },
    StoreMerchant { store_id: StoreId },
    StoreMerchantBalance { store_id: StoreId },
    Roles,
    RoleById { id: RoleId },
    RolesByUserId { user_id: UserId },
    PaymentIntentByInvoice { invoice_id: invoice_v2::InvoiceId },
    PaymentIntentByFee { fee_id: FeeId },
    Customers,
    CustomersWithSource,
    OrdersSetPaymentState { order_id: Orderv2Id },
    OrderSearch,
    OrderBillingInfo,
    InternationalBillingInfos,
    RussiaBillingInfos,
    InternationalBillingInfo { id: InternationalBillingId },
    RussiaBillingInfo { id: RussiaBillingId },
    InternationalBillingInfoByStore { id: StoreId },
    RussiaBillingInfoByStore { id: StoreId },
    BillingTypeByStore { id: StoreId },
    FeesByOrder { id: Orderv2Id },
    FeesPay { id: FeeId },
    FeesPayByOrder { id: Orderv2Id },
    FeesPayByOrders,
    Payouts,
    PayoutById { id: PayoutId },
    PayoutsByOrderIds,
    PayoutsByStoreId { id: BillingStoreId },
    PayoutsCalculate,
    Subscriptions,
}

pub fn create_route_parser() -> RouteParser<Route> {
    let mut route_parser = RouteParser::default();
    route_parser.add_route(r"^/v2/callback/stripe$", || Route::StripeWebhook);
    route_parser.add_route(r"^/external_billing_callback$", || Route::ExternalBillingCallback);
    route_parser.add_route(&format!(r"^{}$", PAYMENTS_CALLBACK_ENDPOINT), || Route::PaymentsInboundTx);
    route_parser.add_route(r"^/invoices$", || Route::Invoices);
    route_parser.add_route(r"^/v2/invoices$", || Route::InvoicesV2);
    route_parser.add_route_with_params(r"^/invoices/by-saga-id/([a-zA-Z0-9-]+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::InvoiceBySagaId { id })
    });
    route_parser.add_route_with_params(r"^/invoices/by-id/([a-zA-Z0-9-]+)/recalc$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::InvoiceByIdRecalc { id })
    });
    route_parser.add_route_with_params(r"^/invoices/by-id/([a-zA-Z0-9-]+)/order_ids$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::InvoiceOrdersIds { id })
    });
    route_parser.add_route_with_params(r"^/invoices/by-id/([a-zA-Z0-9-]+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::InvoiceById { id })
    });
    route_parser.add_route_with_params(r"^/v2/invoices/by-id/([a-zA-Z0-9-]+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::InvoiceByIdV2 { id })
    });
    route_parser.add_route_with_params(r"^/invoices/by-order-id/([a-zA-Z0-9-]+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::InvoiceByOrderId { id })
    });
    route_parser.add_route(r"^/merchants/user$", || Route::UserMerchants);
    route_parser.add_route_with_params(r"^/merchants/user/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|user_id| Route::UserMerchant { user_id })
    });
    route_parser.add_route_with_params(r"^/merchants/user/(\d+)/balance$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|user_id| Route::UserMerchantBalance { user_id })
    });
    route_parser.add_route(r"^/merchants/store$", || Route::StoreMerchants);
    route_parser.add_route_with_params(r"^/merchants/store/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|store_id| Route::StoreMerchant { store_id })
    });
    route_parser.add_route_with_params(r"^/merchants/store/(\d+)/balance$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|store_id| Route::StoreMerchantBalance { store_id })
    });

    route_parser.add_route(r"^/roles$", || Route::Roles);
    route_parser.add_route_with_params(r"^/roles/by-user-id/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|user_id| Route::RolesByUserId { user_id })
    });
    route_parser.add_route_with_params(r"^/roles/by-id/([a-zA-Z0-9-]+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::RoleById { id })
    });

    route_parser.add_route_with_params(r"^/payment_intents/invoices/([a-zA-Z0-9-]+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|invoice_id| Route::PaymentIntentByInvoice { invoice_id })
    });

    route_parser.add_route_with_params(r"^/payment_intents/fees/([a-zA-Z0-9-]+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|fee_id| Route::PaymentIntentByFee { fee_id })
    });

    route_parser.add_route_with_params(r"^/orders/([a-zA-Z0-9-]+)/capture$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::OrdersByIdCapture { id })
    });

    route_parser.add_route_with_params(r"^/orders/([a-zA-Z0-9-]+)/decline$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::OrdersByIdDecline { id })
    });

    route_parser.add_route_with_params(r"^/orders/([a-zA-Z0-9-]+)/set_payment_state$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|order_id| Route::OrdersSetPaymentState { order_id })
    });

    route_parser.add_route(r"^/orders/search$", || Route::OrderSearch);

    route_parser.add_route(r"^/customers$", || Route::Customers);

    route_parser.add_route_with_params(r"^/fees/by-order-id/([a-zA-Z0-9-]+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::FeesByOrder { id })
    });

    route_parser.add_route_with_params(r"^/fees/(d+)/pay$", |params| {
        params.get(0).and_then(|id| id.parse().ok()).map(|id| Route::FeesPay { id })
    });

    route_parser.add_route_with_params(r"^/fees/by-order-id/([a-zA-Z0-9-]+)/pay$", |params| {
        params.get(0).and_then(|id| id.parse().ok()).map(|id| Route::FeesPayByOrder { id })
    });

    route_parser.add_route(r"^fees/by-order-ids/pay$", || Route::FeesPayByOrders);

    route_parser.add_route(r"^/customers/with_source$", || Route::CustomersWithSource);
    route_parser.add_route(r"^/order_billing_info$", || Route::OrderBillingInfo);
    route_parser.add_route(r"^/billing_info/international$", || Route::InternationalBillingInfos);
    route_parser.add_route(r"^/billing_info/russia$", || Route::RussiaBillingInfos);
    route_parser.add_route_with_params(r"^/billing_type/by-store-id/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::BillingTypeByStore { id })
    });
    route_parser.add_route_with_params(r"^/billing_info/international/by-store-id/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::InternationalBillingInfoByStore { id })
    });
    route_parser.add_route_with_params(r"^/billing_info/russia/by-store-id/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::RussiaBillingInfoByStore { id })
    });
    route_parser.add_route_with_params(r"^/billing_info/international/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::InternationalBillingInfo { id })
    });
    route_parser.add_route_with_params(r"^/billing_info/russia/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::RussiaBillingInfo { id })
    });
    route_parser.add_route(r"^/payouts$", || Route::Payouts);
    route_parser.add_route(r"^/payouts/by-order-ids$", || Route::PayoutsByOrderIds);
    route_parser.add_route(r"^/payouts/calculate$", || Route::PayoutsCalculate);
    route_parser.add_route_with_params(r"^/payouts/by-store-id/(\d+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::PayoutsByStoreId { id })
    });
    route_parser.add_route_with_params(r"^/payouts/([a-zA-Z0-9-]+)$", |params| {
        params
            .get(0)
            .and_then(|string_id| string_id.parse().ok())
            .map(|id| Route::PayoutById { id })
    });
    route_parser.add_route(r"^/subscriptions$", || Route::Subscriptions);

    route_parser
}
