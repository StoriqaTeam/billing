use failure::Error as FailureError;
use futures::{Future, IntoFuture};
use hyper::header::Headers;
use stq_http::client::ClientHandle;

#[derive(Clone)]
pub struct StoresMicroservice {
    pub url: String,
    pub http_client: ClientHandle,
    pub headers: Headers,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseProduct {
    pub store_id: i32,
    pub status: ModerationStatus,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum ModerationStatus {
    Draft,
    Moderation,
    Decline,
    Blocked,
    Published,
}

#[derive(Debug, Clone, Serialize)]
struct SearchProductsByName {
    name: String,
    options: Option<ProductsSearchOptions>,
}

#[derive(Debug, Clone, Serialize)]
struct ProductsSearchOptions {
    status: Option<ModerationStatus>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    base_products: Vec<BaseProduct>,
}

impl StoresMicroservice {
    pub fn find_published_products(&self) -> impl Future<Item = Vec<BaseProduct>, Error = FailureError> {
        let url = format!("{}/catalog", self.url);
        
            let http_client = self.http_client.clone();
            let headers = self.headers.clone();
            
        http_client
            .request(hyper::Method::Get, url, None, Some(headers))
            .map_err(FailureError::from)
            .map(|catalog: Catalog| catalog.base_products)
            
    }
}
