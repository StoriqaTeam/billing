mod error;
mod types;

pub use self::error::*;
pub use self::types::*;

use failure::Fail;
use futures::Future;
use hyper::{Headers, Method};
use stq_http::client::HttpClient;

pub trait StoresClient: Send + Sync + 'static {
    fn get_currency_exchange(&self) -> Box<Future<Item = CurrencyExchangeInfoRequest, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct StoresClientImpl<C: HttpClient + Clone> {
    client: C,
    url: String,
}

impl<C: HttpClient + Clone + Send> StoresClientImpl<C> {
    pub fn new(client: C, url: String) -> Self {
        Self { client, url }
    }
}

impl<C: HttpClient + Clone> StoresClient for StoresClientImpl<C> {
    fn get_currency_exchange(&self) -> Box<Future<Item = CurrencyExchangeInfoRequest, Error = Error> + Send> {
        let StoresClientImpl { client, url } = self.clone();
        let url = format!("{}/currency_exchange", url);

        let fut = client
            .request_json::<CurrencyExchangeInfoRequest>(Method::Get, url.clone(), None, None)
            .map_err(ectx!(ErrorSource::StqHttp, ErrorKind::Internal => Method::Get, url, None as Option<Headers>));

        Box::new(fut)
    }
}
