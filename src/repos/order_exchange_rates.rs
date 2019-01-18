use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;

use repos::legacy_acl::*;

use models::authorization::*;
use models::order_exchange_rate::{
    ExchangeRateStatus, LatestExchangeRates, NewOrderExchangeRate, OrderExchangeRateAccess, OrderExchangeRateId, RawNewOrderExchangeRate,
    RawOrderExchangeRate, SetExchangeRateStatus,
};
use models::order_v2::OrderId;
use models::UserId;

use schema::invoices_v2::dsl as InvoicesV2;
use schema::order_exchange_rates::dsl as OrderExchangeRates;
use schema::orders::dsl as Orders;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type OrderExchangeRatesRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, OrderExchangeRateAccess>>;

pub struct OrderExchangeRatesRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: OrderExchangeRatesRepoAcl,
}

pub trait OrderExchangeRatesRepo {
    fn get(&self, rate_id: OrderExchangeRateId) -> RepoResultV2<Option<RawOrderExchangeRate>>;
    fn get_active_rate_for_order(&self, order_id: OrderId) -> RepoResultV2<Option<RawOrderExchangeRate>>;
    fn get_all_rates_for_order(&self, order_id: OrderId) -> RepoResultV2<Vec<RawOrderExchangeRate>>;
    fn add_new_active_rate(&self, new_rate: NewOrderExchangeRate) -> RepoResultV2<LatestExchangeRates>;
    fn expire_current_active_rate(&self, order_id: OrderId) -> RepoResultV2<Option<RawOrderExchangeRate>>;
    fn delete(&self, rate_id: OrderExchangeRateId) -> RepoResultV2<Option<RawOrderExchangeRate>>;
    fn delete_by_order_id(&self, order_id: OrderId) -> RepoResultV2<Vec<RawOrderExchangeRate>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> OrderExchangeRatesRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: OrderExchangeRatesRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> OrderExchangeRatesRepo
    for OrderExchangeRatesRepoImpl<'a, T>
{
    fn get(&self, rate_id: OrderExchangeRateId) -> RepoResultV2<Option<RawOrderExchangeRate>> {
        debug!("Getting a rate with ID: {}", rate_id);

        let query = OrderExchangeRates::order_exchange_rates.filter(OrderExchangeRates::id.eq(rate_id));

        query
            .get_result::<RawOrderExchangeRate>(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|rate| {
                if let Some(ref rate) = rate {
                    acl::check(
                        &*self.acl,
                        Resource::OrderExchangeRate,
                        Action::Read,
                        self,
                        Some(&OrderExchangeRateAccess::from(rate.clone())),
                    )
                    .map_err(ectx!(try ErrorKind::Forbidden))?;
                };
                Ok(rate)
            })
    }

    fn get_active_rate_for_order(&self, order_id: OrderId) -> RepoResultV2<Option<RawOrderExchangeRate>> {
        debug!("Getting active rate for order with ID: {}", order_id);

        acl::check(
            &*self.acl,
            Resource::OrderExchangeRate,
            Action::Read,
            self,
            Some(&OrderExchangeRateAccess { order_id }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let query = OrderExchangeRates::order_exchange_rates.filter(
            OrderExchangeRates::order_id
                .eq(order_id)
                .and(OrderExchangeRates::status.eq(ExchangeRateStatus::Active)),
        );

        query.get_result::<RawOrderExchangeRate>(self.db_conn).optional().map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn get_all_rates_for_order(&self, order_id: OrderId) -> RepoResultV2<Vec<RawOrderExchangeRate>> {
        debug!("Getting all rates for order with ID: {}", order_id);

        acl::check(
            &*self.acl,
            Resource::OrderExchangeRate,
            Action::Read,
            self,
            Some(&OrderExchangeRateAccess { order_id }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let query = OrderExchangeRates::order_exchange_rates
            .filter(OrderExchangeRates::order_id.eq(order_id))
            .order(OrderExchangeRates::id.desc());

        query.get_results::<RawOrderExchangeRate>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn add_new_active_rate(&self, new_rate: NewOrderExchangeRate) -> RepoResultV2<LatestExchangeRates> {
        debug!("Adding a new active rate using payload: {:?}", new_rate);

        acl::check(
            &*self.acl,
            Resource::OrderExchangeRate,
            Action::Write,
            self,
            Some(&new_rate.clone().into()),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        self.db_conn
            .transaction(|| {
                let get_active_rate_query = OrderExchangeRates::order_exchange_rates
                    .filter(
                        OrderExchangeRates::order_id
                            .eq(new_rate.order_id)
                            .and(OrderExchangeRates::status.eq(ExchangeRateStatus::Active)),
                    )
                    .select(OrderExchangeRates::id);

                let rate_to_expire_id = get_active_rate_query.get_result::<OrderExchangeRateId>(self.db_conn).optional()?;

                let last_expired_rate = match rate_to_expire_id {
                    None => None,
                    Some(rate_to_expire_id) => {
                        let expire_rate_command =
                            diesel::update(OrderExchangeRates::order_exchange_rates.filter(OrderExchangeRates::id.eq(rate_to_expire_id)))
                                .set(&SetExchangeRateStatus {
                                    status: ExchangeRateStatus::Expired,
                                });

                        Some(expire_rate_command.get_result::<RawOrderExchangeRate>(self.db_conn)?)
                    }
                };

                let add_new_rate_command =
                    diesel::insert_into(OrderExchangeRates::order_exchange_rates).values(RawNewOrderExchangeRate::from(new_rate));

                let active_rate = add_new_rate_command.get_result::<RawOrderExchangeRate>(self.db_conn)?;

                Ok(LatestExchangeRates {
                    last_expired_rate,
                    active_rate,
                })
            })
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
    }

    fn expire_current_active_rate(&self, order_id: OrderId) -> RepoResultV2<Option<RawOrderExchangeRate>> {
        debug!("Marking the active rate of order with ID: {} as expired", order_id);

        acl::check(
            &*self.acl,
            Resource::OrderExchangeRate,
            Action::Write,
            self,
            Some(&OrderExchangeRateAccess { order_id }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::update(
            OrderExchangeRates::order_exchange_rates.filter(
                OrderExchangeRates::order_id
                    .eq(order_id)
                    .and(OrderExchangeRates::status.eq(ExchangeRateStatus::Active)),
            ),
        )
        .set(&SetExchangeRateStatus {
            status: ExchangeRateStatus::Expired,
        });

        command.get_result::<RawOrderExchangeRate>(self.db_conn).optional().map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn delete(&self, rate_id: OrderExchangeRateId) -> RepoResultV2<Option<RawOrderExchangeRate>> {
        debug!("Deleting a rate with ID: {}", rate_id);

        let order_id = OrderExchangeRates::order_exchange_rates
            .filter(OrderExchangeRates::id.eq(rate_id))
            .select(OrderExchangeRates::order_id)
            .get_result::<OrderId>(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        let order_id = match order_id {
            None => {
                return Ok(None);
            }
            Some(order_id) => order_id,
        };

        acl::check(
            &*self.acl,
            Resource::OrderExchangeRate,
            Action::Write,
            self,
            Some(&OrderExchangeRateAccess { order_id }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::delete(OrderExchangeRates::order_exchange_rates.filter(OrderExchangeRates::id.eq(rate_id)));

        command.get_result::<RawOrderExchangeRate>(self.db_conn).optional().map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn delete_by_order_id(&self, order_id: OrderId) -> RepoResultV2<Vec<RawOrderExchangeRate>> {
        debug!("Deleting rates with order ID: {}", order_id);

        acl::check(
            &*self.acl,
            Resource::OrderExchangeRate,
            Action::Write,
            self,
            Some(&OrderExchangeRateAccess { order_id }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::delete(OrderExchangeRates::order_exchange_rates.filter(OrderExchangeRates::order_id.eq(order_id)));

        command.get_results::<RawOrderExchangeRate>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, OrderExchangeRateAccess>
    for OrderExchangeRatesRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&OrderExchangeRateAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(OrderExchangeRateAccess { order_id }) = obj {
                    let query = Orders::orders
                        .filter(Orders::id.eq(order_id))
                        .inner_join(InvoicesV2::invoices_v2)
                        .select(InvoicesV2::buyer_user_id);

                    match query.get_result::<UserId>(self.db_conn).optional() {
                        Ok(None) => true,
                        Ok(Some(invoice_user_id)) => invoice_user_id.inner() == &user_id.0,
                        Err(_) => false,
                    }
                } else {
                    false
                }
            }
        }
    }
}
