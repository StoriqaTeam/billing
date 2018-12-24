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
use models::invoice_v2::{InvoiceAccess, InvoiceId, NewInvoice, RawInvoice};
use models::UserId;
use schema::invoices_v2::dsl as InvoicesV2;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type InvoicesV2RepoAcl = Box<Acl<Resource, Action, Scope, FailureError, InvoiceAccess>>;

pub struct InvoicesV2RepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: InvoicesV2RepoAcl,
}

pub trait InvoicesV2Repo {
    fn get(&self, invoice_id: InvoiceId) -> RepoResultV2<Option<RawInvoice>>;
    fn create(&self, payload: NewInvoice) -> RepoResultV2<RawInvoice>;
    fn delete(&self, invoice_id: InvoiceId) -> RepoResultV2<Option<RawInvoice>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> InvoicesV2RepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: InvoicesV2RepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> InvoicesV2Repo for InvoicesV2RepoImpl<'a, T> {
    fn get(&self, invoice_id: InvoiceId) -> RepoResultV2<Option<RawInvoice>> {
        debug!("Getting an invoice with ID: {}", invoice_id);

        let query = InvoicesV2::invoices_v2.filter(InvoicesV2::id.eq(invoice_id));

        query
            .get_result(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|invoice: Option<RawInvoice>| {
                if let Some(ref invoice) = invoice {
                    acl::check(
                        &*self.acl,
                        Resource::Invoice,
                        Action::Read,
                        self,
                        Some(&InvoiceAccess::from(invoice.clone())),
                    )
                    .map_err(ectx!(try ErrorKind::Forbidden))?;
                };
                Ok(invoice)
            })
    }

    fn create(&self, payload: NewInvoice) -> RepoResultV2<RawInvoice> {
        debug!("Creating an invoice using payload: {:?}", payload);

        acl::check(&*self.acl, Resource::Invoice, Action::Write, self, Some(&payload.clone().into()))
            .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(InvoicesV2::invoices_v2).values(&payload);

        command.get_result::<RawInvoice>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn delete(&self, invoice_id: InvoiceId) -> RepoResultV2<Option<RawInvoice>> {
        debug!("Deleting an invoice with ID: {}", invoice_id);

        let buyer_user_id = InvoicesV2::invoices_v2
            .filter(InvoicesV2::id.eq(invoice_id))
            .select(InvoicesV2::buyer_user_id)
            .get_result::<UserId>(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })?;

        let buyer_user_id = match buyer_user_id {
            None => {
                return Ok(None);
            }
            Some(buyer_user_id) => buyer_user_id,
        };

        acl::check(
            &*self.acl,
            Resource::Invoice,
            Action::Write,
            self,
            Some(&InvoiceAccess { user_id: buyer_user_id }),
        )
        .map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::delete(InvoicesV2::invoices_v2.filter(InvoicesV2::id.eq(invoice_id)));

        command.get_result::<RawInvoice>(self.db_conn).optional().map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, InvoiceAccess>
    for InvoicesV2RepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&InvoiceAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(InvoiceAccess { user_id: invoice_user_id }) = obj {
                    invoice_user_id.inner() == &user_id.0
                } else {
                    false
                }
            }
        }
    }
}
