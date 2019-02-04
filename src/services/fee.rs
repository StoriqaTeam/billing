//! FeesService Services, presents CRUD operations with fee table
use std::collections::HashMap;
use std::sync::Arc;

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};
use validator::{ValidationError, ValidationErrors};

use failure::Fail;

use futures::Future;
use stq_http::client::HttpClient;
use stq_types::StoreId as StqStoreId;

use client::payments::PaymentsClient;
use client::stripe::{NewCharge, StripeClient};
use services::accounts::AccountService;

use models::{order_v2::OrderId, ChargeId, FeeStatus, UpdateFee};
use repos::{ReposFactory, SearchCustomer, SearchFee};

use super::types::ServiceFutureV2;
use controller::{context::DynamicContext, responses::FeeResponse};
use services::{ErrorContext, ErrorKind};

use services::types::spawn_on_pool;

pub trait FeesService {
    /// Getting fee by order id
    fn get_by_order_id(&self, order_id: OrderId) -> ServiceFutureV2<Option<FeeResponse>>;
    /// Create Charge object in Stripe
    fn create_charge(&self, search: SearchFee) -> ServiceFutureV2<FeeResponse>;
}

pub struct FeesServiceImpl<
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
    C: HttpClient + Clone,
    PC: PaymentsClient + Clone,
    AS: AccountService + Clone,
> {
    pub db_pool: Pool<M>,
    pub cpu_pool: CpuPool,
    pub repo_factory: F,
    pub stripe_client: Arc<dyn StripeClient>,
    pub dynamic_context: DynamicContext<C, PC, AS>,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > FeesService for FeesServiceImpl<T, M, F, C, PC, AS>
{
    fn get_by_order_id(&self, order_id: OrderId) -> ServiceFutureV2<Option<FeeResponse>> {
        debug!("Requesting fee record by order id: {}", order_id);

        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let fees_repo = repo_factory.create_fees_repo(&conn, user_id);

            fees_repo
                .get(SearchFee::OrderId(order_id))
                .map_err(ectx!(convert => order_id))
                .and_then(|fee| {
                    if let Some(fee) = fee {
                        FeeResponse::try_from_fee(fee).map(|res| Some(res))
                    } else {
                        Ok(None)
                    }
                })
        })
    }

    fn create_charge(&self, search: SearchFee) -> ServiceFutureV2<FeeResponse> {
        debug!("Create charge in stripe by params: {:?}", search);

        let repo_factory = self.repo_factory.clone();
        let repo_factory2 = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let db_pool2 = self.db_pool.clone();
        let cpu_pool2 = self.cpu_pool.clone();
        let stripe_client = self.stripe_client.clone();

        let fut = spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let fees_repo = repo_factory.create_fees_repo(&conn, user_id);
            let user_roles_repo = repo_factory.create_user_roles_repo(&conn, user_id);
            let order_repo = repo_factory.create_orders_repo(&conn, user_id);
            let customers_repo = repo_factory.create_customers_repo(&conn, user_id);

            let search_cloned = search.clone();
            let current_fee = fees_repo.get(search.clone()).map_err(ectx!(try convert => search_cloned))?.ok_or({
                let e = format_err!("Fee by search params {:?} not found", search);
                ectx!(try err e, ErrorKind::Internal)
            })?;

            let order_id_cloned = current_fee.order_id.clone();
            let current_order = order_repo
                .get(current_fee.order_id)
                .map_err(ectx!(try convert => order_id_cloned))?
                .ok_or({
                    let e = format_err!("Order by id {} not found", current_fee.order_id);
                    ectx!(try err e, ErrorKind::Internal)
                })?;

            let store_id_cloned = current_order.store_id;

            let store_owner_user_role = user_roles_repo
                .get_by_store_id(StqStoreId(store_id_cloned.inner()))
                .map_err(|e| ectx!(try err e, ErrorKind::Internal => store_id_cloned))?
                .ok_or({
                    let e = format_err!("Store owner for store id {} not found", store_id_cloned);
                    ectx!(try err e, ErrorKind::Internal)
                })?;
            let store_owner = store_owner_user_role.user_id;

            let stripe_customer = customers_repo
                .get(SearchCustomer::UserId(store_owner))
                .map_err(ectx!(try convert => store_owner))?
                .ok_or_else(|| {
                    let mut errors = ValidationErrors::new();
                    let mut error = ValidationError::new("not_exists");
                    error.message = Some(format!("Cannot charge fee - payment card does not exist").into());
                    errors.add("payment_card", error);
                    ectx!(try err ErrorContext::OrderState ,ErrorKind::Validation(serde_json::to_value(errors).unwrap_or_default()))
                })?;

            Ok((current_fee, stripe_customer))
        })
        .and_then(move |(fee, customer)| {
            let new_charge = NewCharge {
                customer_id: customer.id.clone(),
                amount: fee.amount,
                currency: fee.currency,
                capture: true,
            };

            let customer_id_cloned = customer.id.clone();
            let mut metadata = HashMap::new();
            metadata.insert("order_id".to_string(), format!("{}", fee.order_id));
            metadata.insert("fee_id".to_string(), format!("{}", fee.id));

            stripe_client
                .create_charge(new_charge, Some(metadata))
                .map_err(ectx!(convert => customer_id_cloned))
                .map(|charge| (fee, charge))
        })
        .and_then(move |(fee, charge)| {
            spawn_on_pool(db_pool2, cpu_pool2, move |conn| {
                let fees_repo = repo_factory2.create_fees_repo(&conn, user_id);

                let status = if charge.paid {
                    Some(FeeStatus::Paid)
                } else {
                    Some(FeeStatus::Fail)
                };
                let charge_id = Some(charge.id).map(|v| ChargeId::new(v));
                let update_fee = UpdateFee {
                    charge_id,
                    status,
                    ..Default::default()
                };

                let fee_id_cloned = fee.id.clone();
                fees_repo
                    .update(fee.id, update_fee)
                    .map_err(ectx!(convert => fee_id_cloned))
                    .and_then(|res| FeeResponse::try_from_fee(res))
            })
        });

        Box::new(fut)
    }
}
