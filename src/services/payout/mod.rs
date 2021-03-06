mod types;

use std::collections::HashMap;

use chrono::Utc;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::{err_msg, Fail};
use futures::{future, Future};
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};
use stq_types::UserId as StqUserId;
use validator::{ValidationError, ValidationErrors};

use client::payments::{self, PaymentsClient};
use controller::responses::BalancesResponse;
use models::order_v2::{OrderId, OrderPaymentKind, RawOrder, StoreId};
use models::*;
use repos::ReposFactory;
use services::types::spawn_on_pool;
use services::ErrorKind;

use super::types::{ServiceFutureV2, ServiceResultV2};

pub use self::types::*;

pub trait PayoutService {
    fn get_balance(&self, store_id: StoreId) -> ServiceFutureV2<BalancesResponse>;
    fn calculate_payout(&self, payload: CalculatePayoutPayload) -> ServiceFutureV2<CalculatedPayoutOutput>;
    fn get_payout(&self, payout_id: PayoutId) -> ServiceFutureV2<Option<PayoutOutput>>;
    fn get_payouts_by_order_ids(&self, order_ids: GetPayoutsPayload) -> ServiceFutureV2<PayoutsByOrderIdsOutput>;
    fn get_payouts_by_store_id(&self, store_id: StoreId) -> ServiceFutureV2<PayoutsByStoreIdOutput>;
    fn pay_out_to_seller(&self, payload: PayOutToSellerPayload) -> ServiceFutureV2<PayoutOutput>;
}

pub struct PayoutServiceImpl<
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
    PC: PaymentsClient + Clone,
