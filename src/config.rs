//! Config module contains the top-level config for the app.

use config_crate::{Config as RawConfig, ConfigError, Environment, File};
use std::env;

/// Basic settings - HTTP binding, saga and external billing addresses
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: Server,
    pub client: Client,
    pub saga_addr: SagaAddr,
    pub callback: Callback,
    pub external_billing: ExternalBilling,
}

/// Common server settings
#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub host: String,
    pub port: String,
    pub database: String,
    pub thread_count: usize,
}

/// Http client settings
#[derive(Debug, Deserialize, Clone)]
pub struct Client {
    pub http_client_retries: usize,
    pub http_client_buffer_size: usize,
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
    pub create_order_url: String,
    pub create_merchant_url: String,
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
        s.merge(File::with_name("config/base"))?;

        // Note that this file is _optional_
        let env = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        s.merge(File::with_name(&format!("config/{}", env)).required(false))?;

        // Add in settings from the environment (with a prefix of STQ_BILLING)
        s.merge(Environment::with_prefix("STQ_BILLING"))?;

        s.try_into()
    }
}
