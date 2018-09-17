//! Users is a microservice responsible for authentication and managing user profiles.
//! This create is for running the service from `billing_lib`. See `billing_lib` for details.

extern crate billing_lib;
extern crate stq_logging;

fn main() {
    let config = billing_lib::config::Config::new().expect("Can't load app config!");

    // Prepare sentry integration
    let _sentry = billing_lib::sentry_integration::init(config.sentry.as_ref());

    // Prepare logger
    stq_logging::init(config.graylog.as_ref());

    billing_lib::start_server(config, &None, || ());
}
