use diesel::{connection::AnsiTransactionManager, pg::Pg, Connection};
use futures::future;
use r2d2::ManageConnection;

use models::invoice_v2::InvoiceId;
use repos::repo_factory::ReposFactory;

use super::{Context, EventHandlerFuture};

// TODO: handle this event properly
pub fn handle_invoice_paid<T, M, F>(_ctx: Context<T, M, F>, _invoice_id: InvoiceId) -> EventHandlerFuture<()>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
{
    Box::new(future::ok(()))
}
