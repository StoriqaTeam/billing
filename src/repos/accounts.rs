use diesel::{connection::AnsiTransactionManager, pg::Pg, prelude::*, query_dsl::RunQueryDsl, Connection};
use failure::{Error as FailureError, Fail};

use stq_types::UserId;

use models::{authorization::*, Account, AccountId, NewAccount, RawAccount};
use repos::{
    acl,
    error::{ErrorKind, ErrorSource},
    legacy_acl::*,
    types::RepoResult,
};
use schema::accounts::dsl as Accounts;

pub struct AccountsRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: Box<Acl<Resource, Action, Scope, FailureError, Account>>,
}

pub trait AccountsRepo {
    fn get(&self, account_id: AccountId) -> RepoResult<Option<Account>>;
    fn create(&self, payload: NewAccount) -> RepoResult<Account>;
    fn delete(&self, account_id: AccountId) -> RepoResult<Account>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> AccountsRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: Box<Acl<Resource, Action, Scope, FailureError, Account>>) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> AccountsRepo for AccountsRepoImpl<'a, T> {
    fn get(&self, account_id: AccountId) -> RepoResult<Option<Account>> {
        debug!("Getting an account with ID: {}", account_id);

        acl::check(&*self.acl, Resource::Account, Action::Read, self, None)?;

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

    fn create(&self, payload: NewAccount) -> RepoResult<Account> {
        debug!("Creating an account using payload: {:?}", payload);

        acl::check(&*self.acl, Resource::Account, Action::Write, self, None)?;

        let command = diesel::insert_into(Accounts::accounts).values(&payload);

        command.get_result::<RawAccount>(self.db_conn).map(Account::from).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind => payload)
        })
    }

    fn delete(&self, account_id: AccountId) -> RepoResult<Account> {
        debug!("Deleting an account with ID: {}", account_id);

        acl::check(&*self.acl, Resource::Account, Action::Write, self, None)?;

        let command = diesel::delete(Accounts::accounts.filter(Accounts::id.eq(account_id)));

        command.get_result::<RawAccount>(self.db_conn).map(Account::from).map_err(|e| {
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
