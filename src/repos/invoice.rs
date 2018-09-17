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
use models::{Invoice, OrderInfo, UpdateInvoice};
use schema::invoices::dsl::*;
use schema::orders_info::dsl as OrderInfos;

/// Invoices repository, responsible for handling invoice
pub struct InvoiceRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: Box<Acl<Resource, Action, Scope, FailureError, Invoice>>,
}

pub trait InvoiceRepo {
    /// Find specific invoice by ID
    fn find(&self, invoice_id: InvoiceId) -> RepoResult<Option<Invoice>>;

    /// Find specific invoice by saga ID
    fn find_by_saga_id(&self, saga_id: SagaId) -> RepoResult<Option<Invoice>>;

    /// Creates new invoice
    fn create(&self, payload: Invoice) -> RepoResult<Invoice>;

    /// Updates invoice
    fn update(&self, invoice_id: InvoiceId, payload: UpdateInvoice) -> RepoResult<Invoice>;

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
    fn find(&self, invoice_id_arg: InvoiceId) -> RepoResult<Option<Invoice>> {
        invoices
            .filter(invoice_id.eq(invoice_id_arg))
            .get_result(self.db_conn)
            .optional()
            .map_err(From::from)
            .and_then(|invoice_arg: Option<Invoice>| {
                if let Some(ref invoice_arg) = invoice_arg {
                    acl::check(&*self.acl, Resource::Invoice, Action::Read, self, Some(invoice_arg))?;
                };
                Ok(invoice_arg)
            }).map_err(|e: FailureError| e.context(format!("Find specific invoice {} error occured", invoice_id_arg)).into())
    }

    /// Find specific invoice by saga ID
    fn find_by_saga_id(&self, saga_id_arg: SagaId) -> RepoResult<Option<Invoice>> {
        invoices
            .filter(id.eq(saga_id_arg))
            .get_result(self.db_conn)
            .optional()
            .map_err(From::from)
            .and_then(|invoice_arg: Option<Invoice>| {
                if let Some(ref invoice_arg) = invoice_arg {
                    acl::check(&*self.acl, Resource::Invoice, Action::Read, self, Some(invoice_arg))?;
                };
                Ok(invoice_arg)
            }).map_err(|e: FailureError| {
                e.context(format!("Find specific invoice by saga id {} error occured", saga_id_arg))
                    .into()
            })
    }

    /// Creates new invoice
    fn create(&self, payload: Invoice) -> RepoResult<Invoice> {
        let query_invoice = diesel::insert_into(invoices).values(&payload);
        query_invoice
            .get_result::<Invoice>(self.db_conn)
            .map_err(From::from)
            .and_then(|invoice| {
                acl::check(&*self.acl, Resource::Invoice, Action::Write, self, Some(&invoice))?;
                Ok(invoice)
            }).map_err(|e: FailureError| e.context(format!("Create a new invoice {:?} error occured", payload)).into())
    }

    /// update new invoice
    fn update(&self, invoice_id_arg: InvoiceId, payload: UpdateInvoice) -> RepoResult<Invoice> {
        let filter = invoices.filter(invoice_id.eq(invoice_id_arg));

        let query_invoice = diesel::update(filter).set(&payload);
        query_invoice.get_result::<Invoice>(self.db_conn).map_err(|e| {
            e.context(format!(
                "Update invoice id {} with payload {:?} error occured",
                invoice_id_arg, payload
            )).into()
        })
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
                acl::check(&*self.acl, Resource::Invoice, Action::Write, self, Some(&invoice))?;
                Ok(invoice)
            }).map_err(|e: FailureError| e.context(format!("Delete invoice id {} error occured", id_arg)).into())
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, Invoice>
    for InvoiceRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: UserId, scope: &Scope, obj: Option<&Invoice>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(invoice) = obj {
                    OrderInfos::orders_info
                        .filter(OrderInfos::saga_id.eq(invoice.id))
                        .get_results::<OrderInfo>(self.db_conn)
                        .map_err(From::from)
                        .map(|order_infos| order_infos.iter().all(|order_info| order_info.customer_id == user_id))
                        .unwrap_or_else(|_: FailureError| false)
                } else {
                    false
                }
            }
        }
    }
}
