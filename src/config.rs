//! Config module contains the top-level config for the app.
use std::env;

use config_crate::{Config as RawConfig, ConfigError, Environment, File};
use sentry_integration::SentryConfig;
use uuid::Uuid;

use stq_http;
use stq_logging::GrayLogConfig;

/// Basic settings - HTTP binding, saga and external billing addresses
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: Server,
    pub client: Client,
    pub saga_addr: SagaAddr,
    pub stores_microservice: StoresMicroservice,
    pub callback: Callback,
    pub external_billing: ExternalBilling,
    pub payments: Option<Payments>,
    pub payments_mock: PaymentsMock,
    pub graylog: Option<GrayLogConfig>,
    pub sentry: Option<SentryConfig>,
    pub stripe: Stripe,
    pub event_store: EventStore,
    pub fee: FeeValues,
    pub payment_expiry: PaymentExpiry,
}

/// Common server settings
#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub host: String,
    pub port: String,
    pub database: String,
    pub thread_count: usize,
    pub redis: Option<String>,
    pub cache_ttl_sec: u64,
    pub processing_timeout_ms: u32,
}

/// Http client settings
#[derive(Debug, Deserialize, Clone)]
pub struct Client {
    pub http_client_retries: usize,
    pub http_client_buffer_size: usize,
    pub http_timeout_ms: u64,
    pub dns_worker_thread_count: usize,
}

/// Saga microservice url
#[derive(Debug, Deserialize, Clone)]
pub struct SagaAddr {
    pub url: String,
}

/// Stores microservice url
#[derive(Debug, Deserialize, Clone)]
pub struct StoresMicroservice {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Callback {
    pub url: String,
}

/// External billing service url
#[derive(Debug, Deserialize, Clone)]
pub struct ExternalBilling {
    pub invoice_url: String,
    pub login_url: String,
    pub username: String,
    pub password: String,
    pub amount_recalculate_timeout_sec: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Payments {
    pub url: String,
    pub jwt_public_key_base64: String,
    pub user_jwt: String,
    pub user_private_key: String,
    pub device_id: String,
    pub min_pooled_accounts: u32,
    pub accounts: Accounts,
    pub sign_public_key: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PaymentsMock {
    pub use_mock: bool,
    pub min_pooled_accounts: u32,
    pub accounts: Accounts,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Accounts {
    pub main_stq: Uuid,
    pub main_eth: Uuid,
    pub main_btc: Uuid,
    pub cashback_stq: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Stripe {
    pub public_key: String,
    pub secret_key: String,
    pub signing_secret: String,
}

/// Event store processing settings
#[derive(Debug, Deserialize, Clone)]
pub struct EventStore {
    pub max_processing_attempts: u32,
    pub stuck_threshold_sec: u32,
    pub polling_rate_sec: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeeValues {
    pub order_percent: u64,
    pub currency_code: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PaymentExpiry {
    pub crypto_timeout_min: u32,
    pub fiat_timeout_min: u32,
}

/// Creates new app config struct
/// #Examples
/// ```
/// use billing_lib::config::*;
///
/// let config = Config::new();
/// ```
impl Config {
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = RawConfig::new();

        s.set_default("server.processing_timeout_ms", 1000i64).unwrap();
        s.set_default("event_store.max_processing_attempts", 3i64).unwrap();
        s.set_default("event_store.stuck_threshold_sec", 300i64).unwrap();
        s.set_default("event_store.polling_rate_sec", 10i64).unwrap();
        s.set_default("payment_expiry.crypto_timeout_min", 4320i64).unwrap();
        s.set_default("payment_expiry.fiat_timeout_min", 60i64).unwrap();
        s.set_default("payments_mock.use_mock", false).unwrap();
        s.set_default("payments_mock.min_pooled_accounts", 10).unwrap();
        s.set_default("payments_mock.accounts.main_stq", "cc3f3875-e719-427f-9b83-d4dae8d4263a")
            .unwrap();
        s.set_default("payments_mock.accounts.main_eth", "4fbaed3b-5e04-416b-8423-a21a457eeaa4")
            .unwrap();
        s.set_default("payments_mock.accounts.main_btc", "cd03fbed-6779-404d-a2a7-7ebee0a87dea")
            .unwrap();
        s.set_default("payments_mock.accounts.cashback_stq", "38d0017b-c2c9-4234-9154-e77c378998b8")
            .unwrap();

        s.merge(File::with_name("config/base"))?;

        // Note that this file is _optional_
        let env = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        s.merge(File::with_name(&format!("config/{}", env)).required(false))?;

        // Add in settings from the environment (with a prefix of STQ_BILLING)
        s.merge(Environment::with_prefix("STQ_BILLING"))?;

        s.try_into()
    }

    pub fn to_http_config(&self) -> stq_http::client::Config {
        stq_http::client::Config {
            http_client_buffer_size: self.client.http_client_buffer_size,
            http_client_retries: self.client.http_client_retries,
            timeout_duration_ms: self.client.http_timeout_ms,
        }
    }
}
