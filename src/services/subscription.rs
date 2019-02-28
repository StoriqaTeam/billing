use chrono::Datelike;
use chrono::Duration;
use chrono::NaiveDateTime;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};

use failure::Fail;

use stq_http::client::HttpClient;
use stq_types::StoreId;

use super::types::ServiceFutureV2;
use client::payments::PaymentsClient;
use controller::context::DynamicContext;
use controller::requests::CreateSubscriptionsRequest;
use models::{
    Amount, Currency, NewStoreSubscription, StoreSubscription, StoreSubscriptionSearch, SubscriptionSearch, UpdateStoreSubscription,
};
use repos::repo_factory::ReposFactory;
use repos::types::RepoResultV2;
use repos::StoreSubscriptionRepo;
use services::accounts::AccountService;
use services::types::spawn_on_pool;
use services::ErrorKind;

const DEFAULT_CURRENCY: Currency = Currency::Usd;
const DEFAULT_AMOUNT: u128 = 3;
const TRIAL_TIME_DURATION_DAYS: i64 = 30;

pub trait SubscriptionService {
    fn create_all(&self, payload: CreateSubscriptionsRequest) -> ServiceFutureV2<()>;
}

pub struct SubscriptionServiceImpl<
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
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > SubscriptionService for SubscriptionServiceImpl<T, M, F, C, PC, AS>
{
    fn create_all(&self, payload: CreateSubscriptionsRequest) -> ServiceFutureV2<()> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        let now = chrono::offset::Utc::now().naive_utc();
        let max_trial_duration = Duration::days(TRIAL_TIME_DURATION_DAYS);

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let store_subscription_repo = repo_factory.create_store_subscription_repo(&conn, user_id);
            let subscription_repo = repo_factory.create_subscription_repo(&conn, user_id);

            conn.transaction(move || {
                'subscriptions: for new_subscription in payload.subscriptions {
                    let store_id = new_subscription.store_id;
                    let store_subscription =
                        find_update_or_create_store_subscription(&*store_subscription_repo, store_id, now).map_err(ectx!(try convert))?;
                    let trial_duration = store_subscription.trial_start_date.ok_or({
                        let e = format_err!("Store {} has empty trial start time", store_id);
                        ectx!(try err e, ErrorKind::Internal)
                    })? - now;

                    if trial_duration < max_trial_duration {
                        continue 'subscriptions;
                    }

                    let unpaid_store_subscriptions = subscription_repo
                        .search(SubscriptionSearch {
                            paid: Some(false),
                            store_id: Some(store_id),
                            ..Default::default()
                        })
                        .map_err(ectx!(try convert))?;

                    for unpaid_store_subscription in unpaid_store_subscriptions {
                        let old_created_at = unpaid_store_subscription.created_at;
                        if old_created_at.year() == now.year() && old_created_at.month() == now.month() && old_created_at.day() == now.day()
                        {
                            continue 'subscriptions;
                        }
                    }

                    subscription_repo.create(new_subscription).map_err(ectx!(try convert))?;
                }
                Ok(())
            })
        })
    }
}

fn find_update_or_create_store_subscription(
    store_subscription_repo: &StoreSubscriptionRepo,
    store_id: StoreId,
    now: NaiveDateTime,
) -> RepoResultV2<StoreSubscription> {
    let existing_store_subscription = store_subscription_repo.get(StoreSubscriptionSearch::by_store_id(store_id))?;
    if let Some(existing_store_subscription) = existing_store_subscription {
        if existing_store_subscription.trial_start_date.is_some() {
            return Ok(existing_store_subscription);
        }
        let update = UpdateStoreSubscription {
            trial_start_date: Some(now),
        };
        return store_subscription_repo.update(StoreSubscriptionSearch::by_store_id(store_id), update);
    }

    let new_store_subscription = NewStoreSubscription {
        store_id,
        currency: DEFAULT_CURRENCY,
        value: Amount::new(DEFAULT_AMOUNT),
        wallet_address: None,
    };

    store_subscription_repo.create(new_store_subscription)
}
