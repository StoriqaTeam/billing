//! Invoices repo, presents CRUD operations with db for invoice
use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;

use stq_types::{InvoiceId, SagaId, UserId};

use repos::legacy_acl::*;

use super::acl;
use super::types::RepoResult;
use models::authorization::*;
use models::invoice::invoices::dsl::*;
use models::{Invoice, NewInvoice};

/// Invoices repository, responsible for handling invoice
pub struct InvoiceRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: Box<Acl<Resource, Action, Scope, FailureError, Invoice>>,
}

pub trait InvoiceRepo {
    /// Find specific invoice by ID
    fn find(&self, invoice_id: InvoiceId) -> RepoResult<Option<Invoice>>;

    /// Creates new invoice
    fn create(&self, payload: NewInvoice) -> RepoResult<Invoice>;

    /// Deletes invoice
    fn delete(&self, id: SagaId) -> RepoResult<Invoice>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> InvoiceRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: Box<Acl<Resource, Action, Scope, FailureError, Invoice>>) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> InvoiceRepo for InvoiceRepoImpl<'a, T> {
    /// Find specific invoice by ID
    fn find(&self, id_arg: InvoiceId) -> RepoResult<Option<Invoice>> {
        invoices
            .filter(id.eq(id_arg.clone()))
            .get_result(self.db_conn)
            .optional()
            .map_err(From::from)
            .and_then(|invoice_arg: Option<Invoice>| {
                if let Some(ref invoice_arg) = invoice_arg {
                    acl::check(&*self.acl, Resource::Invoice, Action::Read, self, Some(invoice_arg))?;
                };
                Ok(invoice_arg)
            })
            .map_err(|e: FailureError| e.context(format!("Find specific invoice {:?} error occured", id_arg)).into())
    }

    /// Creates new invoice
    fn create(&self, payload: NewInvoice) -> RepoResult<Invoice> {
        let query_invoice = diesel::insert_into(invoices).values(&payload);
        query_invoice
            .get_result::<Invoice>(self.db_conn)
            .map_err(|e| e.context(format!("Create a new invoice {:?} error occured", payload)).into())
    }

    /// Deletes invoice
    fn delete(&self, id_arg: SagaId) -> RepoResult<Invoice> {
        debug!("Delete invoice {:?}.", id_arg);
        let filtered = invoices.filter(id.eq(id_arg));

        let query = diesel::delete(filtered);
        query
            .get_result(self.db_conn)
            .map_err(From::from)
            .and_then(|invoice| {
                acl::check(&*self.acl, Resource::Merchant, Action::Write, self, Some(&invoice))?;
                Ok(invoice)
            })
            .map_err(|e: FailureError| e.context(format!("Delete invoice {:?} error occured", id_arg)).into())
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, Invoice>
    for InvoiceRepoImpl<'a, T>
{
    fn is_in_scope(&self, _invoice_id_arg: UserId, scope: &Scope, _obj: Option<&Invoice>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => false,
        }
    }
}
