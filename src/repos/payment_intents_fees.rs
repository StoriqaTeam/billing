use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::sql_types::Bool;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;
use stq_types::stripe::PaymentIntentId;
use stq_types::StoreId;

use repos::legacy_acl::*;

use models::authorization::*;
use models::fee::FeeId;
use models::{NewPaymentIntentFee, PaymentIntentFee, PaymentIntentFeeAccess, UserRole};

use schema::fees::dsl as FeesDsl;
use schema::orders::dsl as OrdersDsl;
use schema::payment_intents_fees as PaymentIntentsFeesDsl;
use schema::roles::dsl as UserRolesDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type PaymentIntentFeeRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, PaymentIntentFeeAccess>>;
type BoxedExpr = Box<BoxableExpression<crate::schema::payment_intents_fees::table, Pg, SqlType = Bool>>;

#[derive(Debug, Clone)]
pub enum SearchPaymentIntentFee {
    Id(i32),
    FeeId(FeeId),
    PaymentIntentId(PaymentIntentId),
}

pub struct PaymentIntentFeeRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: PaymentIntentFeeRepoAcl,
}

pub trait PaymentIntentFeeRepo {
    fn get(&self, search: SearchPaymentIntentFee) -> RepoResultV2<Option<PaymentIntentFee>>;

    fn create(&self, payload: NewPaymentIntentFee) -> RepoResultV2<PaymentIntentFee>;

    fn delete(&self, search: SearchPaymentIntentFee) -> RepoResultV2<()>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> PaymentIntentFeeRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: PaymentIntentFeeRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> PaymentIntentFeeRepo
    for PaymentIntentFeeRepoImpl<'a, T>
{
    fn get(&self, search: SearchPaymentIntentFee) -> RepoResultV2<Option<PaymentIntentFee>> {
        debug!("Getting a payment intent fee record by search term: {:?}", search);

        let search_exp = into_exp(search);
        let query = PaymentIntentsFeesDsl::table.filter(search_exp);

        query
            .get_result(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|payment_intent_fee: Option<PaymentIntentFee>| {
                if let Some(ref payment_intent_fee) = payment_intent_fee {
                    acl::check(
                        &*self.acl,
                        Resource::PaymentIntentFee,
                        Action::Read,
                        self,
                        Some(&PaymentIntentFeeAccess {
                            fee_id: payment_intent_fee.fee_id,
                        }),
                    )
                    .map_err(ectx!(try ErrorKind::Forbidden))?;
                };
                Ok(payment_intent_fee)
            })
    }

    fn create(&self, payload: NewPaymentIntentFee) -> RepoResultV2<PaymentIntentFee> {
        debug!("Create a payment intent fee record: {:?}", payload);
        let access = PaymentIntentFeeAccess { fee_id: payload.fee_id };
        acl::check(&*self.acl, Resource::PaymentIntentFee, Action::Write, self, Some(&access)).map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(PaymentIntentsFeesDsl::table).values(&payload);

        command.get_result::<PaymentIntentFee>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn delete(&self, search: SearchPaymentIntentFee) -> RepoResultV2<()> {
        debug!("Deleting a payment intent fee record by params: {:?}", search);

        let payment_intent_fee = self.get(search.clone())?;
        let access = payment_intent_fee.as_ref().map(|payment_intent_fee| PaymentIntentFeeAccess {
            fee_id: payment_intent_fee.fee_id,
        });
        acl::check(&*self.acl, Resource::PaymentIntentFee, Action::Write, self, access.as_ref())
            .map_err(ectx!(try ErrorKind::Forbidden))?;

        let search_exp = into_exp(search);
        let command = diesel::delete(PaymentIntentsFeesDsl::table.filter(search_exp));

        command
            .get_result::<PaymentIntentFee>(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .map(|_| ())
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, PaymentIntentFeeAccess>
    for PaymentIntentFeeRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&PaymentIntentFeeAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(ref obj) = obj {
                    let store_id = match PaymentIntentsFeesDsl::table
                        .filter(PaymentIntentsFeesDsl::fee_id.eq(obj.fee_id))
                        .inner_join(FeesDsl::fees.inner_join(OrdersDsl::orders))
                        .select(OrdersDsl::store_id)
                        .get_result::<StoreId>(self.db_conn)
                    {
                        Ok(store_id) => store_id,
                        Err(_) => return false,
                    };

                    UserRolesDsl::roles
                        .filter(UserRolesDsl::user_id.eq(user_id))
                        .get_results::<UserRole>(self.db_conn)
                        .map_err(From::from)
                        .map(|user_roles_arg| {
                            user_roles_arg
                                .iter()
                                .any(|user_role_arg| user_role_arg.data.clone().map(|data| data == store_id.0).unwrap_or_default())
                        })
                        .unwrap_or_else(|_: FailureError| false)
                } else {
                    false
                }
            }
        }
    }
}

fn into_exp(search: SearchPaymentIntentFee) -> BoxedExpr {
    use self::SearchPaymentIntentFee::*;
    match search {
        Id(id) => Box::new(PaymentIntentsFeesDsl::id.eq(id)),
        FeeId(fee_id) => Box::new(PaymentIntentsFeesDsl::fee_id.eq(fee_id)),
        PaymentIntentId(payment_intent_id) => Box::new(PaymentIntentsFeesDsl::payment_intent_id.eq(payment_intent_id)),
    }
}
