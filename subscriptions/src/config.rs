use std::env;

use config_crate::{Config as RawConfig, ConfigError, Environment, File};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub interval_s: usize,
    pub file_name: String,
    pub cluster: String,
    pub thread_count: usize,

    pub stores_microservice: Microservice,
    pub billing_microservice: Microservice,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Microservice {
    pub url: String,
}

impl Config {
    /// Creates config from base.toml, which are overwritten by <env>.toml, where env is one of dev,
    /// k8s, nightly. After that it could be overwritten by env variables like STQ_SUBSCRIPTIONS
    /// (this will override `url` field in config).
    pub fn new() -> Result<Self, ConfigError> {
        // Optional file specific for environment
        let env = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        Config::with_env(env)
    }

    pub fn with_env(env: impl Into<String>) -> Result<Self, ConfigError> {
        let mut s = RawConfig::new();

        s.merge(File::with_name("config/base"))?;
        s.merge(File::with_name(&format!("config/{}", env.into())).required(false))?;
        s.merge(Environment::with_prefix("STQ_SUBSCRIPTIONS"))?;
        s.try_into()
    }
}
