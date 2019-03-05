use failure::Error as FailureError;
use futures::{Future, IntoFuture};
use hyper::header::Headers;
use stq_http::client::ClientHandle;

#[derive(Clone)]
pub struct BillingMicroservice {
    pub url: String,
    pub http_client: ClientHandle,
    pub headers: Headers,
}

#[derive(Clone, Debug, Serialize)]
pub struct NewSubscription {
    pub store_id: i32,
    pub published_base_products_quantity: i32,
}

#[derive(Clone, Debug, Serialize)]
struct CreateSubscriptionsRequest {
    subscriptions: Vec<NewSubscription>,
}

impl BillingMicroservice {
    pub fn create_subscriptions(&self, subscriptions: Vec<NewSubscription>) -> impl Future<Item = (), Error = FailureError> {
        let url = format!("{}/subscriptions", self.url);
        serde_json::to_string(&CreateSubscriptionsRequest { subscriptions })
            .map_err(FailureError::from)
            .into_future()
            .and_then({
                let http_client = self.http_client.clone();
                let headers = self.headers.clone();
                move |body| {
                    http_client
                        .request::<()>(hyper::Method::Post, url, Some(body), Some(headers))
                        .map_err(FailureError::from)
                }
            })
            .map(|_| ())
    }

    pub fn pay_subscriptions(&self) -> impl Future<Item = (), Error = FailureError> {
        let url = format!("{}/subscription/payment", self.url);

        let http_client = self.http_client.clone();

        http_client
            .request::<()>(hyper::Method::Post, url, None, Some(self.headers.clone()))
            .map_err(FailureError::from)
            .map(|_| ())
    }
}
