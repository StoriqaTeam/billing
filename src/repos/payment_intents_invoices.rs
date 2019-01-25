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
use models::invoice_v2::InvoiceId;
use models::UserId;
use models::{NewPaymentIntentInvoice, PaymentIntentInvoice};

use schema::invoices_v2::dsl as InvoicesDsl;
use schema::payment_intents_invoices as PaymentIntentsInvoicesDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type PaymentIntentInvoiceRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, PaymentIntentInvoice>>;
type BoxedExpr = Box<BoxableExpression<crate::schema::payment_intents_invoices::table, Pg, SqlType = Bool>>;

#[derive(Debug, Clone)]
pub enum SearchPaymentIntentInvoice {
    Id(i32),
    InvoiceId(InvoiceId),
    PaymentIntentId(PaymentIntentId),
}

pub struct PaymentIntentInvoiceRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: PaymentIntentInvoiceRepoAcl,
}

pub trait PaymentIntentInvoiceRepo {
    fn get(&self, search: SearchPaymentIntentInvoice) -> RepoResultV2<Option<PaymentIntentInvoice>>;

    fn create(&self, payload: NewPaymentIntentInvoice) -> RepoResultV2<PaymentIntentInvoice>;

    fn delete(&self, search: SearchPaymentIntentInvoice) -> RepoResultV2<()>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> PaymentIntentInvoiceRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: PaymentIntentInvoiceRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> PaymentIntentInvoiceRepo
    for PaymentIntentInvoiceRepoImpl<'a, T>
{
    fn get(&self, search: SearchPaymentIntentInvoice) -> RepoResultV2<Option<PaymentIntentInvoice>> {
        debug!("Getting a payment intent invoice record by search term: {:?}", search);

        let search_exp = into_exp(search);
        let query = PaymentIntentsInvoicesDsl::table.filter(search_exp);

        query
            .get_result(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|payment_intent_invoice: Option<PaymentIntentInvoice>| {
                if let Some(ref payment_intent_invoice) = payment_intent_invoice {
                    acl::check(
                        &*self.acl,
                        Resource::PaymentIntentInvoice,
                        Action::Read,
                        self,
                        Some(&payment_intent_invoice),
                    )
                    .map_err(ectx!(try ErrorKind::Forbidden))?;
                };
                Ok(payment_intent_invoice)
            })
    }

    fn create(&self, payload: NewPaymentIntentInvoice) -> RepoResultV2<PaymentIntentInvoice> {
        debug!("Create a payment intent invoice record: {:?}", payload);
        acl::check(&*self.acl, Resource::PaymentIntentInvoice, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(PaymentIntentsInvoicesDsl::table).values(&payload);

        command.get_result::<PaymentIntentInvoice>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn delete(&self, search: SearchPaymentIntentInvoice) -> RepoResultV2<()> {
        debug!("Deleting a payment intent invoice record by params: {:?}", search);

        let payment_intent_invoice = self.get(search.clone())?;
        acl::check(
            &*self.acl,
            Resource::PaymentIntentInvoice,
            Action::Write,
            self,
            payment_intent_invoice.as_ref(),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let search_exp = into_exp(search);
        let command = diesel::delete(PaymentIntentsInvoicesDsl::table.filter(search_exp));

        command
            .get_result::<PaymentIntentInvoice>(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .map(|_| ())
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, PaymentIntentInvoice>
    for PaymentIntentInvoiceRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&PaymentIntentInvoice>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(ref obj) = obj {
                    let query = PaymentIntentsInvoicesDsl::table
                        .filter(PaymentIntentsInvoicesDsl::invoice_id.eq(obj.invoice_id))
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

fn into_exp(search: SearchPaymentIntentInvoice) -> BoxedExpr {
    match search {
        SearchPaymentIntentInvoice::Id(id) => Box::new(PaymentIntentsInvoicesDsl::id.eq(id)),
        SearchPaymentIntentInvoice::InvoiceId(invoice_id) => Box::new(PaymentIntentsInvoicesDsl::invoice_id.eq(invoice_id)),
        SearchPaymentIntentInvoice::PaymentIntentId(payment_intent_id) => {
            Box::new(PaymentIntentsInvoicesDsl::payment_intent_id.eq(payment_intent_id))
        }
    }
}
