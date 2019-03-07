use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Duration, NaiveDateTime};
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures::{Future, Stream};
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};

use failure::Fail;

use stq_http::client::HttpClient;
use stq_types::{StoreId, UserId};

use super::types::ServiceFutureV2;
use client::payments::{CreateInternalTransaction, PaymentsClient};
use client::stripe::{NewCharge, StripeClient};
use config::Subscription as SubscriptionConfig;
use controller::context::DynamicContext;
use controller::responses::SubscriptionPaymentSearchResponse;
use models::{
    Account, Amount, ChargeId, CurrencyChoice, DbCustomer, FiatCurrency, NewSubscriptionPayment, StoreSubscription,
    StoreSubscriptionSearch, Subscription, SubscriptionPaymentSearch, SubscriptionPaymentStatus, SubscriptionSearch, TransactionId,
    TureCurrency, UpdateSubscription,
};
use repos::repo_factory::ReposFactory;
use repos::{AccountsRepo, CustomersRepo, SearchCustomer, StoreSubscriptionRepo, SubscriptionRepo, UserRolesRepo};
use services::accounts::AccountService;
use services::types::{spawn_on_pool, ServiceResultV2};
use services::ErrorKind;

pub trait SubscriptionPaymentService {
    fn pay_subscriptions(&self) -> ServiceFutureV2<()>;
    fn search(&self, skip: i64, count: i64, payload: SubscriptionPaymentSearch) -> ServiceFutureV2<SubscriptionPaymentSearchResponse>;
}

pub struct SubscriptionPaymentServiceImpl<
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
    pub dynamic_context: DynamicContext<C, PC, AS>,
    pub stripe_client: Arc<dyn StripeClient>,
    pub config: SubscriptionConfig,
}

#[derive(Debug)]
struct FiatPaymentPreparation {
    fiat_currency: FiatCurrency,
    customer: DbCustomer,
    store_subscription: StoreSubscription,
    subscriptions: Vec<Subscription>,
    total_amount: Amount,
}

#[derive(Debug)]
struct CryptoPaymentPreparation {
    store_owner_account: Account,
    ture_currency: TureCurrency,
    store_subscription: StoreSubscription,
    subscriptions: Vec<Subscription>,
    total_amount: Amount,
}

struct FailedPaymentPreparation {
    store_subscription: StoreSubscription,
    subscriptions: Vec<Subscription>,
    total_amount: Amount,
}

enum PaymentPreparation {
    Fiat(FiatPaymentPreparation),
    Crypto(CryptoPaymentPreparation),
    Failed(FailedPaymentPreparation),
}

struct FinishedPayment {
    subscriptions: Vec<Subscription>,
    subscription_payment: NewSubscriptionPayment,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > SubscriptionPaymentService for SubscriptionPaymentServiceImpl<T, M, F, C, PC, AS>
{
    fn pay_subscriptions(&self) -> ServiceFutureV2<()> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        let now = chrono::offset::Utc::now().naive_utc();

        let payment_periodicity_duration = Duration::days(self.config.periodicity_days);

        let stripe_client = self.stripe_client.clone();

        let payments_client = match self.dynamic_context.payments_client.clone() {
            Some(payments_client) => payments_client,
            None => {
                let e = format_err!("Payments client was not found in dynamic context");
                return Box::new(futures::future::err(ectx!(err e, ErrorKind::Internal))) as ServiceFutureV2<()>;
            }
        };

        let accounts_service = match self.dynamic_context.account_service.clone() {
            Some(account_service) => account_service,
            None => {
                let e = format_err!("Accounts service was not found in dynamic context");
                return Box::new(futures::future::err(ectx!(err e, ErrorKind::Internal))) as ServiceFutureV2<()>;
            }
        };

        let fut = spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let store_subscription_repo = repo_factory.create_store_subscription_repo(&conn, user_id);
            let subscription_repo = repo_factory.create_subscription_repo(&conn, user_id);
            let user_role_repo = repo_factory.create_user_roles_repo(&conn, user_id);
            let customer_repo = repo_factory.create_customers_repo(&conn, user_id);
            let accounts_repo = repo_factory.create_accounts_repo_with_sys_acl(&conn);

            conn.transaction(move || {
                let subscriptions_by_stores = subscriptions_to_pay(&*subscription_repo, now, payment_periodicity_duration)?;

                create_payment_preparations(
                    &*store_subscription_repo,
                    &*accounts_repo,
                    &*customer_repo,
                    &*user_role_repo,
                    subscriptions_by_stores,
                )
            })
        })
        .map(futures::stream::iter_ok)
        .flatten_stream()
        .and_then(move |payment_preparation| match payment_preparation {
            PaymentPreparation::Crypto(crypto) => collect_ture_subscription(payments_client.clone(), accounts_service.clone(), crypto),
            PaymentPreparation::Fiat(fiat) => collect_fiat_subscription(stripe_client.clone(), fiat),
            PaymentPreparation::Failed(failed) => into_finished_payment(failed),
        })
        .collect()
        .and_then({
            let repo_factory = self.repo_factory.clone();
            let db_pool = self.db_pool.clone();
            let cpu_pool = self.cpu_pool.clone();
            move |finished_paymnets| {
                spawn_on_pool(db_pool, cpu_pool, move |conn| {
                    let subscription_payment_repo = repo_factory.create_subscription_payment_repo(&conn, user_id);
                    let subscription_repo = repo_factory.create_subscription_repo(&conn, user_id);
                    conn.transaction(move || {
                        for finished_paymnet in finished_paymnets {
                            let subscription_payment = subscription_payment_repo
                                .create(finished_paymnet.subscription_payment)
                                .map_err(ectx!(try convert))?;
                            let subscription_payment_id = subscription_payment.id;
                            for subscription in finished_paymnet.subscriptions {
                                let update_filter = SubscriptionSearch::by_id(subscription.id);
                                let update_payload = UpdateSubscription { subscription_payment_id };
                                subscription_repo
                                    .update(update_filter, update_payload)
                                    .map_err(ectx!(try convert))?;
                            }
                        }
                        Ok(())
                    })
                })
            }
        });

        Box::new(fut)
    }

    fn search(&self, skip: i64, count: i64, payload: SubscriptionPaymentSearch) -> ServiceFutureV2<SubscriptionPaymentSearchResponse> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let subscription_payment_repo = repo_factory.create_subscription_payment_repo(&conn, user_id);

            let resposne = subscription_payment_repo.search(skip, count, payload).map_err(ectx!(try convert))?;

            Ok(resposne.into())
        })
    }
}

