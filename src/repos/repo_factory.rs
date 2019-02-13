use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Error as FailureError;
use std::sync::Arc;
use stq_cache::cache::Cache;
use stq_types::{BillingRole, UserId};

use models::*;
use repos::legacy_acl::{Acl, SystemACL, UnauthorizedACL};
use repos::*;

pub trait ReposFactory<C>: Clone + Send + Sync + 'static
where
    C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
{
    fn create_order_info_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<OrderInfoRepo + 'a>;
    fn create_order_info_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<OrderInfoRepo + 'a>;
    fn create_invoice_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<InvoiceRepo + 'a>;
    fn create_invoice_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<InvoiceRepo + 'a>;
    fn create_user_roles_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<UserRolesRepo + 'a>;
    fn create_user_roles_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<UserRolesRepo + 'a>;
    fn create_accounts_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<AccountsRepo + 'a>;
    fn create_invoices_v2_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<InvoicesV2Repo + 'a>;
    fn create_invoices_v2_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<InvoicesV2Repo + 'a>;
    fn create_orders_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<OrdersRepo + 'a>;
    fn create_orders_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<OrdersRepo + 'a>;
    fn create_order_exchange_rates_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<OrderExchangeRatesRepo + 'a>;
    fn create_order_exchange_rates_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<OrderExchangeRatesRepo + 'a>;
    fn create_event_store_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<EventStoreRepo + 'a>;
    fn create_payment_intent_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<PaymentIntentRepo + 'a>;
    fn create_payment_intent_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<PaymentIntentRepo + 'a>;
    fn create_customers_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<CustomersRepo + 'a>;
    fn create_customers_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<CustomersRepo + 'a>;
    fn create_fees_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<FeeRepo + 'a>;
    fn create_fees_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<FeeRepo + 'a>;
    fn create_payment_intent_invoices_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<PaymentIntentInvoiceRepo + 'a>;
    fn create_payment_intent_invoices_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<PaymentIntentInvoiceRepo + 'a>;
    fn create_payment_intent_fees_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<PaymentIntentFeeRepo + 'a>;
    fn create_payment_intent_fees_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<PaymentIntentFeeRepo + 'a>;
    fn create_store_billing_type_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<StoreBillingTypeRepo + 'a>;
    fn create_store_billing_type_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<StoreBillingTypeRepo + 'a>;
    fn create_international_billing_info_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>)
        -> Box<InternationalBillingInfoRepo + 'a>;
    fn create_international_billing_repo_info_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<InternationalBillingInfoRepo + 'a>;
    fn create_russia_billing_info_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<RussiaBillingInfoRepo + 'a>;
    fn create_russia_billing_info_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<RussiaBillingInfoRepo + 'a>;
    fn create_proxy_companies_billing_info_repo<'a>(
        &self,
        db_conn: &'a C,
        user_id: Option<UserId>,
    ) -> Box<ProxyCompanyBillingInfoRepo + 'a>;
    fn create_proxy_companies_billing_info_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<ProxyCompanyBillingInfoRepo + 'a>;
    fn create_user_wallets_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<UserWalletsRepo + 'a>;
    fn create_user_wallets_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<UserWalletsRepo + 'a>;
    fn create_payouts_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<PayoutsRepo + 'a>;
    fn create_payouts_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<PayoutsRepo + 'a>;
}

pub struct ReposFactoryImpl<C1>
where
    C1: Cache<Vec<BillingRole>>,
{
    roles_cache: Arc<RolesCacheImpl<C1>>,
    max_processing_attempts: u32,
    stuck_threshold_sec: u32,
}

impl<C1> Clone for ReposFactoryImpl<C1>
where
    C1: Cache<Vec<BillingRole>>,
{
    fn clone(&self) -> Self {
        Self {
            roles_cache: self.roles_cache.clone(),
            max_processing_attempts: self.max_processing_attempts.clone(),
            stuck_threshold_sec: self.stuck_threshold_sec.clone(),
        }
    }
}

impl<C1> ReposFactoryImpl<C1>
where
    C1: Cache<Vec<BillingRole>> + Send + Sync + 'static,
{
    pub fn new(roles_cache: RolesCacheImpl<C1>, max_processing_attempts: u32, stuck_threshold_sec: u32) -> Self {
        Self {
            roles_cache: Arc::new(roles_cache),
            max_processing_attempts,
            stuck_threshold_sec,
        }
    }

    pub fn get_roles<'a, C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static>(
        &self,
        id: UserId,
        db_conn: &'a C,
    ) -> Vec<BillingRole> {
        self.create_user_roles_repo_with_sys_acl(db_conn)
            .list_for_user(id)
            .ok()
            .unwrap_or_default()
    }

    fn get_acl<'a, T, C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static>(
        &self,
        db_conn: &'a C,
        user_id: Option<UserId>,
    ) -> Box<Acl<Resource, Action, Scope, FailureError, T>> {
        user_id.map_or(
            Box::new(UnauthorizedACL::default()) as Box<Acl<Resource, Action, Scope, FailureError, T>>,
            |id| {
                let roles = self.get_roles(id, db_conn);
                (Box::new(ApplicationAcl::new(roles, id)) as Box<Acl<Resource, Action, Scope, FailureError, T>>)
            },
        )
    }
}

