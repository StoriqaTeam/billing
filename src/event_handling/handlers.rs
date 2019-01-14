use diesel::{connection::AnsiTransactionManager, pg::Pg, Connection};
use futures::future;
use r2d2::ManageConnection;

use models::{invoice_v2::InvoiceId, Event, EventPayload};
use repos::repo_factory::ReposFactory;

use super::{EventHandler, EventHandlerFuture};

impl<T, M, F> EventHandler<T, M, F>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
{
    pub fn handle_event(self, event: Event) -> EventHandlerFuture<()> {
        let Event { id: _, payload } = event;

        match payload {
            EventPayload::NoOp => Box::new(future::ok(())),
            EventPayload::InvoicePaid { invoice_id } => self.handle_invoice_paid(invoice_id),
        }
    }

    // TODO: handle this event properly
    pub fn handle_invoice_paid(self, _invoice_id: InvoiceId) -> EventHandlerFuture<()> {
        Box::new(future::ok(()))
    }
}
