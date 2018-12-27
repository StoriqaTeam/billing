use diesel::{connection::AnsiTransactionManager, pg::Pg, prelude::*, query_dsl::RunQueryDsl, Connection};
use enum_iterator::IntoEnumIterator;
use failure::{Error as FailureError, Fail};
use std::collections::HashMap;
use stq_types::UserId;

use models::invoice_v2::RawInvoice;
use models::{authorization::*, Account, AccountCount, AccountId, Currency, NewAccount, RawAccount};
use repos::{
    acl,
    error::{ErrorKind, ErrorSource},
    legacy_acl::*,
    types::RepoResultV2,
};
use schema::accounts::dsl as Accounts;
use schema::invoices_v2::dsl as InvoicesV2;

pub struct AccountsRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: Box<Acl<Resource, Action, Scope, FailureError, Account>>,
}

pub trait AccountsRepo {
    fn count(&self) -> RepoResultV2<AccountCount>;
    fn get(&self, account_id: AccountId) -> RepoResultV2<Option<Account>>;
    fn get_many(&self, account_ids: &[AccountId]) -> RepoResultV2<Vec<Account>>;
    fn get_free_account(&self, currency: Currency) -> RepoResultV2<Option<Account>>;
    fn create(&self, payload: NewAccount) -> RepoResultV2<Account>;
    fn delete(&self, account_id: AccountId) -> RepoResultV2<Option<Account>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> AccountsRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: Box<Acl<Resource, Action, Scope, FailureError, Account>>) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> AccountsRepo for AccountsRepoImpl<'a, T> {
    fn count(&self) -> RepoResultV2<AccountCount> {
        debug!("Getting account count");

        acl::check(&*self.acl, Resource::Account, Action::Read, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let query = Accounts::accounts.select((Accounts::currency, Accounts::is_pooled));
        let accounts = query.get_results::<(Currency, bool)>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(try err e, ErrorSource::Diesel, error_kind)
        })?;

        // add initial zero counts for every currency to simplify account pool initialization logic
        let empty_hashmap = Currency::into_enum_iter().map(|currency| (currency, 0)).collect::<HashMap<_, _>>();

        let account_count = accounts.into_iter().fold(
            AccountCount {
                pooled: empty_hashmap.clone(),
                unpooled: empty_hashmap,
            },
            |mut account_count, (currency, is_pooled)| {
                if is_pooled {
                    account_count.pooled.entry(currency).and_modify(|count| *count += 1).or_insert(1);
                } else {
                    account_count.unpooled.entry(currency).and_modify(|count| *count += 1).or_insert(1);
                };
                account_count
            },
        );

        Ok(account_count)
    }

    fn get(&self, account_id: AccountId) -> RepoResultV2<Option<Account>> {
        debug!("Getting an account with ID: {}", account_id);

        acl::check(&*self.acl, Resource::Account, Action::Read, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let query = Accounts::accounts.filter(Accounts::id.eq(account_id));

        query
            .get_result::<RawAccount>(self.db_conn)
            .map(Account::from)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind => account_id)
            })
    }

    fn get_many(&self, account_ids: &[AccountId]) -> RepoResultV2<Vec<Account>> {
        debug!("Getting accounts with IDs: {:?}", account_ids);

        acl::check(&*self.acl, Resource::Account, Action::Read, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let query = Accounts::accounts.filter(Accounts::id.eq_any(account_ids));

        query
            .get_results::<RawAccount>(self.db_conn)
            .map(|raw_accounts| raw_accounts.into_iter().map(Account::from).collect())
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind => account_ids)
            })
    }

    fn get_free_account(&self, currency: Currency) -> RepoResultV2<Option<Account>> {
        debug!("Getting a free account for currency: {:?}", currency);

        acl::check(&*self.acl, Resource::Account, Action::Read, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let query = Accounts::accounts
            .filter(Accounts::currency.eq(currency).and(Accounts::is_pooled.eq(true)))
            .left_join(InvoicesV2::invoices_v2)
            .filter(InvoicesV2::id.is_null());

        query
            .get_result::<(RawAccount, Option<RawInvoice>)>(self.db_conn)
            .map(|(raw_account, _)| Account::from(raw_account))
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind => currency)
            })
    }

    fn create(&self, payload: NewAccount) -> RepoResultV2<Account> {
        debug!("Creating an account using payload: {:?}", payload);

        acl::check(&*self.acl, Resource::Account, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(Accounts::accounts).values(&payload);

        command.get_result::<RawAccount>(self.db_conn).map(Account::from).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind => payload)
        })
    }

    fn delete(&self, account_id: AccountId) -> RepoResultV2<Option<Account>> {
        debug!("Deleting an account with ID: {}", account_id);

        acl::check(&*self.acl, Resource::Account, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::delete(Accounts::accounts.filter(Accounts::id.eq(account_id)));

        command
            .get_result::<RawAccount>(self.db_conn)
            .map(Account::from)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind => account_id)
            })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, Account>
    for AccountsRepoImpl<'a, T>
{
    fn is_in_scope(&self, _user_id: UserId, _scope: &Scope, _obj: Option<&Account>) -> bool {
        true
    }
}
