//! Config module contains the top-level config for the app.
use std::env;

use config_crate::{Config as RawConfig, ConfigError, Environment, File};

use sentry_integration::SentryConfig;

use stq_http;
use stq_logging::GrayLogConfig;

/// Basic settings - HTTP binding, saga and external billing addresses
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: Server,
    pub client: Client,
    pub saga_addr: SagaAddr,
    pub callback: Callback,
    pub external_billing: ExternalBilling,
    pub graylog: Option<GrayLogConfig>,
    pub sentry: Option<SentryConfig>,
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

#[derive(Debug, Deserialize, Clone)]
pub struct Callback {
    pub url: String,
}

/// External billing service url
#[derive(Debug, Deserialize, Clone)]
pub struct ExternalBilling {
    pub invoice_url: String,
    pub merchant_url: String,
    pub login_url: String,
    pub username: String,
    pub password: String,
    pub amount_recalculate_timeout_sec: i32,
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

        s.set_default("server.processing_timeout_ms", 1000 as i64).unwrap();

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
