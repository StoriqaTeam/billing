mod error;
mod types;

pub use self::error::*;
pub use self::types::*;

use failure::Fail;
use futures::Future;
use hyper::{Headers, Method};
use stq_http::client::HttpClient;
use stq_http::request_util::{Currency as CurrencyHeader, FiatCurrency as FiatCurrencyHeader};

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

fn stores_headers() -> Headers {
    let mut headers = Headers::new();
    headers.set(CurrencyHeader("STQ".to_string()));
    headers.set(FiatCurrencyHeader("USD".to_string()));
    headers
}

impl<C: HttpClient + Clone> StoresClient for StoresClientImpl<C> {
    fn get_currency_exchange(&self) -> Box<Future<Item = CurrencyExchangeInfoRequest, Error = Error> + Send> {
        let StoresClientImpl { client, url } = self.clone();
        let url = format!("{}/currency_exchange", url);

        let fut = client
            .request_json::<CurrencyExchangeInfoRequest>(Method::Get, url.clone(), None, Some(stores_headers()))
            .map_err(ectx!(ErrorSource::StqHttp, ErrorKind::Internal => Method::Get, url, None as Option<Headers>));

        Box::new(fut)
    }
}
