use chrono::Utc;
use diesel::{
    connection::{AnsiTransactionManager, Connection},
    expression::dsl::any,
    pg::Pg,
    ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl,
};
use failure::{Error as FailureError, Fail};
use itertools::Itertools;
use std::collections::HashMap;

use models::order_v2::OrderId;
use models::*;
use repos::legacy_acl::*;
use schema::order_payouts::dsl as OrderPayouts;
use schema::payouts::dsl as Payouts;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type PayoutsRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, PayoutAccess>>;

pub trait PayoutsRepo {
    fn create(&self, payout: Payout) -> RepoResultV2<Payout>;
    fn get(&self, id: PayoutId) -> RepoResultV2<Option<Payout>>;
    fn get_by_order_id(&self, order_id: OrderId) -> RepoResultV2<Option<Payout>>;
    fn get_by_order_ids(&self, order_ids: &[OrderId]) -> RepoResultV2<PayoutsByOrderIds>;
    fn mark_as_completed(&self, id: PayoutId) -> RepoResultV2<Payout>;
}

pub struct PayoutsRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: PayoutsRepoAcl,
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> PayoutsRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: PayoutsRepoAcl) -> Self {
        Self { db_conn, acl }
    }

    fn get_payout_by_id(&self, id: PayoutId) -> RepoResultV2<Option<Payout>> {
        let raw_payout_records = self
            .db_conn
            .transaction(move || {
                let raw_payout = Payouts::payouts
                    .filter(Payouts::id.eq(id))
                    .get_result::<RawPayout>(self.db_conn)
                    .optional()?;

                match raw_payout {
                    None => Ok(None),
                    Some(raw_payout) => {
                        let raw_order_payouts = OrderPayouts::order_payouts
                            .filter(OrderPayouts::payout_id.eq(raw_payout.id))
                            .get_results::<RawOrderPayout>(self.db_conn)?;

                        Ok(Some(RawPayoutRecords {
                            raw_payout,
                            raw_order_payouts,
                        }))
                    }
                }
            })
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        match raw_payout_records {
            None => Ok(None),
            Some(raw_payout_records) => raw_payout_records
                .clone()
                .try_into_domain()
                .map(Some)
                .map_err(ectx!(ErrorKind::Internal => raw_payout_records)),
        }
    }

    fn get_payout_by_order_id(&self, order_id: OrderId) -> RepoResultV2<Option<Payout>> {
        let raw_payout_records = self
            .db_conn
            .transaction(move || {
                let payout_id = OrderPayouts::order_payouts
                    .filter(OrderPayouts::order_id.eq(order_id))
                    .select(OrderPayouts::payout_id)
                    .get_result::<PayoutId>(self.db_conn)
                    .optional()?;

                match payout_id {
                    Some(payout_id) => {
                        let raw_payout = Payouts::payouts
                            .filter(Payouts::id.eq(payout_id))
                            .get_result::<RawPayout>(self.db_conn)?;

                        let raw_order_payouts = OrderPayouts::order_payouts
                            .filter(OrderPayouts::payout_id.eq(payout_id))
                            .get_results::<RawOrderPayout>(self.db_conn)?;

                        Ok(Some(RawPayoutRecords {
                            raw_payout,
                            raw_order_payouts,
                        }))
                    }
                    None => Ok(None),
                }
            })
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        match raw_payout_records {
            None => Ok(None),
            Some(raw_payout_records) => raw_payout_records
                .clone()
                .try_into_domain()
                .map(Some)
                .map_err(ectx!(ErrorKind::Internal => raw_payout_records)),
        }
    }

    fn get_payouts_by_order_ids(&self, order_ids: &[OrderId]) -> RepoResultV2<PayoutsByOrderIds> {
        if order_ids.is_empty() {
            return Ok(PayoutsByOrderIds {
                payouts: HashMap::default(),
                order_ids_without_payout: Vec::default(),
            });
        }

        let records = OrderPayouts::order_payouts
            .filter(OrderPayouts::order_id.eq(any(order_ids)))
            .inner_join(Payouts::payouts)
            .get_results::<(RawOrderPayout, RawPayout)>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        let order_ids_without_payout = order_ids
            .iter()
            .filter(|order_id| !records.iter().any(|(order_payout, _payout)| order_payout.order_id == **order_id))
            .cloned()
            .collect::<Vec<_>>();

        records
            .into_iter()
            .group_by(|(_raw_order_payout, raw_payout)| raw_payout.clone())
            .into_iter()
            .try_fold(HashMap::default(), |mut acc, (raw_payout, group)| {
                let payout_records = RawPayoutRecords {
                    raw_payout,
                    raw_order_payouts: group.map(|(raw_order_payout, _raw_payout)| raw_order_payout).collect(),
                };
                let payout = payout_records.try_into_domain().map_err(ectx!(try ErrorKind::Internal))?;
                let entries = payout.order_ids.clone().into_iter().map(|order_id| (order_id, payout.clone()));
                acc.extend(entries);
                Ok(acc)
            })
            .map(|payouts| PayoutsByOrderIds {
                payouts,
                order_ids_without_payout,
            })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> PayoutsRepo for PayoutsRepoImpl<'a, T> {
    fn create(&self, payout: Payout) -> RepoResultV2<Payout> {
        debug!("Creating a payout using payload: {:?}", payout);

        acl::check(
            &*self.acl,
            Resource::Payout,
            Action::Write,
            self,
            Some(&PayoutAccess::from(&payout)),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let RawNewPayoutRecords {
            raw_new_payout,
            raw_new_order_payouts,
        } = RawNewPayoutRecords::from(payout);

        let insert_payout_command = diesel::insert_into(Payouts::payouts).values(&raw_new_payout);
        let insert_order_payouts_command = diesel::insert_into(OrderPayouts::order_payouts).values(&raw_new_order_payouts);

        let raw_payout_records = self
            .db_conn
            .transaction(move || {
                let raw_payout = insert_payout_command.get_result::<RawPayout>(self.db_conn)?;
                let raw_order_payouts = insert_order_payouts_command.get_results::<RawOrderPayout>(self.db_conn)?;
                Ok(RawPayoutRecords {
                    raw_payout,
                    raw_order_payouts,
                })
            })
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        raw_payout_records
            .clone()
            .try_into_domain()
            .map_err(ectx!(ErrorKind::Internal => raw_payout_records))
    }

    fn get(&self, id: PayoutId) -> RepoResultV2<Option<Payout>> {
        debug!("Getting a payout by ID: {}", id);

        let payout = self.get_payout_by_id(id)?;

        match payout {
            None => Ok(None),
            Some(payout) => acl::check(&*self.acl, Resource::Payout, Action::Read, self, Some(&PayoutAccess::from(&payout)))
                .map(|_| Some(payout))
                .map_err(ectx!(ErrorKind::Forbidden)),
        }
    }

    fn get_by_order_id(&self, order_id: OrderId) -> RepoResultV2<Option<Payout>> {
        debug!("Getting a payout by order ID: {}", order_id);

        let payout = self.get_payout_by_order_id(order_id)?;

        match payout {
            None => Ok(None),
            Some(payout) => acl::check(&*self.acl, Resource::Payout, Action::Read, self, Some(&PayoutAccess::from(&payout)))
                .map(|_| Some(payout))
                .map_err(ectx!(ErrorKind::Forbidden)),
        }
    }

    fn mark_as_completed(&self, id: PayoutId) -> RepoResultV2<Payout> {
        debug!("Mark payout with ID: {} as completed", id);

        let user_id = Payouts::payouts
            .filter(Payouts::id.eq(id))
            .select(Payouts::user_id)
            .get_result::<UserId>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        acl::check(&*self.acl, Resource::Payout, Action::Write, self, Some(&PayoutAccess { user_id }))
            .map_err(ectx!(try ErrorKind::Forbidden))?;

        let now = Utc::now().naive_utc();

        diesel::update(Payouts::payouts)
            .set(Payouts::completed_at.eq(now))
            .execute(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        self.get_payout_by_id(id)?.ok_or({
            let e = format_err!("Payout with ID {} not found after update", id);
            ectx!(err e, ErrorKind::Internal)
        })
    }

    fn get_by_order_ids(&self, order_ids: &[OrderId]) -> RepoResultV2<PayoutsByOrderIds> {
        let ids_string = order_ids.iter().map(OrderId::to_string).collect::<Vec<_>>().join(", ");
        debug!("Get payouts by order IDs: {}", ids_string);

        let payouts_by_order_ids = self.get_payouts_by_order_ids(order_ids)?;

        for payout in payouts_by_order_ids.payouts.iter().map(|(_order_id, payout)| payout) {
            acl::check(&*self.acl, Resource::Payout, Action::Read, self, Some(&PayoutAccess::from(payout)))
                .map_err(ectx!(try ErrorKind::Forbidden))?;
        }

        Ok(payouts_by_order_ids)
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, PayoutAccess>
    for PayoutsRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&PayoutAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(PayoutAccess { user_id: payout_user_id }) = obj {
                    payout_user_id.inner() == user_id.0
                } else {
                    false
                }
            }
        }
    }
}