fn create_payment_preparations(
    store_subscription_repo: &StoreSubscriptionRepo,
    accounts_repo: &AccountsRepo,
    customer_repo: &CustomersRepo,
    user_role_repo: &UserRolesRepo,
    subscriptions_by_stores: HashMap<StoreId, Vec<Subscription>>,
) -> ServiceResultV2<Vec<PaymentPreparation>> {
    let mut payment_preparations = Vec::new();
    for (store_id, subscriptions) in subscriptions_by_stores {
        info!(
            "subscription_payment: Ready to collect {} subscriptions from store {}",
            subscriptions.len(),
            store_id
        );
        let store_subscription = store_subscription_repo
            .get(StoreSubscriptionSearch::by_store_id(store_id))
            .map_err(ectx!(try convert))?
            .ok_or({
                let e = format_err!("Store {} does not have store subscription", store_id);
                ectx!(try err e, ErrorKind::Internal)
            })?;

        let total_amount = calculate_total_amount(&store_subscription, &subscriptions)?;

        let store_owner = user_role_repo
            .get_by_store_id(store_id)
            .map_err(ectx!(try convert))?
            .ok_or({
                let e = format_err!("Store {} does not have user roles entry", store_id);
                ectx!(try err e, ErrorKind::Internal)
            })?
            .user_id;

        let payment_preparation = payment_preparation(
            accounts_repo,
            customer_repo,
            store_subscription,
            subscriptions,
            store_owner,
            total_amount,
        )?;

        payment_preparations.push(payment_preparation)
    }

    Ok(payment_preparations)
}

fn payment_preparation(
    accounts_repo: &AccountsRepo,
    customer_repo: &CustomersRepo,
    store_subscription: StoreSubscription,
    subscriptions: Vec<Subscription>,
    store_owner: UserId,
    total_amount: Amount,
) -> ServiceResultV2<PaymentPreparation> {
    match store_subscription.currency.classify() {
        CurrencyChoice::Crypto(ture_currency) => {
            let wallet_address = match store_subscription.wallet_address.clone() {
                Some(wallet_address) => wallet_address,
                None => {
                    warn!(
                        "subscription_payment: User {} has no wallet addess in store subscription",
                        store_owner
                    );
                    return Ok(failed_payment_preparation(store_subscription, subscriptions, total_amount));
                }
            };

            let store_owner_account = match accounts_repo.get_by_wallet_address(wallet_address).map_err(ectx!(try convert))? {
                Some(store_owner_account) => store_owner_account,
                None => {
                    warn!("subscription_payment: Account with wallet address {} not found", store_owner);
                    return Ok(failed_payment_preparation(store_subscription, subscriptions, total_amount));
                }
            };

            Ok(PaymentPreparation::Crypto(CryptoPaymentPreparation {
                store_owner_account,
                ture_currency,
                store_subscription,
                subscriptions,
                total_amount,
            }))
        }
        CurrencyChoice::Fiat(fiat_currency) => {
            let customer = match customer_repo.get(SearchCustomer::UserId(store_owner)).map_err(ectx!(try convert))? {
                Some(customer) => customer,
                None => {
                    warn!("subscription_payment: User {} has no stripe customer", store_owner);
                    return Ok(failed_payment_preparation(store_subscription, subscriptions, total_amount));
                }
            };
            Ok(PaymentPreparation::Fiat(FiatPaymentPreparation {
                fiat_currency,
                customer,
                store_subscription,
                subscriptions,
                total_amount,
            }))
        }
    }
}

