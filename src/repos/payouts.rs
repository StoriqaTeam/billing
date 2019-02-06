use diesel::{
    connection::{AnsiTransactionManager, Connection},
    pg::Pg,
    ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl,
};
use failure::{Error as FailureError, Fail};

use models::order_v2::{OrderId, StoreId};
use models::*;
use repos::legacy_acl::*;
use repos::user_roles::user_is_store_manager;
use schema::orders::dsl as Orders;
use schema::payouts::dsl as Payouts;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type PayoutsRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, PayoutAccess>>;

pub struct PayoutsRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: PayoutsRepoAcl,
}

pub trait PayoutsRepo {
    fn create(&self, payload: Payout) -> RepoResultV2<Payout>;
    fn get(&self, order_id: OrderId) -> RepoResultV2<Option<Payout>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> PayoutsRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: PayoutsRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> PayoutsRepo for PayoutsRepoImpl<'a, T> {
    fn create(&self, payload: Payout) -> RepoResultV2<Payout> {
        debug!("Creating a payout using payload: {:?}", payload);

        acl::check(
            &*self.acl,
            Resource::Payout,
            Action::Write,
            self,
            Some(&PayoutAccess::from(&payload)),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let raw_payout = RawPayout::from(payload);
        let command = diesel::insert_into(Payouts::payouts).values(&raw_payout);

        let raw_payout = command.get_result::<RawPayout>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(try err e, ErrorSource::Diesel, error_kind)
        })?;

        RawPayout::try_into_domain(raw_payout.clone()).map_err(ectx!(ErrorKind::Internal => raw_payout))
    }

    fn get(&self, order_id: OrderId) -> RepoResultV2<Option<Payout>> {
        debug!("Getting a payout with order ID: {}", order_id);

        let query = Payouts::payouts.filter(Payouts::order_id.eq(order_id));

        let raw_payout = query.get_result::<RawPayout>(self.db_conn).optional().map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(try err e, ErrorSource::Diesel, error_kind)
        })?;

        match raw_payout {
            Some(ref raw_payout) => {
                let payout = RawPayout::try_into_domain(raw_payout.clone()).map_err(ectx!(try ErrorKind::Internal => raw_payout))?;

                acl::check(&*self.acl, Resource::Payout, Action::Read, self, Some(&PayoutAccess::from(&payout)))
                    .map_err(ectx!(try ErrorKind::Forbidden))?;

                Ok(Some(payout))
            }
            None => Ok(None),
        }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, PayoutAccess>
    for PayoutsRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&PayoutAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(PayoutAccess { order_id }) = obj {
                    let query = Orders::orders.filter(Orders::id.eq(order_id)).select(Orders::store_id);

                    match query.get_result::<StoreId>(self.db_conn).optional() {
                        Ok(None) => true,
                        Ok(Some(store_id)) => user_is_store_manager(self.db_conn, user_id, store_id),
                        Err(_) => false,
                    }
                } else {
                    false
                }
            }
        }
    }
}
