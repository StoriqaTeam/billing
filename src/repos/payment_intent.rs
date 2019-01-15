use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;
use stq_types::stripe::PaymentIntentId;

use repos::legacy_acl::*;

use models::authorization::*;
use models::{NewPaymentIntent, PaymentIntent, UpdatePaymentIntent};

use schema::payment_intent::dsl as PaymentIntentDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type PaymentIntentRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, PaymentIntent>>;

pub struct PaymentIntentRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: PaymentIntentRepoAcl,
}

pub trait PaymentIntentRepo {
    fn get(&self, payment_intent_id: PaymentIntentId) -> RepoResultV2<Option<PaymentIntent>>;
    fn create(&self, new_payment_intent: NewPaymentIntent) -> RepoResultV2<PaymentIntent>;
    fn update(&self, payment_intent_id: PaymentIntentId, update_payment_intent: UpdatePaymentIntent) -> RepoResultV2<PaymentIntent>;
    fn delete(&self, payment_intent_id: PaymentIntentId) -> RepoResultV2<Option<PaymentIntent>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> PaymentIntentRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: PaymentIntentRepoAcl) -> Self {
        Self { db_conn, acl }
    }

    fn check_write(&self, payment_intent_id: PaymentIntentId) -> RepoResultV2<()> {
        let payment_intent = PaymentIntentDsl::payment_intent
            .filter(PaymentIntentDsl::id.eq(payment_intent_id))
            .get_result(self.db_conn)
            .optional()?;

        let payment_intent = match payment_intent {
            None => {
                return Ok(());
            }
            Some(payment_intent) => payment_intent,
        };

        acl::check(&*self.acl, Resource::PaymentIntent, Action::Write, self, Some(&payment_intent))
            .map_err(ectx!(try ErrorKind::Forbidden))?;

        Ok(())
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> PaymentIntentRepo
    for PaymentIntentRepoImpl<'a, T>
{
    fn get(&self, payment_intent_id: PaymentIntentId) -> RepoResultV2<Option<PaymentIntent>> {
        debug!("Getting a payment intent with ID: {}", payment_intent_id);

        let query = PaymentIntentDsl::payment_intent.filter(PaymentIntentDsl::id.eq(payment_intent_id));

        query
            .get_result(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|payment_intent: Option<PaymentIntent>| {
                if payment_intent.is_some() {
                    acl::check(&*self.acl, Resource::PaymentIntent, Action::Read, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;
                };
                Ok(payment_intent)
            })
    }

    fn create(&self, new_payment_intent: NewPaymentIntent) -> RepoResultV2<PaymentIntent> {
        debug!("Getting a payment intent with ID: {}", new_payment_intent.id);
        acl::check(
            &*self.acl,
            Resource::PaymentIntent,
            Action::Write,
            self,
            Some(&new_payment_intent.clone().into()),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(PaymentIntentDsl::payment_intent).values(&new_payment_intent);

        command.get_result::<PaymentIntent>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn update(&self, payment_intent_id: PaymentIntentId, update_payment_intent: UpdatePaymentIntent) -> RepoResultV2<PaymentIntent> {
        debug!("Updating a payment intent with ID: {}", payment_intent_id);
        self.check_write(payment_intent_id.clone())?;
        let filter = PaymentIntentDsl::payment_intent.filter(PaymentIntentDsl::id.eq(&payment_intent_id));

        let query_payment_intent = diesel::update(filter).set(&update_payment_intent);
        query_payment_intent.get_result::<PaymentIntent>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn delete(&self, payment_intent_id: PaymentIntentId) -> RepoResultV2<Option<PaymentIntent>> {
        debug!("Deleting a payment intent with ID: {}", payment_intent_id);
        self.check_write(payment_intent_id.clone())?;

        let command = diesel::delete(PaymentIntentDsl::payment_intent.filter(PaymentIntentDsl::id.eq(payment_intent_id)));

        command.get_result::<PaymentIntent>(self.db_conn).optional().map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, PaymentIntent>
    for PaymentIntentRepoImpl<'a, T>
{
    fn is_in_scope(&self, _user_id: stq_types::UserId, _scope: &Scope, _obj: Option<&PaymentIntent>) -> bool {
        true
    }
}
