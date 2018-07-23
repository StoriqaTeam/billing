//! Users is a microservice responsible for authentication and managing user profiles.
//! The layered structure of the app is
//!
//! `Application -> Controller -> Service -> Repo + HttpClient`
//!
//! Each layer can only face exceptions in its base layers and can only expose its own errors.
//! E.g. `Service` layer will only deal with `Repo` and `HttpClient` errors and will only return
//! `ServiceError`. That way Controller will only have to deal with ServiceError, but not with `Repo`
//! or `HttpClient` repo.

extern crate config as config_crate;
#[macro_use]
extern crate diesel;
extern crate env_logger;
#[macro_use]
extern crate failure;
extern crate chrono;
extern crate futures;
extern crate futures_cpupool;
extern crate hyper;
extern crate hyper_tls;
#[macro_use]
extern crate log;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate stq_http;
extern crate stq_logging;
extern crate stq_router;
extern crate stq_static_resources;
extern crate stq_types;
extern crate tokio_core;
extern crate uuid;
extern crate validator;

#[macro_use]
pub mod macros;
pub mod config;
pub mod controller;
pub mod errors;
pub mod models;
pub mod repos;
pub mod services;

use std::process;
use std::sync::Arc;

use diesel::pg::PgConnection;
use futures::future;
use futures::{Future, Stream};
use futures_cpupool::CpuPool;
use hyper::server::Http;
use r2d2_diesel::ConnectionManager;
use tokio_core::reactor::Core;

use stq_http::client::Config as HttpConfig;
use stq_http::controller::Application;

use config::Config;
use errors::Error;
use repos::acl::RolesCacheImpl;
use repos::repo_factory::ReposFactoryImpl;

/// Starts new web service from provided `Config`
pub fn start_server<F: FnOnce() + 'static>(config: Config, port: &Option<String>, callback: F) {
    // Prepare reactor
    let mut core = Core::new().expect("Unexpected error creating event loop core");
    let handle = Arc::new(core.handle());

    let http_config = HttpConfig {
        http_client_retries: config.client.http_client_retries,
        http_client_buffer_size: config.client.http_client_buffer_size,
    };
    let client = stq_http::client::Client::new(&http_config, &handle);
    let client_handle = client.handle();
    let client_stream = client.stream();
    handle.spawn(client_stream.for_each(|_| Ok(())));

    // Prepare server
    let thread_count = config.server.thread_count;

    // Prepare server
    let address = {
        let port = port.as_ref().unwrap_or(&config.server.port);
        format!("{}:{}", config.server.host, port).parse().expect("Could not parse address")
    };

    // Prepare database pool
    let database_url: String = config.server.database.parse().expect("Database URL must be set in configuration");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let db_pool = r2d2::Pool::builder().build(manager).expect("Failed to create connection pool");

    // Prepare CPU pool
    let cpu_pool = CpuPool::new(thread_count);

    let roles_cache = RolesCacheImpl::default();

    let repo_factory = ReposFactoryImpl::new(roles_cache.clone());

    let serve = Http::new()
        .serve_addr_handle(&address, &handle, move || {
            let controller = controller::ControllerImpl::new(
                db_pool.clone(),
                cpu_pool.clone(),
                client_handle.clone(),
                config.clone(),
                roles_cache.clone(),
                repo_factory.clone(),
            );

            // Prepare application
            let app = Application::<Error>::new(controller);

            Ok(app)
        })
        .unwrap_or_else(|why| {
            error!("Http Server Initialization Error: {}", why);
            process::exit(1);
        });

    let handle_arc2 = handle.clone();
    handle.spawn(
        serve
            .for_each(move |conn| {
                handle_arc2.spawn(conn.map(|_| ()).map_err(|why| error!("Server Error: {:?}", why)));
                Ok(())
            })
            .map_err(|_| ()),
    );

    info!("Listening on http://{}, threads: {}", address, thread_count);
    handle.spawn_fn(move || {
        callback();
        future::ok(())
    });
    core.run(future::empty::<(), ()>()).unwrap();
}
