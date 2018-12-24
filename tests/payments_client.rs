extern crate billing_lib;
extern crate diesel;
extern crate failure;
extern crate futures;
extern crate hyper;
extern crate serde_json;
extern crate stq_http;
extern crate tokio_core;
extern crate uuid;

use billing_lib::client::payments::{self, CreateAccount, GetRate, PaymentsClient, PaymentsClientImpl};
use billing_lib::models::{Amount, Currency};
use failure::Error as FailureError;
use futures::{Future, Stream};
use std::sync::Arc;
use stq_http::client::{self, Client};
use tokio_core::reactor::Core;
use uuid::Uuid;

fn with_payments_client<F, T>(f: F) -> Result<T, FailureError>
where
    F: FnOnce(Arc<dyn PaymentsClient>) -> Box<Future<Item = T, Error = FailureError>>,
{
    // Autotest user credentials: https://docs.google.com/document/d/1TE9ynEpIDElVGNOV0PAwSzqIodqJYCRYve4Mochxsnc
    // You will probably have to get a fresh token from the Storiqa GraphQL gateway service using the "getJWTByEmail" mutation with credentials from the Google doc
    // GraphQL mutation:
    // mutation {
    //     getJWTByEmail(input: { clientMutationId: "", email: "email@example.com", password: "pass" } ) {
    //         token
    //     }
    // }

    let payments_config = payments::Config {
        url: "https://pay-nightly.stq.cloud".to_string(),
        jwt_public_key_base64: "MIIBCgKCAQEAt3TQPCbcWM/Fba2s6V/WRuQv8SlEQp4F56fSY4LQ+yW2xY3f2fLOw/SurObHMQF7QpnQ7x/2zhGqe11Ad0MHbWG+OARR/B/76X8QhA3xEneZpgc8aykagl0Tr616tazEKr0JxGuUo3qHy+e/dqSQ9T04EjGqccfr3+gfmVZlzml2/kN2EmaFa28Q8NseY5a2TVL9XcEDHHpGHVpoRQI8ibfa92i2Lwo7E33Iz8hpbp+GgeyReua2z341nxSNqk0VSYa6KtNUk03G5YYmsrsoE+ECC69GAD07R4YcGqF4NRKSA0T3L8jY8rVbl5HUCIFuZynZEHWWpFvyMYW+9ffMfwIDAQAB".to_string(),
        user_jwt: "eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiJ9.eyJ1c2VyX2lkIjoxNjYsImV4cCI6MTU0NTc0NDg3NywicHJvdmlkZXIiOiJFbWFpbCJ9.LZwLgbxB2azZIQA_POG8c2iMsVjqgfOcdnB29xihMt_d41Xhwew7nsl3HiZJgNe86P0U7GECi0eXHP_jW9UUJRbGlAKRq7xB6AI4fN4n4jUFgzp_h8dflZ9KOWyhM8LAYyrJUqnS3aZS16WMfktyyTdONZ4igCgV-Zr8tmlXB4B1eik2w5I2_WruPL8hO6xObiiEViBFUT1oowv2wAVyBTtTb-SA5FvQNwi8vls9mWrhLIcpOoIvVLY6-ZTixJW6QkBd3SzbNBRJOf9_27B3nKgbALt0Z58ofpBE0s0MrsC9g1dwY4IPa7Yyya_8r35RH-t-8S-bDD71sjTFajqGdQ".to_string(),
        user_private_key: "a7190fcbbb97a08e0a0f39be542186efc3e59790b61b6338f83960f2519acb4d".to_string(),
        max_accounts: 1000,
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

    let payments_client = Arc::new(PaymentsClientImpl::create_from_config(client_handle, payments_config).unwrap());
    let fut = f(payments_client);

    core.run(fut)
}

#[test]
#[ignore]
fn payments_client_auth_works() {
    with_payments_client(|client| {
        Box::new(client.list_accounts().map_err(FailureError::from).map(|accounts| {
            println!("{:#?}", accounts);
            ()
        }))
    })
    .unwrap()
}

#[test]
#[ignore]
fn payments_client_rate_works() {
    with_payments_client(|client| {
        let id = Uuid::new_v4();
        let input = GetRate {
            id,
            from: Currency::Btc,
            to: Currency::Stq,
            amount_currency: Currency::Btc,
            amount: Amount::new(100_000_000), // 1 BTC
        };

        Box::new(client.get_rate(input).map_err(FailureError::from).map(|rate| {
            println!("{:#?}", rate);
            ()
        }))
    })
    .unwrap()
}

#[test]
#[ignore]
fn payments_client_account_crud_happy() {
    with_payments_client(|client| {
        let client = Arc::new(client);

        let input = CreateAccount {
            id: Uuid::new_v4(),
            currency: Currency::Stq,
            name: "Autotest account".to_string(),
            callback_url: String::default(),
        };

        Box::new(
            client
                .create_account(input)
                .and_then({
                    let client = client.clone();
                    move |account| client.get_account(account.id)
                })
                .and_then({
                    let client = client.clone();
                    move |account| client.list_accounts().map(|_| account)
                })
                .and_then(move |account| client.delete_account(account.id))
                .map(|_| ())
                .map_err(FailureError::from),
        )
    })
    .unwrap()
}
