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
use stq_http::client::HttpClient;
use tokio_timer::Interval;

use client::payments::PaymentsClient;
use models::event_store::EventEntry;
use repos::repo_factory::ReposFactory;
use services::accounts::AccountService;

use self::error::*;

pub type EventHandlerResult<T> = Result<T, Error>;
pub type EventHandlerFuture<T> = Box<Future<Item = T, Error = Error>>;

pub struct EventHandler<T, M, F, HC, PC, AS>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
    HC: HttpClient,
    PC: PaymentsClient,
    AS: AccountService + 'static,
{
    pub cpu_pool: CpuPool,
    pub db_pool: Pool<M>,
    pub repo_factory: F,
    pub http_client: HC,
    pub payments_client: Option<PC>,
    pub account_service: Option<AS>,
    pub saga_url: String,
}

impl<T, M, F, HC, PC, AS> Clone for EventHandler<T, M, F, HC, PC, AS>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
    HC: HttpClient + Clone,
    PC: PaymentsClient + Clone,
    AS: AccountService + Clone + 'static,
{
    fn clone(&self) -> Self {
        Self {
            cpu_pool: self.cpu_pool.clone(),
            db_pool: self.db_pool.clone(),
            repo_factory: self.repo_factory.clone(),
            http_client: self.http_client.clone(),
            payments_client: self.payments_client.clone(),
            account_service: self.account_service.clone(),
            saga_url: self.saga_url.clone(),
        }
    }
}

impl<T, M, F, HC, PC, AS> EventHandler<T, M, F, HC, PC, AS>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
    HC: HttpClient + Clone,
    PC: PaymentsClient + Clone,
    AS: AccountService + Clone + 'static,
{
    pub fn run(self, interval: Duration) -> impl Future<Item = (), Error = FailureError> {
        Interval::new(Instant::now(), interval)
            .map_err(ectx!(ErrorSource::TokioTimer, ErrorKind::Internal))
            .fold(self, |event_handler, _| {
                debug!("Started processing events");
                event_handler.clone().process_events().then(|res| {
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

                    future::ok::<_, FailureError>(event_handler)
                })
            })
            .map(|_| ())
    }

    fn process_events(self) -> EventHandlerFuture<()> {
        let EventHandler {
            cpu_pool,
            db_pool,
            repo_factory,
            ..
        } = self.clone();

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
                self.handle_event(event.clone()).then(move |result| {
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
}

pub fn spawn_on_pool<T, M, Func, R>(db_pool: Pool<M>, cpu_pool: CpuPool, f: Func) -> EventHandlerFuture<R>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    Func: FnOnce(PooledConnection<M>) -> Result<R, Error> + Send + 'static,
    R: Send + 'static,
{
    Box::new(cpu_pool.spawn_fn(move || db_pool.get().map_err(ectx!(ErrorSource::R2d2, ErrorKind::Internal)).and_then(f)))
}