fn failed_payment_preparation(
    store_subscription: StoreSubscription,
    subscriptions: Vec<Subscription>,
    total_amount: Amount,
) -> PaymentPreparation {
    PaymentPreparation::Failed(FailedPaymentPreparation {
        store_subscription,
        subscriptions,
        total_amount,
    })
}

fn subscriptions_to_pay(
    subscription_repo: &SubscriptionRepo,
    now: NaiveDateTime,
    payment_periodicity_duration: Duration,
) -> ServiceResultV2<HashMap<StoreId, Vec<Subscription>>> {
    let unpaid_subscriptions = subscription_repo.get_unpaid().map_err(ectx!(try convert))?;

    let mut by_stores: HashMap<StoreId, Vec<Subscription>> = HashMap::new();
    for unpaid_subscription in unpaid_subscriptions {
        by_stores
            .entry(unpaid_subscription.store_id)
            .or_insert_with(Vec::new)
            .push(unpaid_subscription);
    }

    by_stores.retain(|_, unpaid_subscriptions| {
        let oldest_unpaid = unpaid_subscriptions.iter().map(|s| s.created_at).min().unwrap_or(now);
        now - oldest_unpaid > payment_periodicity_duration
    });

    Ok(by_stores)
}

fn collect_fiat_subscription(
    stripe_client: Arc<dyn StripeClient>,
    payment_preparation: FiatPaymentPreparation,
) -> ServiceFutureV2<FinishedPayment> {
    let new_charge = NewCharge {
        customer_id: payment_preparation.customer.id.clone(),
        amount: payment_preparation.total_amount,
        currency: payment_preparation.store_subscription.currency,
        capture: true,
    };

    let store_id = payment_preparation.store_subscription.store_id;

    let fut = stripe_client
        .create_charge(new_charge, None)
        .then(move |res| match res {
            Ok(charge) => Ok((Some(ChargeId::new(charge.id)), SubscriptionPaymentStatus::Paid)),
            Err(err) => {
                warn!(
                    "subscription_payment: Failed to collect subscription payment from {}: {}",
                    store_id, err
                );
                Ok((None, SubscriptionPaymentStatus::Failed))
            }
        })
        .map(|(charge_id, status)| FinishedPayment {
            subscription_payment: NewSubscriptionPayment {
                store_id: payment_preparation.store_subscription.store_id,
                amount: payment_preparation.total_amount,
                currency: payment_preparation.store_subscription.currency,
                charge_id,
                transaction_id: None,
                status,
            },
            subscriptions: payment_preparation.subscriptions,
        });

    Box::new(fut)
}

fn into_finished_payment(failed_payment_preparation: FailedPaymentPreparation) -> ServiceFutureV2<FinishedPayment> {
    Box::new(futures::future::ok(FinishedPayment {
        subscriptions: failed_payment_preparation.subscriptions,
        subscription_payment: NewSubscriptionPayment {
            store_id: failed_payment_preparation.store_subscription.store_id,
            amount: failed_payment_preparation.total_amount,
            currency: failed_payment_preparation.store_subscription.currency,
            charge_id: None,
            transaction_id: None,
            status: SubscriptionPaymentStatus::Failed,
        },
    }))
}

