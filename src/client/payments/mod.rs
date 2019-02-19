mod error;
pub mod mock;
mod types;

use chrono::Utc;
use failure::Fail;
use futures::{future, prelude::*, Future};
use hyper::{Headers, Method};
use secp256k1::{key::SecretKey, Message, Secp256k1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Debug;
use std::str::FromStr;
use std::sync::Arc;
use stq_http::client::HttpClient;
use uuid::Uuid;

use config;
use models::order_v2::ExchangeId;

pub use self::error::*;
use self::types::AccountResponse;
pub use self::types::{
    Account, CreateAccount, CreateExternalTransaction, CreateInternalTransaction, CreateTransactionRequestBody, Fee, FeesResponse, GetFees,
    GetRate, GetRateResponse, Rate, RateRefresh, RefreshRateResponse, TransactionsResponse,
};

pub trait PaymentsClient: Send + Sync + 'static {
    fn get_account(&self, account_id: Uuid) -> Box<Future<Item = Account, Error = Error> + Send>;

    fn list_accounts(&self) -> Box<Future<Item = Vec<Account>, Error = Error> + Send>;

    fn create_account(&self, input: CreateAccount) -> Box<Future<Item = Account, Error = Error> + Send>;

    fn delete_account(&self, account_id: Uuid) -> Box<Future<Item = (), Error = Error> + Send>;

    fn get_rate(&self, input: GetRate) -> Box<Future<Item = Rate, Error = Error> + Send>;

    fn refresh_rate(&self, exchange_id: ExchangeId) -> Box<Future<Item = RateRefresh, Error = Error> + Send>;

    fn get_fees(&self, input: GetFees) -> Box<Future<Item = FeesResponse, Error = Error> + Send>;

    fn get_transaction(&self, tx_id: Uuid) -> Box<Future<Item = Option<TransactionsResponse>, Error = Error> + Send>;

    fn create_external_transaction(&self, input: CreateExternalTransaction) -> Box<Future<Item = (), Error = Error> + Send>;

    fn create_internal_transaction(&self, input: CreateInternalTransaction) -> Box<Future<Item = (), Error = Error> + Send>;
}

impl<T: ?Sized + PaymentsClient> PaymentsClient for Arc<T> {
    fn get_account(&self, account_id: Uuid) -> Box<Future<Item = Account, Error = Error> + Send> {
        (*self.clone()).get_account(account_id)
    }

    fn list_accounts(&self) -> Box<Future<Item = Vec<Account>, Error = Error> + Send> {
        (*self.clone()).list_accounts()
    }

    fn create_account(&self, input: CreateAccount) -> Box<Future<Item = Account, Error = Error> + Send> {
        (*self.clone()).create_account(input)
    }

    fn delete_account(&self, account_id: Uuid) -> Box<Future<Item = (), Error = Error> + Send> {
        (*self.clone()).delete_account(account_id)
    }

    fn get_rate(&self, input: GetRate) -> Box<Future<Item = Rate, Error = Error> + Send> {
        (*self.clone()).get_rate(input)
    }

    fn refresh_rate(&self, exchange_id: ExchangeId) -> Box<Future<Item = RateRefresh, Error = Error> + Send> {
        (*self.clone()).refresh_rate(exchange_id)
    }

    fn get_fees(&self, input: GetFees) -> Box<Future<Item = FeesResponse, Error = Error> + Send> {
        (*self.clone()).get_fees(input)
    }

    fn get_transaction(&self, tx_id: Uuid) -> Box<Future<Item = Option<TransactionsResponse>, Error = Error> + Send> {
        (*self.clone()).get_transaction(tx_id)
    }

    fn create_external_transaction(&self, input: CreateExternalTransaction) -> Box<Future<Item = (), Error = Error> + Send> {
        (*self.clone()).create_external_transaction(input)
    }

