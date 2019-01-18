use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::{err_msg, Fail};
use futures::{future, Future, IntoFuture, Stream};
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool, PooledConnection};
use uuid::Uuid;

use super::error::{Error, ErrorKind};
use super::types::ServiceFutureV2;
use client::payments::{Account as PaymentsAccount, CreateAccount, PaymentsClient};
use models::*;
use repos::repo_factory::ReposFactory;

pub trait AccountService: 'static {
    fn init_system_accounts(&self) -> ServiceFutureV2<()>;

    fn init_account_pools(&self) -> ServiceFutureV2<()>;

    fn get_account(&self, account_id: Uuid) -> ServiceFutureV2<AccountWithBalance>;

    fn get_main_account(&self, currency: TureCurrency) -> ServiceFutureV2<AccountWithBalance>;

    fn get_stq_cashback_account(&self) -> ServiceFutureV2<AccountWithBalance>;

    fn create_account(&self, account_id: Uuid, name: String, currency: TureCurrency, is_pooled: bool) -> ServiceFutureV2<Account>;

    fn get_or_create_free_pooled_account(&self, currency: TureCurrency) -> ServiceFutureV2<Account>;
}

pub struct AccountServiceImpl<T, M, F, PC>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
    PC: PaymentsClient,
{
    db_pool: Pool<M>,
    cpu_pool: CpuPool,
    repo_factory: F,
    min_accounts_in_pool: u32,
    payments_client: PC,
    payments_callback_url: String,
    system_accounts: SystemAccounts,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        PC: PaymentsClient + Clone,
    > Clone for AccountServiceImpl<T, M, F, PC>
{
    fn clone(&self) -> Self {
        Self {
            db_pool: self.db_pool.clone(),
            cpu_pool: self.cpu_pool.clone(),
            repo_factory: self.repo_factory.clone(),
            min_accounts_in_pool: self.min_accounts_in_pool.clone(),
            payments_client: self.payments_client.clone(),
            payments_callback_url: self.payments_callback_url.clone(),
            system_accounts: self.system_accounts.clone(),
        }
    }
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        PC: PaymentsClient + Clone,
    > AccountService for AccountServiceImpl<T, M, F, PC>
{
    fn init_system_accounts(&self) -> ServiceFutureV2<()> {
        let fut = self
            .clone()
            .spawn_on_pool({
                let system_account_ids = self.system_accounts.0.clone().into_iter().map(|acc| acc.id).collect::<Vec<_>>();
                let repo_factory = self.repo_factory.clone();

                move |conn| {
                    let accounts_repo = repo_factory.create_accounts_repo_with_sys_acl(&conn);
                    accounts_repo
                        .get_many(&system_account_ids)
                        .map_err(ectx!(convert => system_account_ids))
                }
            })
            .and_then({
                let self_clone = self.clone();
                let system_accounts = self.system_accounts.0.clone();
                move |existing_accounts| {
                    let missing_accounts = system_accounts
                        .into_iter()
                        .filter(move |acc| !existing_accounts.iter().any(|existing_acc| existing_acc.id == acc.id));

                    futures::stream::iter_ok::<_, Error>(missing_accounts)
                        .fold(self_clone, |self_, account| {
                            let name = account.to_string();
                            let SystemAccount { id, currency, .. } = account;

                            self_
                                .clone()
                                .create_account(id.inner().clone(), name.clone(), currency, false)
                                .map(move |_| self_)
                                .map_err(ectx!(try ErrorKind::Internal => id.to_string(), name, currency, false))
                        })
                        .map(|_| ())
                }
            });

        Box::new(fut)
    }

    fn init_account_pools(&self) -> ServiceFutureV2<()> {
        let fut = self
            .clone()
            .spawn_on_pool({
                let repo_factory = self.repo_factory.clone();
                move |conn| {
                    let accounts_repo = repo_factory.create_accounts_repo_with_sys_acl(&conn);
                    accounts_repo.count().map_err(ectx!(convert))
                }
            })
            .and_then({
                let self_clone = self.clone();
                let min_accounts_in_pool = self.min_accounts_in_pool.clone();
                move |account_count| {
                    let accounts_to_create = account_count
                        .pooled
                        .into_iter()
                        .filter_map(move |(currency, num_existing)| {
                            (min_accounts_in_pool as u64)
                                .checked_sub(num_existing)
                                .filter(|num_to_create| *num_to_create > 0)
                                .map(|num_to_create| (currency, num_to_create))
                        })
                        .flat_map(|(currency, num_to_create)| (0..num_to_create).map(move |_| currency));

                    futures::stream::iter_ok::<_, Error>(accounts_to_create)
                        .fold(self_clone, |self_, currency| {
                            let account_id = Uuid::new_v4();
                            self_
                                .clone()
                                .create_account(account_id.clone(), account_id.hyphenated().to_string(), currency, true)
                                .map(move |_| self_)
                                .map_err(ectx!(try ErrorKind::Internal => account_id.hyphenated().to_string(), currency, true))
                        })
                        .map(|_| ())
                }
            });

        Box::new(fut)
    }

    fn get_account(&self, account_id: Uuid) -> ServiceFutureV2<AccountWithBalance> {
        let fut = self
            .spawn_on_pool({
                let repo_factory = self.repo_factory.clone();
                move |conn| {
                    let account_repo = repo_factory.create_accounts_repo_with_sys_acl(&conn);
                    let account = account_repo
                        .get(AccountId::new(account_id))
                        .map_err(ectx!(try ErrorKind::Internal => account_id))?;

                    account.ok_or({
                        let e = format_err!("Account {} not found", account_id);
                        ectx!(err e, ErrorKind::Internal)
                    })
                }
            })
            .and_then({
                let payments_client = self.payments_client.clone();
                move |account| {
                    payments_client
                        .get_account(account_id)
                        .map(move |PaymentsAccount { balance, .. }| AccountWithBalance { account, balance })
                        .map_err(ectx!(ErrorKind::Internal => account_id.hyphenated().to_string()))
                }
            });

        Box::new(fut)
    }

    fn get_main_account(&self, currency: TureCurrency) -> ServiceFutureV2<AccountWithBalance> {
        let fut = self
            .system_accounts
            .get(currency, SystemAccountType::Main)
            .ok_or({
                let e = format_err!("Main system account for currency {} is missing", currency);
                ectx!(err e, ErrorKind::Internal)
            })
            .into_future()
            .and_then({
                let self_ = self.clone();
                move |account_id| self_.get_account(account_id.into_inner())
            });

        Box::new(fut)
    }

    fn get_stq_cashback_account(&self) -> ServiceFutureV2<AccountWithBalance> {
        let fut = self
            .system_accounts
            .get(TureCurrency::Stq, SystemAccountType::Cashback)
            .ok_or({
                let e = err_msg("STQ cashback system account is missing");
                ectx!(err e, ErrorKind::Internal)
            })
            .into_future()
            .and_then({
                let self_ = self.clone();
                move |account_id| self_.get_account(account_id.into_inner())
            });

        Box::new(fut)
    }

    fn create_account(&self, account_id: Uuid, name: String, currency: TureCurrency, is_pooled: bool) -> ServiceFutureV2<Account> {
        Box::new(self.create_account_happy(account_id, name, currency, is_pooled).or_else({
            let self_clone = self.clone();
            move |(account_id, error)| self_clone.create_account_revert(account_id).then(|_| Err(error))
        }))
    }

    fn get_or_create_free_pooled_account(&self, currency: TureCurrency) -> ServiceFutureV2<Account> {
        let fut = self
            .spawn_on_pool({
                let repo_factory = self.repo_factory.clone();
                move |conn| {
                    let account_repo = repo_factory.create_accounts_repo_with_sys_acl(&conn);
                    account_repo
                        .get_free_account(currency)
                        .map_err(ectx!(ErrorKind::Internal => currency))
                }
            })
            .and_then({
                let self_clone = self.clone();
                move |free_account| match free_account {
                    Some(free_account) => future::Either::A(future::ok(free_account)),
                    None => {
                        let id = Uuid::new_v4();
                        let name = id.hyphenated().to_string();
                        future::Either::B(
                            self_clone
                                .create_account(id.clone(), name.clone(), currency, true)
                                .map_err(ectx!(convert => id, name, currency, true)),
                        )
                    }
                }
            });

        Box::new(fut)
    }
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        PC: PaymentsClient + Clone,
    > AccountServiceImpl<T, M, F, PC>
{
    pub fn new(
        db_pool: Pool<M>,
        cpu_pool: CpuPool,
        repo_factory: F,
        min_accounts_in_pool: u32,
        payments_client: PC,
        payments_callback_url: String,
        system_accounts: SystemAccounts,
    ) -> Self {
        Self {
            db_pool,
            cpu_pool,
            repo_factory,
            min_accounts_in_pool,
            payments_client,
            payments_callback_url,
            system_accounts,
        }
    }

    fn create_account_happy(
        &self,
        account_id: Uuid,
        name: String,
        currency: TureCurrency,
        is_pooled: bool,
    ) -> Box<Future<Item = Account, Error = (Uuid, Error)>> {
        let input = CreateAccount {
            id: account_id,
            currency,
            name,
            callback_url: self.payments_callback_url.clone(),
        };

        Box::new(
            self.payments_client
                .create_account(input.clone())
                .map_err({
                    let account_id = account_id.clone();
                    move |e| (account_id, ectx!(convert err e => input))
                })
                .and_then({
                    let account_id = account_id.clone();
                    let repo_factory = self.repo_factory.clone();
                    let self_clone = self.clone();

                    move |PaymentsAccount { account_address, .. }| {
                        self_clone
                            .spawn_on_pool(move |conn| {
                                let accounts_repo = repo_factory.create_accounts_repo_with_sys_acl(&conn);
                                let new_account = NewAccount {
                                    id: AccountId::new(account_id),
                                    currency,
                                    is_pooled,
                                    wallet_address: Some(account_address),
                                };
                                accounts_repo.create(new_account.clone()).map_err(ectx!(convert => new_account))
                            })
                            .map_err(move |e| (account_id, e))
                    }
                }),
        )
    }

    fn create_account_revert(&self, account_id: Uuid) -> ServiceFutureV2<()> {
        let fut1 = self
            .spawn_on_pool({
                let repo_factory = self.repo_factory.clone();
                move |conn| {
                    let accounts_repo = repo_factory.create_accounts_repo_with_sys_acl(&conn);
                    accounts_repo.delete(AccountId::new(account_id)).map_err(ectx!(convert))
                }
            })
            .then(|_| Ok(()));

        let fut2 = self
            .payments_client
            .clone()
            .delete_account(account_id)
            .map_err(ectx!(convert))
            .then(|_: Result<_, Error>| Ok(()));

        Box::new(Future::join(fut1, fut2).map(|_| ()))
    }

    fn spawn_on_pool<R, Func>(&self, f: Func) -> ServiceFutureV2<R>
    where
        Func: FnOnce(PooledConnection<M>) -> Result<R, Error> + Send + 'static,
        R: Send + 'static,
    {
        let cpu_pool = self.cpu_pool.clone();
        let db_pool = self.db_pool.clone();
        Box::new(cpu_pool.spawn_fn(move || db_pool.get().map_err(ectx!(ErrorKind::Internal)).and_then(f)))
    }
}
