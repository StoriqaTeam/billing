pub mod error;
mod handlers;

use diesel::{
    connection::{AnsiTransactionManager, Connection},
    pg::Pg,
};
use failure::{Error as FailureError, Fail};
use futures::{future, Future, Stream};
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool, PooledConnection};
use sentry::integrations::failure::capture_error;
use std::time::{Duration, Instant};
use tokio_timer::Interval;

use models::event::*;
use models::event_store::EventEntry;
use repos::repo_factory::ReposFactory;

use self::error::*;
use self::handlers::*;

pub type EventHandlerResult<T> = Result<T, Error>;
pub type EventHandlerFuture<T> = Box<Future<Item = T, Error = Error>>;

pub struct Context<T, M, F>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
{
    pub cpu_pool: CpuPool,
    pub db_pool: Pool<M>,
    pub repo_factory: F,
}

impl<T, M, F> Clone for Context<T, M, F>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
{
    fn clone(&self) -> Self {
        Self {
            cpu_pool: self.cpu_pool.clone(),
            db_pool: self.db_pool.clone(),
            repo_factory: self.repo_factory.clone(),
        }
    }
}

pub fn run<T, M, F>(ctx: Context<T, M, F>, interval: Duration) -> impl Future<Item = (), Error = FailureError>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
{
    Interval::new(Instant::now(), interval)
        .map_err(ectx!(ErrorSource::TokioTimer, ErrorKind::Internal))
        .fold(ctx, |ctx, _| {
            debug!("Started processing events");
            process_events(ctx.clone()).then(|res| {
                match res {
                    Ok(_) => {
                        debug!("Finished processing events");
                    }
                    Err(err) => {
                        let err = FailureError::from(err.context("An error occurred while processing events"));
                        error!("{:?}", &err);
                        capture_error(&err);
                    }
                };

                future::ok::<_, FailureError>(ctx)
            })
        })
        .map(|_| ())
}

pub fn process_events<T, M, F>(ctx: Context<T, M, F>) -> EventHandlerFuture<()>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
{
    let Context {
        cpu_pool,
        db_pool,
        repo_factory,
    } = ctx.clone();

    let fut = spawn_on_pool(db_pool.clone(), cpu_pool.clone(), {
        let repo_factory = repo_factory.clone();
        move |conn| {
            let event_store_repo = repo_factory.create_event_store_repo_with_sys_acl(&conn);

            debug!("Resetting stuck events...");
            let reset_events = event_store_repo.reset_stuck_events().map_err(ectx!(try convert))?;
            debug!("{} events have been reset", reset_events.len());

            debug!("Getting events for processing...");
            event_store_repo
                .get_events_for_processing(1)
                .map(|event_entries| {
                    debug!("Got {} events to process", event_entries.len());
                    event_entries
                        .into_iter()
                        .next()
                        .map(|EventEntry { id: entry_id, event, .. }| (entry_id, event))
                })
                .map_err(ectx!(convert))
        }
    })
    .and_then(move |event| match event {
        None => future::Either::A(future::ok(())),
        Some((entry_id, event)) => future::Either::B(future::lazy(move || {
            debug!("Started processing event #{} - {:?}", entry_id, event);
            handle_event(ctx, event.clone()).then(move |result| {
                spawn_on_pool(db_pool, cpu_pool, move |conn| {
                    let event_store_repo = repo_factory.create_event_store_repo_with_sys_acl(&conn);

                    match result {
                        Ok(()) => {
                            debug!("Finished processing event #{} - {:?}", entry_id, event);
                            event_store_repo.complete_event(entry_id).map_err(ectx!(try convert => entry_id))?;
                            Ok(())
                        }
                        Err(e) => {
                            debug!("Failed to process event #{} - {:?}", entry_id, event);
                            event_store_repo.fail_event(entry_id).map_err(ectx!(try convert => entry_id))?;
                            Err(e)
                        }
                    }
                })
            })
        })),
    });

    Box::new(fut)
}

pub fn handle_event<T, M, F>(ctx: Context<T, M, F>, event: Event) -> EventHandlerFuture<()>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
{
    let Event { id: _, payload } = event;

    match payload {
        EventPayload::NoOp => Box::new(future::ok(())),
        EventPayload::InvoicePaid { invoice_id } => handle_invoice_paid(ctx, invoice_id),
    }
}

fn spawn_on_pool<T, M, Func, R>(db_pool: Pool<M>, cpu_pool: CpuPool, f: Func) -> EventHandlerFuture<R>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    Func: FnOnce(PooledConnection<M>) -> Result<R, Error> + Send + 'static,
    R: Send + 'static,
{
    Box::new(cpu_pool.spawn_fn(move || db_pool.get().map_err(ectx!(ErrorSource::R2d2, ErrorKind::Internal)).and_then(f)))
}