    fn create_internal_transaction(&self, input: CreateInternalTransaction) -> Box<Future<Item = (), Error = Error> + Send> {
        (*self.clone()).create_internal_transaction(input)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub url: String,
    pub jwt_public_key_base64: String,
    pub user_jwt: String,
    pub user_private_key: String,
    pub device_id: String,
}

impl From<config::Payments> for Config {
    fn from(config: config::Payments) -> Self {
        let config::Payments {
            url,
            jwt_public_key_base64,
            user_jwt,
            user_private_key,
            device_id,
            ..
        } = config;
        Config {
            url,
            jwt_public_key_base64,
            user_jwt,
            user_private_key,
            device_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub user_id: u32,
    pub exp: u64,
    pub provider: String,
}

#[derive(Clone)]
pub struct PaymentsClientImpl<C: HttpClient + Clone> {
    client: C,
    url: String,
    user_id: u32,
    user_jwt: String,
    user_private_key: SecretKey,
    device_id: String,
}

impl<C: HttpClient + Clone + Send> PaymentsClientImpl<C> {
    const MAX_ACCOUNTS: u32 = 1_000_000;

    pub fn create_from_config(client: C, config: Config) -> Result<Self, Error> {
        let Config {
            url,
            jwt_public_key_base64,
            user_jwt,
            user_private_key,
            device_id,
        } = config;

        let jwt_public_key = base64::decode(jwt_public_key_base64.as_str()).map_err({
            let jwt_public_key_base64 = jwt_public_key_base64.clone();
            ectx!(try ErrorSource::Base64, ErrorKind::Internal => jwt_public_key_base64)
        })?;

        let jwt_validation = jwt::Validation {
            algorithms: vec![jwt::Algorithm::RS256],
            leeway: 60,
            ..jwt::Validation::default()
        };

        let user_id = jwt::decode::<JwtClaims>(&user_jwt, &jwt_public_key, &jwt_validation)
            .map_err({
                let user_jwt = user_jwt.clone();
                ectx!(
                    try ErrorSource::JsonWebToken,
                    ErrorKind::Internal => user_jwt, jwt_public_key_base64, &jwt::Validation::default()
                )
            })?
            .claims
            .user_id;

        let user_private_key =
            SecretKey::from_str(&user_private_key).map_err(ectx!(try ErrorSource::Secp256k1, ErrorKind::Internal => user_private_key))?;

        Ok(Self {
            client,
            url,
            user_id,
            user_jwt,
            user_private_key,
            device_id,
        })
    }

    pub fn request_with_auth<Req, Res>(&self, method: Method, query: String, body: Req) -> impl Future<Item = Res, Error = Error> + Send
    where
        Req: Debug + Serialize + Send + 'static,
        Res: for<'de> Deserialize<'de> + Send + 'static,
    {
        let self_clone = self.clone();
        serde_json::to_string(&body)
            .into_future()
            .map_err(ectx!(ErrorSource::SerdeJson, ErrorKind::Internal => body))
            .and_then({
                let device_id = self.device_id.clone();
                move |body| {
                    let timestamp = Utc::now().timestamp().to_string();

                    let mut hasher = Sha256::new();
                    hasher.input(&timestamp);
                    hasher.input(&device_id);
                    let hash = hasher.result();

                    Message::from_slice(&hash)
                        .map_err(ectx!(ErrorSource::Secp256k1, ErrorKind::Internal => hash))
                        .map(|message| (body, timestamp, device_id, message))
                }
            })
            .and_then(move |(body, timestamp, device_id, message)| {
                let signature = hex::encode(
                    Secp256k1::new()
                        .sign(&message, &self_clone.user_private_key)
                        .serialize_compact()
                        .to_vec(),
                );

                let mut headers = Headers::new();
                headers.set_raw("authorization", format!("Bearer {}", self_clone.user_jwt));
                headers.set_raw("timestamp", timestamp);
                headers.set_raw("device-id", device_id);
                headers.set_raw("sign", signature);

                let url = format!("{}{}", &self_clone.url, &query);
                self_clone
                    .client
                    .request_json::<Res>(method.clone(), url.clone(), Some(body.clone()), Some(headers.clone()))
                    .map_err(ectx!(
                        ErrorSource::StqHttp,
                        ErrorKind::Internal => method, url, Some(body), Some(headers)
                    ))
            })
    }
}

impl<C: Clone + HttpClient> PaymentsClient for PaymentsClientImpl<C> {
    fn get_account(&self, account_id: Uuid) -> Box<Future<Item = Account, Error = Error> + Send> {
        let query = format!("/v1/accounts/{}", account_id).to_string();
        Box::new(
            self.request_with_auth::<_, AccountResponse>(Method::Get, query.clone(), json!({}))
                .map_err(ectx!(ErrorKind::Internal => Method::Get, query, json!({})))
                .and_then(|res| AccountResponse::try_into_account(res.clone()).map_err(ectx!(ErrorKind::Internal => res))),
        )
    }

    fn list_accounts(&self) -> Box<Future<Item = Vec<Account>, Error = Error> + Send> {
        let query = format!("/v1/users/{}/accounts?offset=0&limit={}", self.user_id, Self::MAX_ACCOUNTS);
        Box::new(
            self.request_with_auth::<_, Vec<AccountResponse>>(Method::Get, query.clone(), json!({}))
                .map_err(ectx!(ErrorKind::Internal => Method::Get, query, json!({})))
                .and_then(|res| {
                    res.into_iter()
                        .map(|account_res| {
                            AccountResponse::try_into_account(account_res.clone()).map_err(ectx!(ErrorKind::Internal => account_res))
                        })
                        .collect::<Result<Vec<_>, _>>()
                }),
        )
    }

    fn create_account(&self, input: CreateAccount) -> Box<Future<Item = Account, Error = Error> + Send> {
        let query = format!("/v1/users/{}/accounts", self.user_id);
        Box::new(
            self.request_with_auth::<_, AccountResponse>(Method::Post, query.clone(), input.clone())
                .map_err(ectx!(ErrorKind::Internal => Method::Post, query, input))
                .and_then(|res| AccountResponse::try_into_account(res.clone()).map_err(ectx!(ErrorKind::Internal => res))),
        )
    }

    fn delete_account(&self, account_id: Uuid) -> Box<Future<Item = (), Error = Error> + Send> {
        let query = format!("/v1/accounts/{}", account_id);
        Box::new(
            self.request_with_auth::<_, ()>(Method::Delete, query.clone(), json!({}))
                .map_err(ectx!(ErrorKind::Internal => Method::Delete, query, json!({}))),
        )
    }

    fn get_rate(&self, input: GetRate) -> Box<Future<Item = Rate, Error = Error> + Send> {
        let query = format!("/v1/rate");
        Box::new(
            self.request_with_auth::<_, GetRateResponse>(Method::Post, query.clone(), input.clone())
                .map_err(ectx!(ErrorKind::Internal => Method::Post, query, input))
                .map(Rate::from),
        )
    }

    fn refresh_rate(&self, exchange_id: ExchangeId) -> Box<Future<Item = RateRefresh, Error = Error> + Send> {
        let query = format!("/v1/rate/refresh");
        Box::new(
            self.request_with_auth::<_, RefreshRateResponse>(Method::Post, query.clone(), json!({ "rateId": exchange_id }))
                .map_err(ectx!(ErrorKind::Internal => Method::Post, query, json!({ "rateId": exchange_id })))
                .map(RateRefresh::from),
        )
    }

    fn get_fees(&self, input: GetFees) -> Box<Future<Item = FeesResponse, Error = Error> + Send> {
        let query = format!("/v1/fees");
        Box::new(
            self.request_with_auth::<_, FeesResponse>(Method::Post, query.clone(), input.clone())
                .map_err(ectx!(ErrorKind::Internal => Method::Post, query.clone(), input.clone())),
        )
    }

    fn get_transaction(&self, tx_id: Uuid) -> Box<Future<Item = Option<TransactionsResponse>, Error = Error> + Send> {
        let query = format!("/v1/transactions/{}", tx_id);

        Box::new(
            self.request_with_auth::<_, Option<TransactionsResponse>>(Method::Get, query.clone(), json!({}))
                .map_err(ectx!(ErrorKind::Internal => Method::Get, query)),
        )
    }

    fn create_external_transaction(&self, input: CreateExternalTransaction) -> Box<Future<Item = (), Error = Error> + Send> {
        let body = CreateTransactionRequestBody::new_external(input, self.user_id.clone());
        let query = format!("/v1/transactions");

        Box::new(
            self.request_with_auth::<_, Option<TransactionsResponse>>(Method::Post, query.clone(), body.clone())
                .map_err(ectx!(ErrorKind::Internal => Method::Post, query, body))
                .map(|_| ()),
        )
    }

    fn create_internal_transaction(&self, input: CreateInternalTransaction) -> Box<Future<Item = (), Error = Error> + Send> {
        let CreateInternalTransaction { from, to, .. } = input;

        let fut = Future::join(self.get_account(from), self.get_account(to)).and_then({
            let self_ = self.clone();
            move |(from, to)| {
                if from.currency != to.currency {
                    let e = format_err!(
                        "Currency mismatch between accounts {} - {} and {} - {}",
                        from.currency,
                        from.id,
                        to.currency,
                        to.id
                    );
                    future::Either::A(future::err(ectx!(err e, ErrorKind::Internal)))
                } else {
                    let body = CreateTransactionRequestBody::new_internal(input, from.currency, self_.user_id);
                    let query = format!("/v1/transactions");
                    future::Either::B(
                        self_
                            .request_with_auth::<_, TransactionsResponse>(Method::Post, query.clone(), body.clone())
                            .map_err(ectx!(ErrorKind::Internal => Method::Post, query, body))
                            .map(|_| ()),
                    )
                }
            }
        });

        Box::new(fut)
    }
}