> {
    pub db_pool: Pool<M>,
    pub cpu_pool: CpuPool,
    pub repo_factory: F,
    pub user_id: Option<StqUserId>,
    pub payments_client: Option<PC>,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        PC: PaymentsClient + Clone,
    > PayoutService for PayoutServiceImpl<T, M, F, PC>
{
    fn get_balance(&self, store_id: StoreId) -> ServiceFutureV2<BalancesResponse> {
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let repo_factory = self.repo_factory.clone();
        let user_id = self.user_id.clone();

        let fut = spawn_on_pool(db_pool.clone(), cpu_pool.clone(), move |conn| {
            let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
            let payouts_repo = repo_factory.create_payouts_repo(&conn, user_id);

            let orders_for_payout = orders_repo
                .get_orders_for_payout(store_id.clone(), None)
                .map_err(ectx!(try convert => store_id))?;

            let order_ids_without_payout = {
                let order_ids = orders_for_payout.iter().map(|o| o.id).collect::<Vec<_>>();

                payouts_repo
                    .get_by_order_ids(&order_ids)
                    .map(|p| p.order_ids_without_payout)
                    .map_err(ectx!(try convert => order_ids))
            }?;

            orders_for_payout
                .into_iter()
                .filter(|order| order_ids_without_payout.contains(&order.id))
                .try_fold(
                    HashMap::new(),
                    |mut hash_map,
                     RawOrder {
                         total_amount,
                         seller_currency,
                         ..
                     }| {
                        {
                            let gross_amount = hash_map.entry(seller_currency).or_insert(Amount::zero());
                            *gross_amount = gross_amount.checked_add(total_amount)?;
                        }
                        Some(hash_map)
                    },
                )
                .ok_or({
                    let e = err_msg("Overflow while calculating the gross amount of a payout");
                    ectx!(err e, ErrorKind::Internal)
                })
                .map(|hash| {
                    BalancesResponse::new(
                        hash.into_iter()
                            .map(|(currency, gross_amount)| (currency.into(), gross_amount.to_super_unit(currency)))
                            .collect(),
                    )
                })
        });

        Box::new(fut)
    }

    fn calculate_payout(&self, payload: CalculatePayoutPayload) -> ServiceFutureV2<CalculatedPayoutOutput> {
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let repo_factory = self.repo_factory.clone();
        let user_id = self.user_id.clone();

        let payments_client = match self.payments_client.clone() {
            None => return Box::new(future::err(ErrorKind::NotFound.into())),
            Some(payments_client) => payments_client,
        };

        let CalculatePayoutPayload {
            store_id,
            currency,
            wallet_address,
        } = payload;

        let fut = spawn_on_pool(db_pool.clone(), cpu_pool.clone(), move |conn| {
            let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
            let payouts_repo = repo_factory.create_payouts_repo(&conn, user_id);

            let orders_for_payout = orders_repo
                .get_orders_for_payout(store_id.clone(), Some(currency.clone().into()))
                .map_err(ectx!(try convert => store_id, currency))?;

            let order_ids_without_payout = {
                let order_ids = orders_for_payout.iter().map(|o| o.id).collect::<Vec<_>>();

                payouts_repo
                    .get_by_order_ids(&order_ids)
                    .map(|p| p.order_ids_without_payout)
                    .map_err(ectx!(try convert => order_ids))
            }?;

            orders_for_payout
                .into_iter()
                .filter(|order| order_ids_without_payout.contains(&order.id))
                .try_fold(
                    CalculatedPayoutExcludingFees {
                        order_ids: Vec::default(),
                        currency,
                        gross_amount: Amount::zero(),
                    },
                    |mut payout, RawOrder { id, total_amount, .. }| {
                        payout.order_ids.push(id);
                        payout.gross_amount = payout.gross_amount.checked_add(total_amount)?;
                        Some(payout)
                    },
                )
                .ok_or({
                    let e = err_msg("Overflow while calculating the gross amount of a payout");
                    ectx!(err e, ErrorKind::Internal)
                })
        })
        .and_then(move |calculated_payout_excluding_fees| {
            let CalculatedPayoutExcludingFees {
                order_ids,
                currency,
                gross_amount,
            } = calculated_payout_excluding_fees;

            let input = payments::GetFees {
                currency,
                account_address: wallet_address.into_inner(),
            };

            payments_client
                .get_fees(input.clone())
                .map(move |payments::FeesResponse { currency: _, fees }| CalculatedPayoutOutput {
                    order_ids,
                    currency,
                    gross_amount: gross_amount.to_super_unit(currency.into()),
                    blockchain_fee_options: fees
                        .into_iter()
                        .map(|fee| BlockchainFeeOption::from_payments_fee(currency, fee))
                        .collect(),
                })
                .map_err(ectx!(convert => input))
        })
        .then(|res| {
            debug!("Calculated payout: {:?}", res);
            res
        });

        Box::new(fut)
    }

    fn get_payout(&self, payout_id: PayoutId) -> ServiceFutureV2<Option<PayoutOutput>> {
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let repo_factory = self.repo_factory.clone();
        let user_id = self.user_id.clone();

        spawn_on_pool(db_pool.clone(), cpu_pool.clone(), move |conn| {
            let payouts_repo = repo_factory.create_payouts_repo(&conn, user_id);
            payouts_repo
                .get(payout_id)
                .map_err(ectx!(convert => payout_id))
                .map(|payout| payout.map(PayoutOutput::from))
        })
    }

    fn get_payouts_by_order_ids(&self, payload: GetPayoutsPayload) -> ServiceFutureV2<PayoutsByOrderIdsOutput> {
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let repo_factory = self.repo_factory.clone();
        let user_id = self.user_id.clone();

        spawn_on_pool(db_pool.clone(), cpu_pool.clone(), move |conn| {
            let payouts_repo = repo_factory.create_payouts_repo(&conn, user_id);
            payouts_repo
                .get_by_order_ids(&payload.order_ids)
                .map(PayoutsByOrderIdsOutput::from)
                .map_err(ectx!(convert => payload.order_ids.to_vec()))
        })
    }

    fn get_payouts_by_store_id(&self, store_id: StoreId) -> ServiceFutureV2<PayoutsByStoreIdOutput> {
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let repo_factory = self.repo_factory.clone();
        let user_id = self.user_id.clone();

        spawn_on_pool(db_pool.clone(), cpu_pool.clone(), move |conn| {
            let orders_repo = repo_factory.create_orders_repo(&conn, user_id);
            let payouts_repo = repo_factory.create_payouts_repo(&conn, user_id);

            let order_ids = orders_repo
                .get_order_ids_by_store_id(store_id.clone())
                .map_err(ectx!(try convert => store_id))?;

            payouts_repo
                .get_by_order_ids(&order_ids)
                .map_err(ectx!(convert => order_ids.to_vec()))
                .map(|payouts| {
                    let payouts_by_order_ids = PayoutsByOrderIdsOutput::from(payouts);
                    PayoutsByStoreIdOutput {
                        store_id,
                        payouts_by_order_ids,
                    }
                })
        })
    }

    fn pay_out_to_seller(&self, payload: PayOutToSellerPayload) -> ServiceFutureV2<PayoutOutput> {
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let repo_factory = self.repo_factory.clone();
        let user_id = self.user_id.clone();

        let user_id = match user_id {
            None => return Box::new(future::err(ErrorKind::Forbidden.into())),
            Some(user_id) => user_id,
        };

        let PayOutToSellerPayload {
            order_ids,
            payment_details:
                PaymentDetails::Crypto(CryptoPaymentDetails {
                    wallet_currency,
                    wallet_address,
                    blockchain_fee,
                }),
        } = payload;

        let blockchain_fee = Amount::from_super_unit(wallet_currency.into(), blockchain_fee);

        spawn_on_pool(db_pool.clone(), cpu_pool.clone(), move |conn| {
            let orders_repo = repo_factory.create_orders_repo(&conn, Some(user_id));
            let payouts_repo = repo_factory.create_payouts_repo(&conn, Some(user_id));
            let event_store_repo = repo_factory.create_event_store_repo_with_sys_acl(&conn);

            let order_ids_clone = order_ids.clone();
            let orders = orders_repo
                .get_many(&order_ids_clone)
                .map_err(ectx!(try convert => order_ids_clone))?;

            if orders.len() != order_ids.len() {
                let missing_ids = order_ids
                    .iter()
                    .filter(|order_id| orders.iter().all(|order| order.id != **order_id))
                    .map(OrderId::to_string)
                    .collect::<Vec<_>>();

                let mut errors = ValidationErrors::new();
                let mut error = ValidationError::new("missing_orders");
                error.message = Some(format!("Missing orders with IDs: {}", missing_ids.join(", ")).into());
                errors.add("order_ids", error);

                return Err(ErrorKind::from(errors).into());
            }

            let OrdersForPayout { currency, orders } = validate_orders_for_payout(orders)?;
            if wallet_currency != currency {
                let mut errors = ValidationErrors::new();
                let mut error = ValidationError::new("currency_mismatch");
                error.message = Some(format!("Currency of the orders differs from the wallet currency").into());
                error.add_param("orders_currency".into(), &currency);
                error.add_param("wallet_currency".into(), &wallet_currency);
                errors.add("wallet_currency", error);

                return Err(ErrorKind::from(errors).into());
            }

            let PayoutsByOrderIds {
                payouts,
                order_ids_without_payout: _,
            } = payouts_repo.get_by_order_ids(&order_ids).map_err(ectx!(try convert))?;

            if !payouts.is_empty() {
                let order_ids = payouts.keys().cloned().collect::<Vec<_>>();

                let mut errors = ValidationErrors::new();
                let mut error = ValidationError::new("payouts_exist");
                error.message = Some("Payouts already exist for some orders".into());
                error.add_param("payouts".into(), &order_ids);
                errors.add("order_ids", error);

                return Err(ErrorKind::from(errors).into());
            }

            let gross_amount = orders
                .iter()
                .map(|o| o.total_amount)
                .try_fold(Amount::new(0), |acc, next| acc.checked_add(next))
                .ok_or(ErrorKind::Internal)?;

            let net_amount = gross_amount.checked_sub(blockchain_fee).ok_or({
                let mut errors = ValidationErrors::new();
                let mut error = ValidationError::new("payout_lt_fee");
                error.message = Some("Payout is less than the blockchain fee".into());
                error.add_param("payouts".into(), &order_ids);
                errors.add("blockchain_fee", error);

                ErrorKind::from(errors)
            })?;

            let payout = Payout {
                id: PayoutId::generate(),
                gross_amount,
                net_amount,
                target: PayoutTarget::CryptoWallet(CryptoWalletPayoutTarget {
                    currency,
                    wallet_address,
                    blockchain_fee,
                }),
                user_id: UserId::new(user_id.clone().0),
                status: PayoutStatus::Processing {
                    initiated_at: Utc::now().naive_utc(),
                },
                order_ids,
            };

            let payout_initiated_event = Event::new(EventPayload::PayoutInitiated { payout_id: payout.id });
            event_store_repo
                .add_event(payout_initiated_event.clone())
                .map_err(ectx!(try convert => payout_initiated_event))?;

            payouts_repo
                .create(payout.clone())
                .map(PayoutOutput::from)
                .map_err(ectx!(convert => payout))
        })
    }
}

fn validate_orders_for_payout(orders: Vec<RawOrder>) -> ServiceResultV2<OrdersForPayout> {
    let mut errors = ValidationErrors::new();

    let first_order = match orders.iter().next().cloned() {
        None => {
            let mut error = ValidationError::new("empty");
            error.message = Some("Order list is empty".into());
            errors.add("order_ids", error);

            return Err(ErrorKind::from(errors).into());
        }
        Some(order) => order,
    };

    for order in &orders {
        if order.state != PaymentState::PaymentToSellerNeeded {
            let mut error = ValidationError::new("wrong_state");
            error.message = Some("Order has the wrong state".into());
            error.add_param("order".into(), &json!({ "id": order.id, "state": order.state }));
            errors.add("order_ids", error);
        }
    }

    if orders.iter().any(|order| order.seller_currency != first_order.seller_currency) {
        let mut error = ValidationError::new("different_currencies");
        error.message = Some("Orders have different currencies".into());
        errors.add("order_ids", error);
    };

    let currency = match first_order.payment_kind() {
        OrderPaymentKind::Crypto { currency } => currency,
        OrderPaymentKind::Fiat { currency, stripe_fee: _ } => {
            let mut error = ValidationError::new("fiat_not_supported");
            error.message = Some("Fiat orders are not supported".into());
            error.add_param("currency".into(), &currency);
            errors.add("order_ids", error);
            return Err(ErrorKind::from(errors).into());
        }
    };

    if !errors.is_empty() {
        return Err(ErrorKind::from(errors).into());
    }

    Ok(OrdersForPayout {
        currency,
        orders: orders
            .into_iter()
            .map(|RawOrder { id, total_amount, .. }| OrderForPayout {
                order_id: id,
                total_amount,
            })
            .collect(),
    })
}
