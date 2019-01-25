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

use repos::legacy_acl::*;

use models::authorization::*;
use models::UserId;
use models::{NewPaymentIntent, PaymentIntent, PaymentIntentAccess, UpdatePaymentIntent};

use schema::invoices_v2::dsl as InvoicesDsl;
use schema::payment_intent::dsl as PaymentIntentDsl;
use schema::payment_intents_invoices as PaymentIntentsInvoicesDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type PaymentIntentRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, PaymentIntentAccess>>;

#[derive(Debug, Clone)]
pub enum SearchPaymentIntent {
    Id(PaymentIntentId),
}

pub struct PaymentIntentRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: PaymentIntentRepoAcl,
}

pub trait PaymentIntentRepo {
    fn get(&self, search: SearchPaymentIntent) -> RepoResultV2<Option<PaymentIntent>>;
    fn create(&self, new_payment_intent: NewPaymentIntent) -> RepoResultV2<PaymentIntent>;
    fn update(&self, payment_intent_id: PaymentIntentId, update_payment_intent: UpdatePaymentIntent) -> RepoResultV2<PaymentIntent>;
    fn delete(&self, payment_intent_id: PaymentIntentId) -> RepoResultV2<Option<PaymentIntent>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> PaymentIntentRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: PaymentIntentRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> PaymentIntentRepo
    for PaymentIntentRepoImpl<'a, T>
{
    fn get(&self, search: SearchPaymentIntent) -> RepoResultV2<Option<PaymentIntent>> {
        debug!("Getting a payment intent by search term: {:?}", search);

        let search_exp: Box<BoxableExpression<PaymentIntentDsl::payment_intent, _, SqlType = Bool>> = match search {
            SearchPaymentIntent::Id(payment_intent_id) => Box::new(PaymentIntentDsl::id.eq(payment_intent_id)),
        };

        let query = PaymentIntentDsl::payment_intent.filter(search_exp);

        query
            .get_result(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|payment_intent: Option<PaymentIntent>| {
                if let Some(ref payment_intent) = payment_intent {
                    acl::check(
                        &*self.acl,
                        Resource::PaymentIntent,
                        Action::Read,
                        self,
                        Some(&payment_intent.into()),
                    )
                    .map_err(ectx!(try ErrorKind::Forbidden))?;
                };
                Ok(payment_intent)
            })
    }

    fn create(&self, new_payment_intent: NewPaymentIntent) -> RepoResultV2<PaymentIntent> {
        debug!("Create a payment intent with ID: {}", new_payment_intent.id);
        acl::check(&*self.acl, Resource::PaymentIntent, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(PaymentIntentDsl::payment_intent).values(&new_payment_intent);

        command.get_result::<PaymentIntent>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn update(&self, payment_intent_id: PaymentIntentId, update_payment_intent: UpdatePaymentIntent) -> RepoResultV2<PaymentIntent> {
        debug!("Updating a payment intent with ID: {}", payment_intent_id);
        acl::check(&*self.acl, Resource::PaymentIntent, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let filter = PaymentIntentDsl::payment_intent.filter(PaymentIntentDsl::id.eq(&payment_intent_id));

        let query_payment_intent = diesel::update(filter).set(&update_payment_intent);
        query_payment_intent.get_result::<PaymentIntent>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn delete(&self, payment_intent_id: PaymentIntentId) -> RepoResultV2<Option<PaymentIntent>> {
        debug!("Deleting a payment intent with ID: {}", payment_intent_id);
        acl::check(&*self.acl, Resource::PaymentIntent, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::delete(PaymentIntentDsl::payment_intent.filter(PaymentIntentDsl::id.eq(payment_intent_id)));

        command.get_result::<PaymentIntent>(self.db_conn).optional().map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, PaymentIntentAccess>
    for PaymentIntentRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&PaymentIntentAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(PaymentIntentAccess { id }) = obj {
                    let query = PaymentIntentsInvoicesDsl::table
                        .filter(PaymentIntentsInvoicesDsl::payment_intent_id.eq(id))
                        .inner_join(InvoicesDsl::invoices_v2)
                        .select(InvoicesDsl::buyer_user_id);

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
