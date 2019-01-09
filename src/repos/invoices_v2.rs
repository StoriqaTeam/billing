use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;
use models::amount::Amount;

use repos::legacy_acl::*;

use models::authorization::*;
use models::invoice_v2::*;
use models::{AccountId, TransactionId, UserId};
use schema::amounts_received::dsl as AmountsReceived;
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
    fn get_by_account_id(&self, account_id: AccountId) -> RepoResultV2<Option<RawInvoice>>;
    fn create(&self, input: NewInvoice) -> RepoResultV2<RawInvoice>;
    fn increase_amount_captured(
        &self,
        account_id: AccountId,
        transaction_id: TransactionId,
        amount_received: Amount,
    ) -> RepoResultV2<RawInvoice>;
    fn set_amount_paid(&self, invoice_id: InvoiceId, input: InvoiceSetAmountPaid) -> RepoResultV2<RawInvoice>;
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

    fn get_by_account_id(&self, account_id: AccountId) -> RepoResultV2<Option<RawInvoice>> {
        debug!("Getting an invoice by account ID: {}", account_id);

        let query = InvoicesV2::invoices_v2.filter(InvoicesV2::account_id.eq(account_id));

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

    fn create(&self, input: NewInvoice) -> RepoResultV2<RawInvoice> {
        debug!("Creating an invoice using input: {:?}", input);

        acl::check(&*self.acl, Resource::Invoice, Action::Write, self, Some(&input.clone().into()))
            .map_err(ectx!(try ErrorKind::Forbidden))?;

        let payload = RawNewInvoice::from(input);

        diesel::insert_into(InvoicesV2::invoices_v2)
            .values(&payload)
            .get_result::<RawInvoice>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
    }

    fn increase_amount_captured(
        &self,
        account_id: AccountId,
        transaction_id: TransactionId,
        amount_received: Amount,
    ) -> RepoResultV2<RawInvoice> {
        debug!(
            "Increasing amount captured for invoice with account ID = {} by amount = {}, tx id = {}",
            &account_id, &amount_received, &transaction_id
        );

        let query = InvoicesV2::invoices_v2.filter(InvoicesV2::account_id.eq(account_id));

        let invoice = query
            .get_result::<RawInvoice>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|invoice| {
                acl::check(
                    &*self.acl,
                    Resource::Invoice,
                    Action::Write,
                    self,
                    Some(&InvoiceAccess::from(invoice.clone())),
                )
                .map_err(ectx!(try ErrorKind::Forbidden))
                .map(|_| invoice)
            })?;

        let invoice_id = invoice.id;
        let new_amount_received = NewAmountReceived {
            id: transaction_id,
            invoice_id,
            amount_received,
        };

        let new_amount_captured = invoice.amount_captured.checked_add(amount_received).ok_or({
            let e = format_err!(
                "Overflow occurred when adding amounts. Previous amount captured: {}, amount received: {}",
                invoice.amount_captured,
                amount_received,
            );
            ectx!(try err e, ErrorKind::Internal)
        })?;

        self.db_conn
            .transaction(move || {
                diesel::insert_into(AmountsReceived::amounts_received)
                    .values(new_amount_received)
                    .get_result::<RawAmountReceived>(self.db_conn)?;

                diesel::update(InvoicesV2::invoices_v2.filter(InvoicesV2::id.eq(invoice_id)))
                    .set(InvoicesV2::amount_captured.eq(&new_amount_captured))
                    .get_result::<RawInvoice>(self.db_conn)
            })
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
    }

    fn set_amount_paid(&self, invoice_id: InvoiceId, input: InvoiceSetAmountPaid) -> RepoResultV2<RawInvoice> {
        debug!(
            "Setting amount paid for invoice with ID = {} using payload: {:?}",
            &invoice_id, &input
        );

        let query = InvoicesV2::invoices_v2.filter(InvoicesV2::id.eq(invoice_id));

        query
            .get_result::<RawInvoice>(self.db_conn)
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|invoice| {
                acl::check(
                    &*self.acl,
                    Resource::Invoice,
                    Action::Write,
                    self,
                    Some(&InvoiceAccess::from(invoice.clone())),
                )
                .map_err(ectx!(try ErrorKind::Forbidden))
            })?;

        let changeset = RawInvoiceSetAmountPaid::from(input);

        let command = diesel::update(InvoicesV2::invoices_v2.filter(InvoicesV2::id.eq(invoice_id))).set(&changeset);

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
