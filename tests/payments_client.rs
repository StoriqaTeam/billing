extern crate billing_lib;
extern crate diesel;
extern crate failure;
extern crate futures;
extern crate hyper;
extern crate serde_json;
extern crate stq_http;
extern crate tokio_core;
extern crate uuid;

use billing_lib::client::payments::{self, PaymentsClientImpl};
use failure::Error as FailureError;
use futures::{Future, Stream};
use stq_http::client::{self, Client};
use tokio_core::reactor::Core;

#[test]
#[ignore]
fn payments_client_auth_works() {
    let payments_config = payments::Config {
        url: "https://pay-nightly.stq.cloud".to_string(),
        jwt_public_key_base64: "MIIBCgKCAQEAt3TQPCbcWM/Fba2s6V/WRuQv8SlEQp4F56fSY4LQ+yW2xY3f2fLOw/SurObHMQF7QpnQ7x/2zhGqe11Ad0MHbWG+OARR/B/76X8QhA3xEneZpgc8aykagl0Tr616tazEKr0JxGuUo3qHy+e/dqSQ9T04EjGqccfr3+gfmVZlzml2/kN2EmaFa28Q8NseY5a2TVL9XcEDHHpGHVpoRQI8ibfa92i2Lwo7E33Iz8hpbp+GgeyReua2z341nxSNqk0VSYa6KtNUk03G5YYmsrsoE+ECC69GAD07R4YcGqF4NRKSA0T3L8jY8rVbl5HUCIFuZynZEHWWpFvyMYW+9ffMfwIDAQAB".to_string(),
        user_jwt: "eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiJ9.eyJ1c2VyX2lkIjoxNjYsImV4cCI6MTU0NTI5NTA0NiwicHJvdmlkZXIiOiJFbWFpbCJ9.pJYiGjwmpCkwZdDkHyKI2_jglJDmEx3hR6aDYz6tFqavEjgCtbROkIn1_XAJJbxj_VtmchI3c095fGCEvBHb4IPbs9Txf-JdW8CAZ-MmHueHyxtXjwvCH51tOzkLXL13Fg_umlcJNLYC9XiHCnwkCWCgVoZztN4xwQAFir_uM07czMgFaCWA72kqaauRW7LuHcjVfJdIYxIQiHKtUkpt8W9-99zl4CaXq0raEMl9OA4u6-sR1cok72pHglSdeO3-fdQ7AayxdU0EWoB35DniOL0uzD-vE4ZXdlS7zbyes9fnw0sgU5XmbzhfwzIHvqq7QArPM4Eb8fnhYGai-CoVBw".to_string(),
        user_private_key: "a7190fcbbb97a08e0a0f39be542186efc3e59790b61b6338f83960f2519acb4d".to_string(),
    };

    let http_client_config = client::Config {
        http_client_buffer_size: 3,
        http_client_retries: 3,
        timeout_duration_ms: 5000,
    };

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let client = Client::new(&http_client_config, &handle);
    let client_handle = client.handle();
    let client_stream = client.stream();
    handle.spawn(client_stream.for_each(|_| Ok(())));

    let payments_client = PaymentsClientImpl::create_from_config(client_handle, payments_config).unwrap();
    let fut = payments_client
        .request_with_auth::<(), serde_json::Value>(hyper::Method::Get, "/v1/users/me", ())
        .map_err(FailureError::from)
        .then(|response| {
            println!("{:?}", response);
            assert!(response.is_ok());
            futures::future::ok::<(), FailureError>(())
        });

    core.run(fut).unwrap();
}