impl<C, C1> ReposFactory<C> for ReposFactoryImpl<C1>
where
    C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    C1: Cache<Vec<BillingRole>> + Send + Sync + 'static,
{
    fn create_order_info_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<OrderInfoRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(OrderInfoRepoImpl::new(db_conn, acl)) as Box<OrderInfoRepo>
    }

    fn create_order_info_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<OrderInfoRepo + 'a> {
        Box::new(OrderInfoRepoImpl::new(
            db_conn,
            Box::new(SystemACL::default()) as Box<Acl<Resource, Action, Scope, FailureError, OrderInfo>>,
        )) as Box<OrderInfoRepo>
    }

    fn create_invoice_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<InvoiceRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(InvoiceRepoImpl::new(db_conn, acl)) as Box<InvoiceRepo>
    }

    fn create_invoice_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<InvoiceRepo + 'a> {
        Box::new(InvoiceRepoImpl::new(
            db_conn,
            Box::new(SystemACL::default()) as Box<Acl<Resource, Action, Scope, FailureError, Invoice>>,
        )) as Box<InvoiceRepo>
    }

    fn create_user_roles_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<UserRolesRepo + 'a> {
        Box::new(UserRolesRepoImpl::new(
            db_conn,
            Box::new(SystemACL::default()) as Box<Acl<Resource, Action, Scope, FailureError, UserRole>>,
            self.roles_cache.clone(),
        )) as Box<UserRolesRepo>
    }

    fn create_user_roles_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<UserRolesRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(UserRolesRepoImpl::new(db_conn, acl, self.roles_cache.clone())) as Box<UserRolesRepo>
    }

    fn create_accounts_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<AccountsRepo + 'a> {
        Box::new(AccountsRepoImpl::new(
            db_conn,
            Box::new(SystemACL::default()) as Box<Acl<Resource, Action, Scope, FailureError, Account>>,
        )) as Box<AccountsRepo>
    }

    fn create_invoices_v2_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<InvoicesV2Repo + 'a> {
        Box::new(InvoicesV2RepoImpl::new(db_conn, Box::new(SystemACL::default()))) as Box<InvoicesV2Repo>
    }

    fn create_invoices_v2_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<InvoicesV2Repo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(InvoicesV2RepoImpl::new(db_conn, acl)) as Box<InvoicesV2Repo>
    }

    fn create_orders_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<OrdersRepo + 'a> {
        Box::new(OrdersRepoImpl::new(db_conn, Box::new(SystemACL::default()))) as Box<OrdersRepo>
    }

    fn create_orders_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<OrdersRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(OrdersRepoImpl::new(db_conn, acl)) as Box<OrdersRepo>
    }

    fn create_order_exchange_rates_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<OrderExchangeRatesRepo + 'a> {
        Box::new(OrderExchangeRatesRepoImpl::new(db_conn, Box::new(SystemACL::default()))) as Box<OrderExchangeRatesRepo>
    }

    fn create_order_exchange_rates_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<OrderExchangeRatesRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(OrderExchangeRatesRepoImpl::new(db_conn, acl)) as Box<OrderExchangeRatesRepo>
    }

    fn create_event_store_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<EventStoreRepo + 'a> {
        Box::new(EventStoreRepoImpl::new(
            db_conn,
            self.max_processing_attempts,
            self.stuck_threshold_sec,
        )) as Box<EventStoreRepo>
    }

    fn create_payment_intent_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<PaymentIntentRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(PaymentIntentRepoImpl::new(db_conn, acl))
    }

    fn create_payment_intent_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<PaymentIntentRepo + 'a> {
        let acl = Box::new(SystemACL::default());
        Box::new(PaymentIntentRepoImpl::new(db_conn, acl))
    }

    fn create_customers_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<CustomersRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(CustomersRepoImpl::new(db_conn, acl))
    }

    fn create_customers_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<CustomersRepo + 'a> {
        let acl = Box::new(SystemACL::default());
        Box::new(CustomersRepoImpl::new(db_conn, acl))
    }

    fn create_fees_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<FeeRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(FeeRepoImpl::new(db_conn, acl))
    }

    fn create_fees_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<FeeRepo + 'a> {
        let acl = Box::new(SystemACL::default());
        Box::new(FeeRepoImpl::new(db_conn, acl))
    }

    fn create_store_billing_type_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<StoreBillingTypeRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(StoreBillingTypeRepoImpl::new(db_conn, acl))
    }

    fn create_store_billing_type_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<StoreBillingTypeRepo + 'a> {
        let acl = Box::new(SystemACL::default());
        Box::new(StoreBillingTypeRepoImpl::new(db_conn, acl))
    }

    fn create_international_billing_info_repo<'a>(
        &self,
        db_conn: &'a C,
        user_id: Option<UserId>,
    ) -> Box<InternationalBillingInfoRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(InternationalBillingInfoRepoImpl::new(db_conn, acl))
    }

    fn create_international_billing_repo_info_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<InternationalBillingInfoRepo + 'a> {
        let acl = Box::new(SystemACL::default());
        Box::new(InternationalBillingInfoRepoImpl::new(db_conn, acl))
    }

    fn create_russia_billing_info_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<RussiaBillingInfoRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(RussiaBillingInfoRepoImpl::new(db_conn, acl))
    }

    fn create_russia_billing_info_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<RussiaBillingInfoRepo + 'a> {
        let acl = Box::new(SystemACL::default());
        Box::new(RussiaBillingInfoRepoImpl::new(db_conn, acl))
    }

    fn create_proxy_companies_billing_info_repo<'a>(
        &self,
        db_conn: &'a C,
        user_id: Option<UserId>,
    ) -> Box<ProxyCompanyBillingInfoRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(ProxyCompanyBillingInfoRepoImpl::new(db_conn, acl))
    }

    fn create_proxy_companies_billing_info_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<ProxyCompanyBillingInfoRepo + 'a> {
        let acl = Box::new(SystemACL::default());
        Box::new(ProxyCompanyBillingInfoRepoImpl::new(db_conn, acl))
    }

    fn create_payment_intent_invoices_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<PaymentIntentInvoiceRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(PaymentIntentInvoiceRepoImpl::new(db_conn, acl))
    }

    fn create_payment_intent_invoices_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<PaymentIntentInvoiceRepo + 'a> {
        let acl = Box::new(SystemACL::default());
        Box::new(PaymentIntentInvoiceRepoImpl::new(db_conn, acl))
    }

    fn create_payment_intent_fees_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<PaymentIntentFeeRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(PaymentIntentFeeRepoImpl::new(db_conn, acl))
    }

    fn create_payment_intent_fees_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<PaymentIntentFeeRepo + 'a> {
        let acl = Box::new(SystemACL::default());
        Box::new(PaymentIntentFeeRepoImpl::new(db_conn, acl))
    }

    fn create_user_wallets_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<UserWalletsRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(UserWalletsRepoImpl::new(db_conn, acl))
    }

    fn create_user_wallets_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<UserWalletsRepo + 'a> {
        let acl = Box::new(SystemACL::default());
        Box::new(UserWalletsRepoImpl::new(db_conn, acl))
    }

    fn create_payouts_repo<'a>(&self, db_conn: &'a C, user_id: Option<UserId>) -> Box<PayoutsRepo + 'a> {
        let acl = self.get_acl(db_conn, user_id);
        Box::new(PayoutsRepoImpl::new(db_conn, acl))
    }

    fn create_payouts_repo_with_sys_acl<'a>(&self, db_conn: &'a C) -> Box<PayoutsRepo + 'a> {
        let acl = Box::new(SystemACL::default());
        Box::new(PayoutsRepoImpl::new(db_conn, acl))
    }
}

#[cfg(test)]
pub mod tests {
    extern crate diesel;
    extern crate futures;
    extern crate futures_cpupool;
    extern crate hyper;
    extern crate r2d2;
    extern crate serde_json;
    extern crate stq_http;
    extern crate tokio_core;
    extern crate uuid;

    use futures::Future;
    use hyper::Headers;
    use services::accounts::AccountService;
    use services::types::ServiceFutureV2;
    use std::error::Error;
    use std::fmt;
    use std::sync::Arc;
    use std::time::SystemTime;
    use stq_http::client::HttpClient;
    use stq_http::client::Response;

    use chrono::NaiveDateTime;
    use diesel::connection::AnsiTransactionManager;
    use diesel::connection::SimpleConnection;
    use diesel::deserialize::QueryableByName;
    use diesel::pg::Pg;
    use diesel::query_builder::AsQuery;
    use diesel::query_builder::QueryFragment;
    use diesel::query_builder::QueryId;
    use diesel::sql_types::HasSqlType;
    use diesel::Connection;
    use diesel::ConnectionResult;
    use diesel::QueryResult;
    use diesel::Queryable;
    use futures::Stream;
    use futures_cpupool::CpuPool;
    use r2d2::ManageConnection;
    use tokio_core::reactor::Handle;
    use uuid::Uuid;

    use std::collections::HashMap;
    use stq_static_resources::{Currency, OrderState};
    use stq_types::stripe::PaymentIntentId;
    use stq_types::UserId;
    use stq_types::*;

