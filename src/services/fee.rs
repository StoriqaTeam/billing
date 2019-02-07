//! FeesService Services, presents CRUD operations with fee table
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures::IntoFuture;
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

use models::{
    order_v2::{OrderId, OrdersSearch, StoreId},
    Amount, ChargeId, Currency, Fee, FeeStatus, UpdateFee,
};
use repos::{ReposFactory, SearchCustomer, SearchFee, SearchFeeParams};

use super::types::ServiceFutureV2;
use controller::{context::DynamicContext, requests::FeesPayByOrdersRequest, responses::FeeResponse};
use models::order_v2::OrderId as Orderv2Id;
use services::{Error, ErrorContext, ErrorKind};

use services::types::spawn_on_pool;

pub trait FeesService {
    /// Getting fee by order id
    fn get_by_order_id(&self, order_id: OrderId) -> ServiceFutureV2<Option<FeeResponse>>;
    /// Create Charge object in Stripe
    fn create_charge(&self, search: SearchFee) -> ServiceFutureV2<FeeResponse>;
    /// Create Charge object in Stripe
    fn create_charge_for_several_fees(&self, params: FeesPayByOrdersRequest) -> ServiceFutureV2<Vec<FeeResponse>>;
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
        let user_id = self.dynamic_context.user_id;
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        let fut = spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let fees_repo = repo_factory.create_fees_repo(&conn, user_id);
            let order_repo = repo_factory.create_orders_repo(&conn, user_id);

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

            Ok((current_order.store_id, current_fee))
        })
        .and_then({
            let self_cloned = self.clone();
            move |(store_id, fee)| self_cloned.create_charge_by_fees(store_id, vec![fee])
        })
        .and_then(|responses| {
            responses.into_iter().next().ok_or({
                let e = format_err!("Responses is empty");
                ectx!(err e, ErrorKind::Internal)
            })
        });

        Box::new(fut)
    }

    fn create_charge_for_several_fees(&self, params: FeesPayByOrdersRequest) -> ServiceFutureV2<Vec<FeeResponse>> {
        debug!("Create charge in stripe by params: {:?}", params);
        self.create_charge_by_order_ids(params.order_ids)
    }
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > FeesServiceImpl<T, M, F, C, PC, AS>
{
    fn create_charge_by_order_ids(&self, orders: Vec<Orderv2Id>) -> ServiceFutureV2<Vec<FeeResponse>> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let fut = spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let fees_repo = repo_factory.create_fees_repo(&conn, user_id);
            let order_repo = repo_factory.create_orders_repo(&conn, user_id);

            let orders = order_repo
                .search(0, orders.len() as i64, OrdersSearch::by_order_ids(orders.clone()))
                .map_err(ectx!(try convert))?;

            let store_ids: HashSet<StoreId> = orders.orders.iter().map(|order| order.store_id).collect();

            verify_store_ids(&store_ids)?;

            let store_id = store_ids.into_iter().next().ok_or({
                let e = format_err!("fee store not fount");
                ectx!(try err e, ErrorKind::Internal)
            })?;

            let fees = fees_repo
                .search(SearchFeeParams::by_order_ids(orders.orders.iter().map(|o| o.id).collect()))
                .map_err(ectx!(try convert))?;

            Ok((store_id, fees))
        })
        .and_then({
            let self_clone = self.clone();

            move |(store_id, fees)| self_clone.create_charge_by_fees(store_id, fees)
        });

        Box::new(fut)
    }

    fn create_charge_by_fees(&self, store_id: StoreId, fees: Vec<Fee>) -> ServiceFutureV2<Vec<FeeResponse>> {
        let user_id = self.dynamic_context.user_id;
        let repo_factory = self.repo_factory.clone();
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let stripe_client = self.stripe_client.clone();
        let fut = spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let user_roles_repo = repo_factory.create_user_roles_repo(&conn, user_id);
            let customers_repo = repo_factory.create_customers_repo(&conn, user_id);

            validate_charge_fees(&fees)?;

            let store_owner_user_role = user_roles_repo
                .get_by_store_id(StqStoreId(store_id.inner()))
                .map_err(|e| ectx!(try err e, ErrorKind::Internal => store_id))?
                .ok_or({
                    let e = format_err!("Store owner for store id {} not found", store_id);
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

            Ok((fees, stripe_customer))
        })
        .and_then(move |(fees, customer)| {
            total_amount(fees.clone())
                .into_future()
                .and_then({
                    let fees = fees.clone();
                    move |amount| extract_currency(fees).map(move |currency| (currency, amount))
                })
                .and_then(move |(currency, amount)| {
                    let new_charge = NewCharge {
                        customer_id: customer.id.clone(),
                        amount,
                        currency,
                        capture: true,
                    };

                    let customer_id_cloned = customer.id.clone();

                    stripe_client
                        .create_charge(new_charge, create_charge_metadata(&fees))
                        .map_err(ectx!(convert => customer_id_cloned))
                        .map(|charge| (fees, charge))
                })
        })
        .and_then({
            let repo_factory = self.repo_factory.clone();
            let db_pool = self.db_pool.clone();
            let cpu_pool = self.cpu_pool.clone();
            move |(fees, charge)| {
                spawn_on_pool(db_pool, cpu_pool, move |conn| {
                    let fees_repo = repo_factory.create_fees_repo(&conn, user_id);
                    conn.transaction(|| {
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
                        let fee_result: Result<Vec<_>, _> = fees
                            .into_iter()
                            .map(|fee| {
                                let fee_id_cloned = fee.id.clone();
                                fees_repo
                                    .update(fee.id, update_fee.clone())
                                    .map_err(ectx!(convert => fee_id_cloned))
                                    .and_then(|res| FeeResponse::try_from_fee(res))
                            })
                            .collect();
                        fee_result
                    })
                })
            }
        });

        Box::new(fut)
    }
}