fn collect_ture_subscription<PC: PaymentsClient, AS: AccountService>(
    payments_client: PC,
    accounts_service: AS,
    payment_preparation: CryptoPaymentPreparation,
) -> ServiceFutureV2<FinishedPayment> {
    let transaction_id = TransactionId::generate();
    let store_id = payment_preparation.store_subscription.store_id;
    let fut = accounts_service
        .get_main_account(payment_preparation.ture_currency)
        .map(|account_with_balance| account_with_balance.account.id)
        .map({
            let from = payment_preparation.store_owner_account.id.inner().clone();
            let amount = payment_preparation.total_amount.clone();
            move |main_account_id| CreateInternalTransaction {
                id: transaction_id.inner().clone(),
                from,
                to: main_account_id.inner().clone(),
                amount,
            }
        })
        .and_then(move |transaction| payments_client.create_internal_transaction(transaction).map_err(ectx!(convert)))
        .then(move |res| match res {
            Ok(_) => Ok((transaction_id, SubscriptionPaymentStatus::Paid)),
            Err(err) => {
                warn!(
                    "subscription_payment: Failed to collect crypto subscription payment from {}: {}",
                    store_id, err
                );
                Ok((transaction_id, SubscriptionPaymentStatus::Failed))
            }
        })
        .map(move |(transaction_id, status)| FinishedPayment {
            subscription_payment: NewSubscriptionPayment {
                store_id,
                amount: payment_preparation.total_amount,
                currency: payment_preparation.store_subscription.currency,
                charge_id: None,
                transaction_id: Some(transaction_id),
                status,
            },
            subscriptions: payment_preparation.subscriptions,
        });

    Box::new(fut)
}

fn calculate_total_amount(store_subscription: &StoreSubscription, subscriptions: &[Subscription]) -> ServiceResultV2<Amount> {
    let mut total_amount = Amount::zero();
    for subscription in subscriptions {
        let subscription_amount = Amount::from(subscription.published_base_products_quantity)
            .checked_mul(store_subscription.value)
            .ok_or({
                let e = format_err!(
                    "Could not calculate total amount: checked multiplication error for store {}",
                    store_subscription.store_id
                );
                ectx!(try err e, ErrorKind::Internal)
            })?;
        total_amount = total_amount.checked_add(subscription_amount).ok_or({
            let e = format_err!(
                "Could not calculate total amount: checked addition error for store {}",
                store_subscription.store_id
            );
            ectx!(try err e, ErrorKind::Internal)
        })?;
    }
    Ok(total_amount)
}

#[cfg(test)]
mod tests {

    use super::*;

    use chrono::NaiveDate;

    use stq_types::{Quantity, SubscriptionId};

    use models::NewSubscription;
    use repos::types::RepoResultV2;

    struct SubscriptionRepoStub;

    impl SubscriptionRepo for SubscriptionRepoStub {
        fn create(&self, _new_subscription: NewSubscription) -> RepoResultV2<Subscription> {
            unimplemented!()
        }
        fn get(&self, _search: SubscriptionSearch) -> RepoResultV2<Option<Subscription>> {
            unimplemented!()
        }
        fn get_unpaid(&self) -> RepoResultV2<Vec<Subscription>> {
            Ok(vec![
                Subscription {
                    id: SubscriptionId(1),
                    store_id: StoreId(1),
                    published_base_products_quantity: Quantity(1),
                    subscription_payment_id: None,
                    created_at: NaiveDate::from_ymd(2019, 2, 9).and_hms(12, 0, 0),
                },
                Subscription {
                    id: SubscriptionId(2),
                    store_id: StoreId(1),
                    published_base_products_quantity: Quantity(1),
                    subscription_payment_id: None,
                    created_at: NaiveDate::from_ymd(2019, 2, 11).and_hms(12, 0, 0),
                },
                Subscription {
                    id: SubscriptionId(3),
                    store_id: StoreId(2),
                    published_base_products_quantity: Quantity(1),
                    subscription_payment_id: None,
                    created_at: NaiveDate::from_ymd(2019, 2, 10).and_hms(12, 0, 0),
                },
                Subscription {
                    id: SubscriptionId(4),
                    store_id: StoreId(3),
                    published_base_products_quantity: Quantity(1),
                    subscription_payment_id: None,
                    created_at: NaiveDate::from_ymd(2019, 2, 11).and_hms(12, 0, 0),
                },
            ])
        }
        fn search(&self, _search: SubscriptionSearch) -> RepoResultV2<Vec<Subscription>> {
            unimplemented!()
        }
        fn update(&self, _search: SubscriptionSearch, _payload: UpdateSubscription) -> RepoResultV2<Subscription> {
            unimplemented!()
        }
    }

    #[test]
    fn correctly_finds_subscriptions() {
        //given
        let subscription_repo = SubscriptionRepoStub;
        let now = NaiveDate::from_ymd(2019, 2, 11).and_hms(12, 0, 0);
        let payment_periodicity_duration = Duration::days(1);
        //when
        let subscriptions_to_pay =
            subscriptions_to_pay(&subscription_repo, now, payment_periodicity_duration).expect("subscriptions_to_pay failed");
        //then
        assert_eq!(
            subscriptions_to_pay
                .iter()
                .flat_map(|(_, s)| s.iter())
                .map(|s| s.id)
                .collect::<Vec<_>>(),
            vec![SubscriptionId(1), SubscriptionId(2)]
        );
    }
}
