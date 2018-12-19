mod error;
mod types;

use chrono::Utc;
use failure::Fail;
use futures::prelude::*;
use hyper::{Headers, Method};
use secp256k1::{key::SecretKey, Message, Secp256k1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Debug;
use std::str::FromStr;
use stq_http::client::HttpClient;

use config;

use self::error::*;
use self::types::*;

pub trait PaymentsClient: Send + Sync + 'static {
    fn create_account(&self, input: CreateAccountRequest) -> Box<Future<Item = CreateAccountResponse, Error = Error> + Send>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub url: String,
    pub jwt_public_key_base64: String,
    pub user_jwt: String,
    pub user_private_key: String,
}

impl From<config::Payments> for Config {
    fn from(config: config::Payments) -> Self {
        let config::Payments {
            url,
            jwt_public_key_base64,
            user_jwt,
            user_private_key,
        } = config;
        Config {
            url,
            jwt_public_key_base64,
            user_jwt,
            user_private_key,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub user_id: u64,
    pub exp: u64,
    pub provider: String,
}

#[derive(Clone)]
pub struct PaymentsClientImpl<C: HttpClient + Clone> {
    client: C,
    url: String,
    user_id: u64,
    user_jwt: String,
    user_private_key: SecretKey,
}

impl<C: HttpClient + Clone + Send> PaymentsClientImpl<C> {
    pub fn create_from_config(client: C, config: Config) -> Result<Self, Error> {
        let Config {
            url,
            jwt_public_key_base64,
            user_jwt,
            user_private_key,
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
        })
    }

    pub fn request_with_auth<Req, Res>(
        &self,
        method: Method,
        query: &'static str,
        body: Req,
    ) -> impl Future<Item = Res, Error = Error> + Send + '_
    where
        Req: Debug + Serialize + Send + 'static,
        Res: for<'de> Deserialize<'de> + Send + 'static,
    {
        serde_json::to_string(&body)
            .into_future()
            .map_err(ectx!(ErrorSource::SerdeJson, ErrorKind::Internal => body))
            .and_then(|body| {
                let timestamp = Utc::now().timestamp().to_string();
                let device_id = "";

                let mut hasher = Sha256::new();
                hasher.input(&timestamp);
                hasher.input(&device_id);
                let hash = hasher.result();

                Message::from_slice(&hash)
                    .map_err(ectx!(ErrorSource::Secp256k1, ErrorKind::Internal => hash))
                    .map(|message| (body, timestamp, device_id, message))
            })
            .and_then(move |(body, timestamp, device_id, message)| {
                let signature = hex::encode(Secp256k1::new().sign(&message, &self.user_private_key).serialize_compact().to_vec());

                let mut headers = Headers::new();
                headers.set_raw("authorization", format!("Bearer {}", self.user_jwt));
                headers.set_raw("timestamp", timestamp);
                headers.set_raw("device-id", device_id);
                headers.set_raw("sign", signature);

                let url = format!("{}{}", &self.url, &query);
                self.client
                    .request_json::<Res>(method.clone(), url.clone(), Some(body.clone()), Some(headers.clone()))
                    .map_err(ectx!(
                        ErrorSource::StqHttp,
                        ErrorKind::Internal => method, url, Some(body), Some(headers)
                    ))
            })
    }
}

impl<C: Clone + HttpClient> PaymentsClient for PaymentsClientImpl<C> {
    fn create_account(&self, _input: CreateAccountRequest) -> Box<Future<Item = CreateAccountResponse, Error = Error> + Send> {
        unimplemented!()
    }
}
