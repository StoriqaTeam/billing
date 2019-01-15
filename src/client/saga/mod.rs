mod error;
mod types;

use failure::Fail;
use futures::{prelude::*, Future};
use hyper::{Headers, Method};
use stq_http::client::HttpClient;

pub use self::error::*;
pub use self::types::OrderStateUpdate;

pub trait SagaClient: Send + Sync + 'static {
    fn update_order_states(&self, order_states: Vec<OrderStateUpdate>) -> Box<Future<Item = (), Error = Error> + Send>;
}

#[derive(Clone)]
pub struct SagaClientImpl<C: HttpClient + Clone> {
    client: C,
    url: String,
}

impl<C: HttpClient + Clone + Send> SagaClientImpl<C> {
    pub fn new(client: C, url: String) -> Self {
        Self { client, url }
    }
}

impl<C: HttpClient + Clone> SagaClient for SagaClientImpl<C> {
    fn update_order_states(&self, order_state_updates: Vec<OrderStateUpdate>) -> Box<Future<Item = (), Error = Error> + Send> {
        let SagaClientImpl { client, url } = self.clone();

        let fut = serde_json::to_string(&order_state_updates)
            .map_err(ectx!(ErrorSource::SerdeJson, ErrorKind::Internal => order_state_updates))
            .into_future()
            .and_then(move |body| {
                let url = format!("{}/orders/update_state", url);
                client
                    .request_json::<()>(Method::Post, url.clone(), Some(body.clone()), None)
                    .map_err(ectx!(ErrorSource::StqHttp, ErrorKind::Internal => Method::Post, url, Some(body), None as Option<Headers>))
            });

        Box::new(fut)
    }
}
