extern crate config as config_crate;
extern crate failure;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio;
extern crate tokio_core;
extern crate tokio_signal;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate stq_http;

mod billing;
mod config;
mod stores;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use failure::{Error as FailureError, Fail};
use futures::stream::Stream;
use futures::Future;
use tokio::timer::Interval;
use tokio_core::reactor::Core;

use self::config::Config;

fn main() {
    env_logger::init();
    let mut core = Core::new().expect("Unexpected error creating event loop core");
    let handle = Arc::new(core.handle());

    let config = Config::new().expect("Could not create config.");

    let http_client_config = stq_http::client::Config {
        http_client_retries: 1,
        http_client_buffer_size: 1024 * 1024,
        timeout_duration_ms: 5_000,
    };

    let http_client = stq_http::client::Client::new(&http_client_config, &*handle);
    let http_client_handle = http_client.handle();
    let client_stream = http_client.stream();
    handle.spawn(client_stream.for_each(|_| Ok(())));

    let mut headers = hyper::header::Headers::new();
    headers.set_raw("Currency", "STQ");
    headers.set_raw("FiatCurrency", "EUR");
    headers.set_raw("Authorization", "1");

    let stores_microservice = stores::StoresMicroservice {
        url: config.stores_microservice.url,
        http_client: http_client_handle.clone(),
        headers: headers.clone(),
    };

    let billing_microservice = billing::BillingMicroservice {
        url: config.billing_microservice.url,
        http_client: http_client_handle.clone(),
        headers: headers.clone(),
    };

    let interval =
        Interval::new(Instant::now(), Duration::from_secs(config.interval_s as u64)).map_err(|e| e.context("timer creation error").into());

    let stream = interval.and_then(move |_| {
        new_subscriptions(stores_microservice.clone(), billing_microservice.clone())
            .then(|res| match res {
                Ok(_) => Ok(()),
                Err(err) => {
                    warn!("Failed to create new subscriptions: {}", err);
                    Ok(())
                }
            })
            .and_then({
                let billing_microservice = billing_microservice.clone();
                move |_| {
                    info!("Paying subscriptions");
                    billing_microservice.pay_subscriptions()
                }
            })
    });

    let fut = stream
        .or_else(|e: FailureError| {
            error!("Error occurred: {}.", e);
            futures::future::ok(())
        })
        .for_each(|_| futures::future::ok(()));

    handle.spawn(fut);

    core.run(tokio_signal::ctrl_c().flatten_stream().take(1u64).for_each(|()| {
        info!("Ctrl+C received. Exit");

        Ok(())
    }))
    .unwrap();
}

fn new_subscriptions(
    stores_microservice: stores::StoresMicroservice,
    billing_microservice: billing::BillingMicroservice,
) -> impl Future<Item = (), Error = FailureError> {
    info!("Adding new subscriptions");
    stores_microservice
        .find_published_products()
        .map(|base_products| {
            let mut by_store_quantity: HashMap<i32, i32> = HashMap::new();
            for base_product in base_products {
                *by_store_quantity.entry(base_product.store_id).or_insert(0) += 1;
            }
            by_store_quantity
                .into_iter()
                .map(|(store_id, published_base_products_quantity)| billing::NewSubscription {
                    store_id,
                    published_base_products_quantity,
                })
                .collect::<Vec<_>>()
        })
        .and_then(move |subscriptions| billing_microservice.create_subscriptions(subscriptions))
}