fn validate_charge_fees(fees: &[Fee]) -> Result<(), Error> {
    for fee in fees {
        if fee.status == FeeStatus::Paid {
            let mut errors = ValidationErrors::new();
            let mut error = ValidationError::new("wrong_fee_status");
            error.message = Some(format!("Cannot charge fee - fee {} has status \"{}\"", fee.id, FeeStatus::Paid).into());
            errors.add("order_id", error);
            return Err(ectx!(err ErrorContext::OrderState ,ErrorKind::Validation(serde_json::to_value(errors).unwrap_or_default())));
        }
    }
    Ok(())
}

fn extract_currency(fees: Vec<Fee>) -> Result<Currency, Error> {
    let currencies: HashSet<Currency> = fees.iter().map(|fee| fee.currency).collect();
    if currencies.len() != 1 {
        let mut errors = ValidationErrors::new();
        let mut error = ValidationError::new("wrong_currency");
        error.message = Some(format!("Cannot charge fee - orders have different currencies").into());
        errors.add("order_id", error);
        return Err(ectx!(err ErrorContext::OrderState ,ErrorKind::Validation(serde_json::to_value(errors).unwrap_or_default())));
    }
    let currency = currencies.into_iter().next().ok_or({
        let e = format_err!("currency not fount");
        ectx!(try err e, ErrorKind::Internal)
    })?;
    Ok(currency)
}

fn total_amount(fees: Vec<Fee>) -> Result<Amount, Error> {
    fees.iter()
        .map(|fee| fee.amount)
        .try_fold(Amount::zero(), |acc, next| acc.checked_add(next))
        .ok_or_else(|| {
            let e = format_err!("Amount checked add error");
            ectx!(err e, ErrorKind::Internal)
        })
}

fn create_charge_metadata(fees: &[Fee]) -> Option<HashMap<String, String>> {
    if fees.len() > 1 {
        None
    } else {
        fees.first().map(|fee| {
            let mut metadata = HashMap::new();
            metadata.insert("order_id".to_string(), format!("{}", fee.order_id));
            metadata.insert("fee_id".to_string(), format!("{}", fee.id));
            metadata
        })
    }
}

fn verify_store_ids(store_ids: &HashSet<StoreId>) -> Result<(), Error> {
    if store_ids.len() != 1 {
        let mut errors = ValidationErrors::new();
        let mut error = ValidationError::new("wrong_store_id");
        error.message = Some(format!("Cannot charge fee - orders belong to different stores").into());
        errors.add("order_id", error);
        return Err(ectx!(err ErrorContext::OrderState ,ErrorKind::Validation(serde_json::to_value(errors).unwrap_or_default())));
    }
    Ok(())
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > Clone for FeesServiceImpl<T, M, F, C, PC, AS>
{
    fn clone(&self) -> Self {
        FeesServiceImpl {
            db_pool: self.db_pool.clone(),
            cpu_pool: self.cpu_pool.clone(),
            repo_factory: self.repo_factory.clone(),
            stripe_client: self.stripe_client.clone(),
            dynamic_context: self.dynamic_context.clone(),
        }
    }
}