    use client::payments::{self, CreateAccount, CreateInternalTransaction, GetRate, PaymentsClient, RateRefresh};
    use config::Config;
    use controller::context::{DynamicContext, StaticContext};
    use models::invoice_v2::{InvoiceId as InvoiceV2Id, InvoiceSetAmountPaid, NewInvoice as NewInvoiceV2, RawInvoice as RawInvoiceV2};
    use models::order_v2::{ExchangeId, NewOrder, OrderId as OrderV2Id, OrderSearchResults, OrdersSearch, RawOrder, StoreId as StoreV2Id};
    use models::*;
    use models::{Currency as BillingCurrency, NewPaymentIntent, PaymentIntent, TransactionId, TureCurrency, UpdatePaymentIntent};
    use repos::*;
    use services::*;

    #[derive(Default, Copy, Clone)]
    pub struct ReposFactoryMock;

    impl<C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> ReposFactory<C> for ReposFactoryMock {
        fn create_order_info_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<OrderInfoRepo + 'a> {
            Box::new(OrderInfoRepoMock::default())
        }

        fn create_order_info_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<OrderInfoRepo + 'a> {
            Box::new(OrderInfoRepoMock::default())
        }

        fn create_invoice_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<InvoiceRepo + 'a> {
            Box::new(InvoiceRepoMock::default())
        }

        fn create_invoice_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<InvoiceRepo + 'a> {
            Box::new(InvoiceRepoMock::default())
        }

        fn create_user_roles_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<UserRolesRepo + 'a> {
            Box::new(UserRolesRepoMock::default())
        }

        fn create_user_roles_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<UserRolesRepo + 'a> {
            Box::new(UserRolesRepoMock::default())
        }

        fn create_accounts_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<AccountsRepo + 'a> {
            Box::new(AccountsRepoMock::default())
        }

        fn create_invoices_v2_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<InvoicesV2Repo + 'a> {
            Box::new(InvoicesV2RepoMock::default())
        }

        fn create_invoices_v2_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<InvoicesV2Repo + 'a> {
            Box::new(InvoicesV2RepoMock::default())
        }

        fn create_orders_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<OrdersRepo + 'a> {
            Box::new(OrdersRepoMock::default())
        }

        fn create_orders_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<OrdersRepo + 'a> {
            Box::new(OrdersRepoMock::default())
        }

        fn create_order_exchange_rates_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<OrderExchangeRatesRepo + 'a> {
            Box::new(OrderExchangeRatesRepoMock::default())
        }

        fn create_order_exchange_rates_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<OrderExchangeRatesRepo + 'a> {
            Box::new(OrderExchangeRatesRepoMock::default())
        }

        fn create_event_store_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<EventStoreRepo + 'a> {
            Box::new(EventStoreRepoMock::default())
        }

        fn create_payment_intent_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<PaymentIntentRepo + 'a> {
            Box::new(PaymentIntentRepoMock::default())
        }

        fn create_payment_intent_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<PaymentIntentRepo + 'a> {
            Box::new(PaymentIntentRepoMock::default())
        }

        fn create_customers_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<CustomersRepo + 'a> {
            Box::new(CustomersRepoMock::default())
        }

        fn create_customers_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<CustomersRepo + 'a> {
            Box::new(CustomersRepoMock::default())
        }

        fn create_fees_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<FeeRepo + 'a> {
            Box::new(FeesRepoMock::default())
        }

        fn create_fees_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<FeeRepo + 'a> {
            Box::new(FeesRepoMock::default())
        }

        fn create_store_billing_type_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<StoreBillingTypeRepo + 'a> {
            Box::new(StoreBillingTypeRepoMock::default())
        }

        fn create_store_billing_type_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<StoreBillingTypeRepo + 'a> {
            Box::new(StoreBillingTypeRepoMock::default())
        }

        fn create_international_billing_info_repo<'a>(
            &self,
            _db_conn: &'a C,
            _user_id: Option<UserId>,
        ) -> Box<InternationalBillingInfoRepo + 'a> {
            Box::new(InternationalBillingInfoRepoMock::default())
        }

        fn create_international_billing_repo_info_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<InternationalBillingInfoRepo + 'a> {
            Box::new(InternationalBillingInfoRepoMock::default())
        }

        fn create_russia_billing_info_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<RussiaBillingInfoRepo + 'a> {
            Box::new(RussiaBillingInfoRepoMock::default())
        }

        fn create_russia_billing_info_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<RussiaBillingInfoRepo + 'a> {
            Box::new(RussiaBillingInfoRepoMock::default())
        }

        fn create_proxy_companies_billing_info_repo<'a>(
            &self,
            _db_conn: &'a C,
            _user_id: Option<UserId>,
        ) -> Box<ProxyCompanyBillingInfoRepo + 'a> {
            Box::new(ProxyCompanyBillingInfoRepoMock::default())
        }

        fn create_proxy_companies_billing_info_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<ProxyCompanyBillingInfoRepo + 'a> {
            Box::new(ProxyCompanyBillingInfoRepoMock::default())
        }

        fn create_payment_intent_invoices_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<PaymentIntentInvoiceRepo + 'a> {
            Box::new(PaymentIntentInvoiceRepoMock::default())
        }

        fn create_payment_intent_invoices_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<PaymentIntentInvoiceRepo + 'a> {
            Box::new(PaymentIntentInvoiceRepoMock::default())
        }

        fn create_payment_intent_fees_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<PaymentIntentFeeRepo + 'a> {
            Box::new(PaymentIntentFeeRepoMock::default())
        }

        fn create_payment_intent_fees_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<PaymentIntentFeeRepo + 'a> {
            Box::new(PaymentIntentFeeRepoMock::default())
        }

        fn create_user_wallets_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<UserWalletsRepo + 'a> {
            Box::new(UserWalletsRepoMock::default())
        }

        fn create_user_wallets_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<UserWalletsRepo + 'a> {
            Box::new(UserWalletsRepoMock::default())
        }

        fn create_payouts_repo<'a>(&self, _db_conn: &'a C, _user_id: Option<UserId>) -> Box<PayoutsRepo + 'a> {
            Box::new(PayoutsRepoMock::default())
        }

        fn create_payouts_repo_with_sys_acl<'a>(&self, _db_conn: &'a C) -> Box<PayoutsRepo + 'a> {
            Box::new(PayoutsRepoMock::default())
        }
    }

    #[derive(Clone, Default)]
    pub struct PaymentIntentFeeRepoMock;

    impl PaymentIntentFeeRepo for PaymentIntentFeeRepoMock {
        fn get(&self, _search: SearchPaymentIntentFee) -> RepoResultV2<Option<PaymentIntentFee>> {
            Ok(Some(payment_intent_fee()))
        }

        fn create(&self, _payload: NewPaymentIntentFee) -> RepoResultV2<PaymentIntentFee> {
            Ok(payment_intent_fee())
        }

        fn delete(&self, _search: SearchPaymentIntentFee) -> RepoResultV2<()> {
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    pub struct PaymentIntentInvoiceRepoMock;

    impl PaymentIntentInvoiceRepo for PaymentIntentInvoiceRepoMock {
        fn get(&self, search: SearchPaymentIntentInvoice) -> RepoResultV2<Option<PaymentIntentInvoice>> {
            let res = match search {
                SearchPaymentIntentInvoice::Id(id) => PaymentIntentInvoice {
                    id,
                    ..create_payment_intent_invoice()
                },
                SearchPaymentIntentInvoice::InvoiceId(invoice_id) => PaymentIntentInvoice {
                    invoice_id,
                    ..create_payment_intent_invoice()
                },
                SearchPaymentIntentInvoice::PaymentIntentId(payment_intent_id) => PaymentIntentInvoice {
                    payment_intent_id,
                    ..create_payment_intent_invoice()
                },
            };

            Ok(Some(res))
        }

        fn create(&self, payload: NewPaymentIntentInvoice) -> RepoResultV2<PaymentIntentInvoice> {
            Ok(PaymentIntentInvoice {
                payment_intent_id: payload.payment_intent_id,
                invoice_id: payload.invoice_id,
                ..create_payment_intent_invoice()
            })
        }

        fn delete(&self, _search: SearchPaymentIntentInvoice) -> RepoResultV2<()> {
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    pub struct ProxyCompanyBillingInfoRepoMock;

    impl ProxyCompanyBillingInfoRepo for ProxyCompanyBillingInfoRepoMock {
        fn create(&self, _new_proxy_companies_billing_info: NewProxyCompanyBillingInfo) -> RepoResultV2<ProxyCompanyBillingInfo> {
            Ok(proxy_companies_billing_info())
        }

        fn get(&self, _search: ProxyCompanyBillingInfoSearch) -> RepoResultV2<Option<ProxyCompanyBillingInfo>> {
            Ok(Some(proxy_companies_billing_info()))
        }

        fn update(
            &self,
            _search_params: ProxyCompanyBillingInfoSearch,
            _payload: UpdateProxyCompanyBillingInfo,
        ) -> RepoResultV2<ProxyCompanyBillingInfo> {
            Ok(proxy_companies_billing_info())
        }
    }

    #[derive(Clone, Default)]
    pub struct StoreBillingTypeRepoMock;

    impl StoreBillingTypeRepo for StoreBillingTypeRepoMock {
        fn create(&self, _new_store_billing_type: NewStoreBillingType) -> RepoResultV2<StoreBillingType> {
            Ok(store_billing_type())
        }

        fn get(&self, _search: StoreBillingTypeSearch) -> RepoResultV2<Option<StoreBillingType>> {
            Ok(Some(store_billing_type()))
        }

        fn search(&self, _search: StoreBillingTypeSearch) -> RepoResultV2<Vec<StoreBillingType>> {
            Ok(vec![store_billing_type()])
        }

        fn update(&self, _search: StoreBillingTypeSearch, _payload: UpdateStoreBillingType) -> RepoResultV2<StoreBillingType> {
            Ok(store_billing_type())
        }

        fn delete(&self, _search_params: StoreBillingTypeSearch) -> RepoResultV2<StoreBillingType> {
            Ok(store_billing_type())
        }
    }

    #[derive(Clone, Default)]
    pub struct InternationalBillingInfoRepoMock;

    impl InternationalBillingInfoRepo for InternationalBillingInfoRepoMock {
        fn create(&self, _new_store_billing_type: NewInternationalBillingInfo) -> RepoResultV2<InternationalBillingInfo> {
            Ok(international_billing_info())
        }

        fn get(&self, _search: InternationalBillingInfoSearch) -> RepoResultV2<Option<InternationalBillingInfo>> {
            Ok(Some(international_billing_info()))
        }

        fn search(&self, _search: InternationalBillingInfoSearch) -> RepoResultV2<Vec<InternationalBillingInfo>> {
            Ok(vec![international_billing_info()])
        }

        fn update(
            &self,
            _search_params: InternationalBillingInfoSearch,
            _payload: UpdateInternationalBillingInfo,
        ) -> RepoResultV2<InternationalBillingInfo> {
            Ok(international_billing_info())
        }

        fn delete(&self, _search_params: InternationalBillingInfoSearch) -> RepoResultV2<Option<InternationalBillingInfo>> {
            Ok(Some(international_billing_info()))
        }
    }

    #[derive(Clone, Default)]
    pub struct RussiaBillingInfoRepoMock;

    impl RussiaBillingInfoRepo for RussiaBillingInfoRepoMock {
        fn create(&self, _new_store_billing_type: NewRussiaBillingInfo) -> RepoResultV2<RussiaBillingInfo> {
            Ok(russian_billing_info())
        }

        fn get(&self, _search: RussiaBillingInfoSearch) -> RepoResultV2<Option<RussiaBillingInfo>> {
            Ok(Some(russian_billing_info()))
        }

        fn search(&self, _search: RussiaBillingInfoSearch) -> RepoResultV2<Vec<RussiaBillingInfo>> {
            Ok(vec![russian_billing_info()])
        }

        fn update(&self, _search_params: RussiaBillingInfoSearch, _payload: UpdateRussiaBillingInfo) -> RepoResultV2<RussiaBillingInfo> {
            Ok(russian_billing_info())
        }

        fn delete(&self, _search_params: RussiaBillingInfoSearch) -> RepoResultV2<Option<RussiaBillingInfo>> {
            Ok(Some(russian_billing_info()))
        }
    }

    #[derive(Clone, Default)]
    pub struct CustomersRepoMock;

    impl CustomersRepo for CustomersRepoMock {
        fn get(&self, search: SearchCustomer) -> RepoResultV2<Option<DbCustomer>> {
            let customer = create_db_customer();

            let res = match search {
                SearchCustomer::Id(id) => DbCustomer { id, ..customer },
                SearchCustomer::UserId(user_id) => DbCustomer { user_id, ..customer },
            };

            Ok(Some(res))
        }

        fn create(&self, payload: NewDbCustomer) -> RepoResultV2<DbCustomer> {
            let customer = create_db_customer();

            Ok(DbCustomer {
                id: payload.id,
                user_id: payload.user_id,
                email: payload.email,
                ..customer
            })
        }

        fn update(&self, id: CustomerId, payload: UpdateDbCustomer) -> RepoResultV2<DbCustomer> {
            let customer = create_db_customer();

            Ok(DbCustomer {
                id,
                email: payload.email,
                ..customer
            })
        }

        fn delete(&self, id: CustomerId) -> RepoResultV2<Option<DbCustomer>> {
            let customer = create_db_customer();

            Ok(Some(DbCustomer { id, ..customer }))
        }
    }

    #[derive(Clone, Default)]
    pub struct FeesRepoMock;

    impl FeeRepo for FeesRepoMock {
        fn get(&self, search: SearchFee) -> RepoResultV2<Option<Fee>> {
            let fee = create_fee();

            let res = match search {
                SearchFee::Id(fee_id) => Fee { id: fee_id, ..fee },
                SearchFee::OrderId(order_id) => Fee { order_id, ..fee },
            };

            Ok(Some(res))
        }

        fn search(&self, _search_term: SearchFeeParams) -> RepoResultV2<Vec<Fee>> {
            Ok(vec![create_fee()])
        }

        fn create(&self, payload: NewFee) -> RepoResultV2<Fee> {
            let fee = create_fee();

            Ok(Fee {
                order_id: payload.order_id,
                amount: payload.amount,
                status: payload.status,
                currency: payload.currency,
                crypto_currency: payload.crypto_currency,
                crypto_amount: payload.crypto_amount,
                ..fee
            })
        }

        fn update(&self, fee_id: FeeId, _payload: UpdateFee) -> RepoResultV2<Fee> {
            let fee = create_fee();

            Ok(Fee { id: fee_id, ..fee })
        }

        fn delete(&self, _fee_id: FeeId) -> RepoResultV2<()> {
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    pub struct PaymentIntentRepoMock;

    impl PaymentIntentRepo for PaymentIntentRepoMock {
        fn get(&self, _search: SearchPaymentIntent) -> RepoResultV2<Option<PaymentIntent>> {
            Ok(Some(create_payment_intent()))
        }

        fn create(&self, _new_payment_intent: NewPaymentIntent) -> RepoResultV2<PaymentIntent> {
            Ok(create_payment_intent())
        }

        fn update(&self, _payment_intent_id: PaymentIntentId, _update_payment_intent: UpdatePaymentIntent) -> RepoResultV2<PaymentIntent> {
            Ok(create_payment_intent())
        }

        fn delete(&self, _payment_intent_id: PaymentIntentId) -> RepoResultV2<Option<PaymentIntent>> {
            Ok(Some(create_payment_intent()))
        }
    }

    #[derive(Clone, Default)]
    pub struct OrderInfoRepoMock;

    impl OrderInfoRepo for OrderInfoRepoMock {
        /// Find specific order_info by ID
        fn find(&self, _order_info_id: OrderInfoId) -> RepoResult<Option<OrderInfo>> {
            Ok(Some(create_order_info()))
        }

        /// Find specific order_info by order ID
        fn find_by_order_id(&self, _order_id: OrderId) -> RepoResult<Option<OrderInfo>> {
            Ok(Some(create_order_info()))
        }

        /// Find order_infos by saga ID
        fn find_by_saga_id(&self, _saga_id: SagaId) -> RepoResult<Vec<OrderInfo>> {
            Ok(vec![create_order_info()])
        }

        /// Creates new order_info
        fn create(&self, _payload: NewOrderInfo) -> RepoResult<OrderInfo> {
            Ok(create_order_info())
        }

        /// Updates specific order_info
        fn update_status(&self, saga_id_arg: SagaId, new_status: OrderState) -> RepoResult<Vec<OrderInfo>> {
            let mut order_info = create_order_info();
            order_info.saga_id = saga_id_arg;
            order_info.status = new_status;
            Ok(vec![order_info])
        }

        /// Delete order_infos by saga ID
        fn delete_by_saga_id(&self, saga_id_arg: SagaId) -> RepoResult<Vec<OrderInfo>> {
            let mut order_info = create_order_info();
            order_info.saga_id = saga_id_arg;
            Ok(vec![order_info])
        }
    }

    #[derive(Clone, Default)]
    pub struct InvoiceRepoMock;

    impl InvoiceRepo for InvoiceRepoMock {
        /// Find specific invoice by ID
        fn find(&self, _invoice_id: InvoiceId) -> RepoResult<Option<Invoice>> {
            Ok(Some(create_invoice()))
        }

        /// Find specific invoice by saga ID
        fn find_by_saga_id(&self, _saga_id: SagaId) -> RepoResult<Option<Invoice>> {
            Ok(Some(create_invoice()))
        }

        /// Creates new invoice
        fn create(&self, _payload: Invoice) -> RepoResult<Invoice> {
            Ok(create_invoice())
        }

        /// update new invoice
        fn update(&self, _invoice_id_arg: InvoiceId, _payload: UpdateInvoice) -> RepoResult<Invoice> {
            Ok(create_invoice())
        }

        /// Deletes invoice
        fn delete(&self, _id: SagaId) -> RepoResult<Invoice> {
            Ok(create_invoice())
        }
    }

    #[derive(Clone, Default)]
    pub struct UserRolesRepoMock;

    impl UserRolesRepo for UserRolesRepoMock {
        fn get_by_store_id(&self, store_id: StoreId) -> RepoResultV2<Option<UserRole>> {
            Ok(Some(UserRole {
                id: RoleId::new(),
                user_id: UserId(1),
                name: BillingRole::User,
                data: Some(serde_json::json!(store_id.0)),
            }))
        }

        fn list_for_user(&self, user_id_value: UserId) -> RepoResult<Vec<BillingRole>> {
            Ok(match user_id_value.0 {
                1 => vec![BillingRole::Superuser],
                _ => vec![BillingRole::User],
            })
        }

        fn create(&self, payload: NewUserRole) -> RepoResult<UserRole> {
            Ok(UserRole {
                id: RoleId::new(),
                user_id: payload.user_id,
                name: payload.name,
                data: None,
            })
        }

        fn delete(&self, payload: RemoveUserRole) -> RepoResult<UserRole> {
            Ok(UserRole {
                id: RoleId::new(),
                user_id: payload.user_id,
                name: payload.name,
                data: None,
            })
        }

        fn delete_by_user_id(&self, user_id_arg: UserId) -> RepoResult<Vec<UserRole>> {
            Ok(vec![UserRole {
                id: RoleId::new(),
                user_id: user_id_arg,
                name: BillingRole::User,
                data: None,
            }])
        }

        fn delete_by_id(&self, id: RoleId) -> RepoResult<UserRole> {
            Ok(UserRole {
                id: id,
                user_id: UserId(1),
                name: BillingRole::User,
                data: None,
            })
        }
    }

    #[derive(Clone, Default)]
    pub struct AccountsRepoMock;

    impl AccountsRepo for AccountsRepoMock {
        fn count(&self) -> RepoResultV2<AccountCount> {
            Ok(AccountCount {
                unpooled: HashMap::default(),
                pooled: HashMap::default(),
            })
        }

        fn get(&self, _account_id: AccountId) -> RepoResultV2<Option<Account>> {
            Ok(None)
        }

        fn get_by_wallet_address(&self, _wallet_address: WalletAddress) -> RepoResultV2<Option<Account>> {
            Ok(None)
        }

        fn get_many(&self, _account_ids: &[AccountId]) -> RepoResultV2<Vec<Account>> {
            Ok(vec![])
        }

        fn create(&self, payload: NewAccount) -> RepoResultV2<Account> {
            let NewAccount {
                id,
                currency,
                is_pooled,
                wallet_address,
            } = payload;
            Ok(Account {
                id,
                currency,
                is_pooled,
                created_at: NaiveDateTime::from_timestamp(0, 0),
                wallet_address,
            })
        }

        fn delete(&self, _account_id: AccountId) -> RepoResultV2<Option<Account>> {
            Ok(Some(Account {
                id: AccountId::new(Uuid::nil()),
                currency: TureCurrency::Stq,
                is_pooled: false,
                created_at: NaiveDateTime::from_timestamp(0, 0),
                wallet_address: "0x0".to_string().into(),
            }))
        }

        fn get_free_account(&self, _currency: TureCurrency) -> RepoResultV2<Option<Account>> {
            Ok(None)
        }
    }

    #[derive(Debug, Default)]
    pub struct InvoicesV2RepoMock;

    impl InvoicesV2Repo for InvoicesV2RepoMock {
        fn get(&self, _account_id: InvoiceV2Id) -> RepoResultV2<Option<RawInvoiceV2>> {
            Ok(None)
        }

        fn create(&self, payload: NewInvoiceV2) -> RepoResultV2<RawInvoiceV2> {
            let NewInvoiceV2 {
                id,
                account_id,
                buyer_currency,
                amount_captured,
                buyer_user_id,
            } = payload;

            Ok(RawInvoiceV2 {
                id,
                account_id,
                buyer_currency,
                amount_captured,
                final_amount_paid: None,
                final_cashback_amount: None,
                paid_at: None,
                created_at: NaiveDateTime::from_timestamp(0, 0),
                updated_at: NaiveDateTime::from_timestamp(0, 0),
                buyer_user_id,
                status: OrderState::New,
            })
        }

        fn delete(&self, _invoice_id: InvoiceV2Id) -> RepoResultV2<Option<RawInvoiceV2>> {
            Ok(None)
        }

        fn get_by_account_id(&self, _account_id: AccountId) -> RepoResultV2<Option<RawInvoiceV2>> {
            unimplemented!()
        }

        fn unlink_account(&self, _invoice_id: InvoiceV2Id) -> RepoResultV2<RawInvoiceV2> {
            unimplemented!()
        }

        fn increase_amount_captured(
            &self,
            _account_id: AccountId,
            _transaction_id: TransactionId,
            _amount_received: Amount,
        ) -> RepoResultV2<RawInvoiceV2> {
            unimplemented!()
        }

        fn set_amount_paid(&self, _invoice_id: InvoiceV2Id, _input: InvoiceSetAmountPaid) -> RepoResultV2<RawInvoiceV2> {
            unimplemented!()
        }

        fn set_amount_paid_fiat(&self, _invoice_id: InvoiceV2Id, _input: InvoiceSetAmountPaid) -> RepoResultV2<RawInvoiceV2> {
            unimplemented!()
        }
    }

    #[derive(Debug, Default)]
    pub struct OrdersRepoMock;

    impl OrdersRepo for OrdersRepoMock {
        fn get(&self, _order_id: OrderV2Id) -> RepoResultV2<Option<RawOrder>> {
            Ok(None)
        }

        fn get_many(&self, _order_ids: &[OrderV2Id]) -> RepoResultV2<Vec<RawOrder>> {
            Ok(vec![])
        }

        fn get_many_by_invoice_id(&self, _invoice_id: InvoiceV2Id) -> RepoResultV2<Vec<RawOrder>> {
            Ok(vec![])
        }

        fn search(&self, _skip: i64, _count: i64, _search: OrdersSearch) -> RepoResultV2<OrderSearchResults> {
            Ok(OrderSearchResults {
                total_count: 0,
                orders: vec![],
            })
        }

        fn create(&self, payload: NewOrder) -> RepoResultV2<RawOrder> {
            let NewOrder {
                id,
                seller_currency,
                total_amount,
                cashback_amount,
                invoice_id,
                store_id,
            } = payload;

            Ok(RawOrder {
                id,
                seller_currency,
                total_amount,
                cashback_amount,
                invoice_id,
                created_at: NaiveDateTime::from_timestamp(0, 0),
                updated_at: NaiveDateTime::from_timestamp(0, 0),
                store_id,
                state: PaymentState::Initial,
                stripe_fee: None,
            })
        }

        fn delete(&self, _order_id: OrderV2Id) -> RepoResultV2<Option<RawOrder>> {
            Ok(None)
        }

        fn delete_by_invoice_id(&self, _invoice_id: InvoiceV2Id) -> RepoResultV2<Vec<RawOrder>> {
            Ok(vec![])
        }
        fn update_state(&self, order_id: OrderV2Id, _state: PaymentState) -> RepoResultV2<RawOrder> {
            Ok(RawOrder {
                id: order_id,
                seller_currency: BillingCurrency::Btc,
                total_amount: Amount::new(0),
                cashback_amount: Amount::new(0),
                invoice_id: InvoiceV2Id::generate(),
                created_at: NaiveDateTime::from_timestamp(0, 0),
                updated_at: NaiveDateTime::from_timestamp(0, 0),
                store_id: StoreV2Id::new(1),
                state: PaymentState::Initial,
                stripe_fee: None,
            })
        }
        fn update_stripe_fee(&self, order_id: OrderV2Id, stripe_fee: Amount) -> RepoResultV2<RawOrder> {
            Ok(RawOrder {
                id: order_id,
                seller_currency: BillingCurrency::Btc,
                total_amount: Amount::new(0),
                cashback_amount: Amount::new(0),
                invoice_id: InvoiceV2Id::generate(),
                created_at: NaiveDateTime::from_timestamp(0, 0),
                updated_at: NaiveDateTime::from_timestamp(0, 0),
                store_id: StoreV2Id::new(1),
                state: PaymentState::Initial,
                stripe_fee: Some(stripe_fee),
            })
        }
    }

    #[derive(Debug, Default)]
    pub struct OrderExchangeRatesRepoMock;

    impl OrderExchangeRatesRepo for OrderExchangeRatesRepoMock {
        fn get(&self, _rate_id: OrderExchangeRateId) -> RepoResultV2<Option<RawOrderExchangeRate>> {
            Ok(None)
        }

        fn get_active_rate_for_order(&self, _order_id: OrderV2Id) -> RepoResultV2<Option<RawOrderExchangeRate>> {
            Ok(None)
        }

        fn get_all_rates_for_order(&self, _order_id: OrderV2Id) -> RepoResultV2<Vec<RawOrderExchangeRate>> {
            Ok(vec![])
        }

        fn add_new_active_rate(&self, new_rate: NewOrderExchangeRate) -> RepoResultV2<LatestExchangeRates> {
            let NewOrderExchangeRate {
                order_id,
                exchange_id,
                exchange_rate,
            } = new_rate;

            Ok(LatestExchangeRates {
                active_rate: RawOrderExchangeRate {
                    id: OrderExchangeRateId::new(1),
                    order_id,
                    exchange_id,
                    exchange_rate,
                    status: ExchangeRateStatus::Active,
                    created_at: NaiveDateTime::from_timestamp(0, 0),
                    updated_at: NaiveDateTime::from_timestamp(0, 0),
                },
                last_expired_rate: None,
            })
        }

        fn expire_current_active_rate(&self, _order_id: OrderV2Id) -> RepoResultV2<Option<RawOrderExchangeRate>> {
            Ok(None)
        }

        fn delete(&self, _rate_id: OrderExchangeRateId) -> RepoResultV2<Option<RawOrderExchangeRate>> {
            Ok(None)
        }

        fn delete_by_order_id(&self, _order_id: OrderV2Id) -> RepoResultV2<Vec<RawOrderExchangeRate>> {
            Ok(vec![])
        }
    }

    #[derive(Debug, Default)]
    pub struct EventStoreRepoMock;

    impl EventStoreRepo for EventStoreRepoMock {
        fn add_event(&self, event: Event) -> RepoResultV2<EventEntry> {
            Ok(EventEntry {
                id: EventEntryId::new(1),
                event,
                status: EventStatus::Pending,
                attempt_count: 0,
                created_at: chrono::Utc::now().naive_utc(),
                status_updated_at: chrono::Utc::now().naive_utc(),
                scheduled_on: None,
            })
        }

        fn add_scheduled_event(&self, event: Event, scheduled_on: NaiveDateTime) -> RepoResultV2<EventEntry> {
            Ok(EventEntry {
                id: EventEntryId::new(1),
                event,
                status: EventStatus::Pending,
                attempt_count: 0,
                created_at: chrono::Utc::now().naive_utc(),
                status_updated_at: chrono::Utc::now().naive_utc(),
                scheduled_on: Some(scheduled_on),
            })
        }

        fn get_events_for_processing(&self, limit: u32) -> RepoResultV2<Vec<EventEntry>> {
            Ok((0..limit)
                .map(|i| EventEntry {
                    id: EventEntryId::new(i as i64),
                    event: Event {
                        id: EventId::generate(),
                        payload: EventPayload::NoOp,
                    },
                    status: EventStatus::InProgress,
                    attempt_count: 1,
                    created_at: chrono::Utc::now().naive_utc(),
                    status_updated_at: chrono::Utc::now().naive_utc(),
                    scheduled_on: None,
                })
                .collect::<Vec<_>>())
        }

        fn reset_stuck_events(&self) -> RepoResultV2<Vec<EventEntry>> {
            Ok(vec![])
        }

        fn complete_event(&self, event_entry_id: EventEntryId) -> RepoResultV2<EventEntry> {
            Ok(EventEntry {
                id: event_entry_id,
                event: Event {
                    id: EventId::generate(),
                    payload: EventPayload::NoOp,
                },
                status: EventStatus::Completed,
                attempt_count: 1,
                created_at: chrono::Utc::now().naive_utc(),
                status_updated_at: chrono::Utc::now().naive_utc(),
                scheduled_on: None,
            })
        }

        fn fail_event(&self, event_entry_id: EventEntryId) -> RepoResultV2<EventEntry> {
            Ok(EventEntry {
                id: event_entry_id,
                event: Event {
                    id: EventId::generate(),
                    payload: EventPayload::NoOp,
                },
                status: EventStatus::Failed,
                attempt_count: 5,
                created_at: chrono::Utc::now().naive_utc(),
                status_updated_at: chrono::Utc::now().naive_utc(),
                scheduled_on: None,
            })
        }
    }

    #[derive(Debug, Default)]
    pub struct UserWalletsRepoMock;

    impl UserWalletsRepo for UserWalletsRepoMock {
        fn add(&self, _payload: NewActiveUserWallet) -> RepoResultV2<UserWallet> {
            unimplemented!()
        }

        fn get(&self, _id: UserWalletId) -> RepoResultV2<Option<UserWallet>> {
            unimplemented!()
        }

        fn get_currency_wallets_by_user_id(&self, _currency: TureCurrency, _user_id: ::models::UserId) -> RepoResultV2<Vec<UserWallet>> {
            unimplemented!()
        }

        fn deactivate(&self, _id: UserWalletId) -> RepoResultV2<UserWallet> {
            unimplemented!()
        }

        fn deactivate_wallets_by_user_id(&self, _user_id: ::models::UserId) -> RepoResultV2<Vec<UserWallet>> {
            unimplemented!()
        }
    }

    #[derive(Debug, Default)]
    pub struct PayoutsRepoMock;

    impl PayoutsRepo for PayoutsRepoMock {
        fn create(&self, _payload: Payout) -> RepoResultV2<Payout> {
            unimplemented!()
        }

        fn get(&self, _id: PayoutId) -> RepoResultV2<Option<Payout>> {
            unimplemented!()
        }

        fn get_by_order_id(&self, _order_id: OrderV2Id) -> RepoResultV2<Option<Payout>> {
            unimplemented!()
        }

        fn get_by_order_ids(&self, _order_ids: &[OrderV2Id]) -> RepoResultV2<PayoutsByOrderIds> {
            unimplemented!()
        }

        fn mark_as_completed(&self, _id: PayoutId) -> RepoResultV2<Payout> {
            unimplemented!()
        }
    }

    fn payment_intent_fee() -> PaymentIntentFee {
        PaymentIntentFee {
            id: 1,
            fee_id: FeeId::new(1),
            payment_intent_id: PaymentIntentId("PaymentIntentId".to_string()),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        }
    }

    fn russian_billing_info() -> RussiaBillingInfo {
        RussiaBillingInfo {
            id: RussiaBillingId(1),
            store_id: StoreId(1),
            swift_bic: SwiftId("swift_bic".to_string()),
            branch_name: None,
            personal_account: None,
            bank_name: "bank_name".to_string(),
            tax_id: "tax_id".to_string(),
            correspondent_account: "correspondent_account".to_string(),
            current_account: "current_account".to_string(),
            beneficiary_full_name: "beneficiary_full_name".to_string(),
        }
    }

    fn international_billing_info() -> InternationalBillingInfo {
        InternationalBillingInfo {
            id: InternationalBillingId(1),
            store_id: StoreId(1),
            swift: SwiftId("swift_bic".to_string()),
            currency: Currency::RUB,
            account: "account".to_string(),
            name: "name".to_string(),
            bank: "bank".to_string(),
            bank_address: "bank_address".to_string(),
            country: "country".to_string(),
            city: "city".to_string(),
            recipient_address: "recipient_address".to_string(),
        }
    }

    fn proxy_companies_billing_info() -> ProxyCompanyBillingInfo {
        ProxyCompanyBillingInfo {
            id: ProxyCompanyBillingInfoId(1),
            country_alpha3: Alpha3("RUS".to_string()),
            swift: SwiftId("swift_bic".to_string()),
            currency: Currency::RUB,
            account: "account".to_string(),
            name: "name".to_string(),
            bank: "bank".to_string(),
            bank_address: "bank_address".to_string(),
            country: "country".to_string(),
            city: "city".to_string(),
            recipient_address: "recipient_address".to_string(),
        }
    }

    fn create_payment_intent_invoice() -> PaymentIntentInvoice {
        let now = chrono::offset::Utc::now().naive_utc();
        PaymentIntentInvoice {
            id: 0,
            invoice_id: InvoiceV2Id::new(Uuid::new_v4()),
            payment_intent_id: PaymentIntentId("PaymentIntentId".to_string()),
            created_at: now,
            updated_at: now,
        }
    }

    fn store_billing_type() -> StoreBillingType {
        StoreBillingType {
            id: StoreBillingTypeId(1),
            store_id: StoreId(1),
            billing_type: BillingType::International,
        }
    }

    pub fn create_service(
        user_id: Option<UserId>,
        handle: Arc<Handle>,
    ) -> Service<MockConnection, MockConnectionManager, ReposFactoryMock, MockHttpClient, MockPaymentsClient, MockAccountService> {
        let manager = MockConnectionManager::default();
        let db_pool = r2d2::Pool::builder().build(manager).expect("Failed to create connection pool");
        let cpu_pool = CpuPool::new(1);

        let config = Config::new().unwrap();
        let client = stq_http::client::Client::new(&config.to_http_config(), &handle);
        let client_handle = client.handle();
        let client_stream = client.stream();
        handle.spawn(client_stream.for_each(|_| Ok(())));

        let static_context = StaticContext::new(db_pool, cpu_pool, client_handle.clone(), Arc::new(config), MOCK_REPO_FACTORY);

        let dynamic_context = DynamicContext::new(user_id, String::default(), MockHttpClient::default(), None, None);

        Service::new(static_context, dynamic_context)
    }

    fn create_db_customer() -> DbCustomer {
        let now = chrono::offset::Utc::now().naive_utc();
        DbCustomer {
            id: CustomerId::new("cus_ELDLXPOc4SVNP4".to_string()),
            user_id: UserId(1),
            email: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn create_fee() -> Fee {
        let now = chrono::offset::Utc::now().naive_utc();
        Fee {
            id: FeeId::new(1),
            order_id: OrderV2Id::new(Uuid::new_v4()),
            amount: Amount::new(100),
            status: FeeStatus::NotPaid,
            currency: BillingCurrency::Eur,
            charge_id: None,
            metadata: None,
            created_at: now,
            updated_at: now,
            crypto_currency: None,
            crypto_amount: None,
        }
    }

    fn create_payment_intent() -> PaymentIntent {
        let now = chrono::offset::Utc::now().naive_utc();
        PaymentIntent {
            id: PaymentIntentId("PaymentIntentId".to_string()),
            amount: Amount::new(200),
            amount_received: Amount::new(100),
            client_secret: None,
            currency: BillingCurrency::Eth,
            last_payment_error_message: None,
            receipt_email: None,
            charge_id: None,
            status: PaymentIntentStatus::Other,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn create_order_info() -> OrderInfo {
        OrderInfo {
            id: OrderInfoId::new(),
            order_id: OrderId::new(),
            customer_id: UserId(1),
            store_id: StoreId(1),
            saga_id: SagaId::new(),
            status: OrderState::New,
            total_amount: ProductPrice(100.0),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        }
    }

    pub fn create_invoice() -> Invoice {
        Invoice {
            id: SagaId::new(),
            invoice_id: InvoiceId::new(),
            transactions: serde_json::Value::default(),
            amount: ProductPrice(1f64),
            amount_captured: ProductPrice(1f64),
            currency: Currency::STQ,
            price_reserved: SystemTime::now(),
            state: OrderState::New,
            wallet: Some(Uuid::new_v4().to_string()),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        }
    }

    #[derive(Default)]
    pub struct MockConnection {
        tr: AnsiTransactionManager,
    }

    impl Connection for MockConnection {
        type Backend = Pg;
        type TransactionManager = AnsiTransactionManager;

        fn establish(_database_url: &str) -> ConnectionResult<MockConnection> {
            Ok(MockConnection::default())
        }

        fn execute(&self, _query: &str) -> QueryResult<usize> {
            unimplemented!()
        }

        fn query_by_index<T, U>(&self, _source: T) -> QueryResult<Vec<U>>
        where
            T: AsQuery,
            T::Query: QueryFragment<Pg> + QueryId,
            Pg: HasSqlType<T::SqlType>,
            U: Queryable<T::SqlType, Pg>,
        {
            unimplemented!()
        }

        fn query_by_name<T, U>(&self, _source: &T) -> QueryResult<Vec<U>>
        where
            T: QueryFragment<Pg> + QueryId,
            U: QueryableByName<Pg>,
        {
            unimplemented!()
        }

        fn execute_returning_count<T>(&self, _source: &T) -> QueryResult<usize>
        where
            T: QueryFragment<Pg> + QueryId,
        {
            unimplemented!()
        }

        fn transaction_manager(&self) -> &Self::TransactionManager {
            &self.tr
        }
    }

    impl SimpleConnection for MockConnection {
        fn batch_execute(&self, _query: &str) -> QueryResult<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    pub struct MockConnectionManager;

    impl ManageConnection for MockConnectionManager {
        type Connection = MockConnection;
        type Error = MockError;

        fn connect(&self) -> Result<MockConnection, MockError> {
            Ok(MockConnection::default())
        }

        fn is_valid(&self, _conn: &mut MockConnection) -> Result<(), MockError> {
            Ok(())
        }

        fn has_broken(&self, _conn: &mut MockConnection) -> bool {
            false
        }
    }

    #[derive(Default, Clone)]
    pub struct MockHttpClient;

    impl HttpClient for MockHttpClient {
        fn request(
            &self,
            _method: hyper::Method,
            _url: String,
            _body: Option<String>,
            _headers: Option<Headers>,
        ) -> Box<Future<Item = Response, Error = stq_http::client::Error> + Send> {
            unimplemented!()
        }
    }

    #[derive(Default, Clone)]
    pub struct MockPaymentsClient;

    impl PaymentsClient for MockPaymentsClient {
        fn get_account(&self, _account_id: Uuid) -> Box<Future<Item = payments::Account, Error = payments::Error> + Send> {
            unimplemented!()
        }

        fn list_accounts(&self) -> Box<Future<Item = Vec<payments::Account>, Error = payments::Error> + Send> {
            unimplemented!()
        }

        fn create_account(&self, _input: CreateAccount) -> Box<Future<Item = payments::Account, Error = payments::Error> + Send> {
            unimplemented!()
        }

        fn delete_account(&self, _account_id: Uuid) -> Box<Future<Item = (), Error = payments::Error> + Send> {
            unimplemented!()
        }

        fn get_rate(&self, _input: GetRate) -> Box<Future<Item = payments::Rate, Error = payments::Error> + Send> {
            unimplemented!()
        }

        fn refresh_rate(&self, _exchange_id: ExchangeId) -> Box<Future<Item = RateRefresh, Error = payments::Error> + Send> {
            unimplemented!()
        }

        fn create_internal_transaction(&self, _input: CreateInternalTransaction) -> Box<Future<Item = (), Error = payments::Error> + Send> {
            unimplemented!()
        }
    }

    #[derive(Default, Clone)]
    pub struct MockAccountService;

    impl AccountService for MockAccountService {
        fn init_system_accounts(&self) -> ServiceFutureV2<()> {
            unimplemented!()
        }

        fn init_account_pools(&self) -> ServiceFutureV2<()> {
            unimplemented!()
        }

        fn create_account(&self, _account_id: Uuid, _name: String, _currency: TureCurrency, _is_pooled: bool) -> ServiceFutureV2<Account> {
            unimplemented!()
        }

        fn get_account(&self, _account_id: Uuid) -> ServiceFutureV2<AccountWithBalance> {
            unimplemented!()
        }

        fn get_main_account(&self, _currency: TureCurrency) -> ServiceFutureV2<AccountWithBalance> {
            unimplemented!()
        }

        fn get_stq_cashback_account(&self) -> ServiceFutureV2<AccountWithBalance> {
            unimplemented!()
        }

        fn get_or_create_free_pooled_account(&self, _currency: TureCurrency) -> ServiceFutureV2<Account> {
            unimplemented!()
        }
    }

    #[derive(Debug)]
    pub struct MockError {}

    impl fmt::Display for MockError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "SuperError is here!")
        }
    }

    impl Error for MockError {
        fn description(&self) -> &str {
            "I'm the superhero of errors"
        }

        fn cause(&self) -> Option<&Error> {
            None
        }
    }

    pub const MOCK_REPO_FACTORY: ReposFactoryMock = ReposFactoryMock {};

}
