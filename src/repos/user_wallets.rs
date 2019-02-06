use diesel::{
    connection::{AnsiTransactionManager, Connection},
    pg::Pg,
    ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl,
};
use failure::{Error as FailureError, Fail};

use models::*;
use repos::legacy_acl::*;
use schema::user_wallets::dsl as UserWallets;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type UserWalletsRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, UserWalletAccess>>;

pub struct UserWalletsRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: UserWalletsRepoAcl,
}

pub trait UserWalletsRepo {
    fn add(&self, payload: NewActiveUserWallet) -> RepoResultV2<UserWallet>;
    fn get(&self, id: UserWalletId) -> RepoResultV2<Option<UserWallet>>;
    fn get_currency_wallets_by_user_id(&self, currency: TureCurrency, user_id: UserId) -> RepoResultV2<Vec<UserWallet>>;
    fn deactivate(&self, id: UserWalletId) -> RepoResultV2<UserWallet>;
    fn deactivate_wallets_by_user_id(&self, user_id: UserId) -> RepoResultV2<Vec<UserWallet>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> UserWalletsRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: UserWalletsRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> UserWalletsRepo
    for UserWalletsRepoImpl<'a, T>
{
    fn add(&self, payload: NewActiveUserWallet) -> RepoResultV2<UserWallet> {
        debug!("Adding a user wallet using payload: {:?}", payload);

        acl::check(
            &*self.acl,
            Resource::UserWallet,
            Action::Write,
            self,
            Some(&UserWalletAccess::from(&payload)),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let insert_user_wallet = InsertUserWallet::from(payload);

        let command = diesel::insert_into(UserWallets::user_wallets).values(&insert_user_wallet);

        command
            .get_result::<RawUserWallet>(self.db_conn)
            .map(UserWallet::from)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
    }

    fn get(&self, user_wallet_id: UserWalletId) -> RepoResultV2<Option<UserWallet>> {
        debug!("Getting a user wallet with ID: {}", user_wallet_id);

        let query = UserWallets::user_wallets.filter(UserWallets::id.eq(user_wallet_id));

        query
            .get_result::<RawUserWallet>(self.db_conn)
            .map(UserWallet::from)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|user_wallet| {
                if let Some(ref user_wallet) = user_wallet {
                    acl::check(
                        &*self.acl,
                        Resource::UserWallet,
                        Action::Read,
                        self,
                        Some(&UserWalletAccess::from(user_wallet)),
                    )
                    .map_err(ectx!(try ErrorKind::Forbidden))?;
                };
                Ok(user_wallet)
            })
    }

    fn get_currency_wallets_by_user_id(&self, currency: TureCurrency, user_id: UserId) -> RepoResultV2<Vec<UserWallet>> {
        debug!("Getting user wallets for currency {} with user ID: {}", currency, user_id);

        acl::check(
            &*self.acl,
            Resource::UserWallet,
            Action::Read,
            self,
            Some(&UserWalletAccess { user_id }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let query = UserWallets::user_wallets
            .filter(UserWallets::currency.eq(currency))
            .filter(UserWallets::user_id.eq(user_id))
            .filter(UserWallets::is_active.eq(true));

        query
            .get_results::<RawUserWallet>(self.db_conn)
            .map(|raw_user_wallets| raw_user_wallets.into_iter().map(UserWallet::from).collect::<Vec<_>>())
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
    }

    fn deactivate(&self, user_wallet_id: UserWalletId) -> RepoResultV2<UserWallet> {
        debug!("Deactivating a user wallet with ID: {}", user_wallet_id);

        let user_id = UserWallets::user_wallets
            .filter(UserWallets::id.eq(user_wallet_id))
            .select(UserWallets::user_id)
            .get_result::<UserId>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        acl::check(
            &*self.acl,
            Resource::UserWallet,
            Action::Write,
            self,
            Some(&UserWalletAccess { user_id }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command =
            diesel::update(UserWallets::user_wallets.filter(UserWallets::id.eq(user_wallet_id))).set(UserWallets::is_active.eq(false));

        command
            .get_result::<RawUserWallet>(self.db_conn)
            .map(UserWallet::from)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
    }

    fn deactivate_wallets_by_user_id(&self, user_id: UserId) -> RepoResultV2<Vec<UserWallet>> {
        debug!("Deactivating wallets for user with ID: {}", user_id);

        acl::check(
            &*self.acl,
            Resource::UserWallet,
            Action::Write,
            self,
            Some(&UserWalletAccess { user_id }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command =
            diesel::update(UserWallets::user_wallets.filter(UserWallets::user_id.eq(user_id))).set(UserWallets::is_active.eq(false));

        command
            .get_results::<RawUserWallet>(self.db_conn)
            .map(|raw_user_wallets| raw_user_wallets.into_iter().map(UserWallet::from).collect::<Vec<_>>())
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, UserWalletAccess>
    for UserWalletsRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&UserWalletAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(UserWalletAccess {
                    user_id: user_wallet_user_id,
                }) = obj
                {
                    user_id.0 == user_wallet_user_id.inner()
                } else {
                    false
                }
            }
        }
    }
}
