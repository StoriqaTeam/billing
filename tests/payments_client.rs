extern crate billing_lib;
extern crate diesel;
extern crate failure;
extern crate futures;
extern crate hyper;
extern crate serde_json;
extern crate stq_http;
extern crate tokio_core;
extern crate uuid;

use billing_lib::client::payments::{self, CreateAccount, PaymentsClient, PaymentsClientImpl};
use billing_lib::models::Currency;
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
    let payments_config = payments::Config {
        url: "https://pay-nightly.stq.cloud".to_string(),
        jwt_public_key_base64: "MIIBCgKCAQEAt3TQPCbcWM/Fba2s6V/WRuQv8SlEQp4F56fSY4LQ+yW2xY3f2fLOw/SurObHMQF7QpnQ7x/2zhGqe11Ad0MHbWG+OARR/B/76X8QhA3xEneZpgc8aykagl0Tr616tazEKr0JxGuUo3qHy+e/dqSQ9T04EjGqccfr3+gfmVZlzml2/kN2EmaFa28Q8NseY5a2TVL9XcEDHHpGHVpoRQI8ibfa92i2Lwo7E33Iz8hpbp+GgeyReua2z341nxSNqk0VSYa6KtNUk03G5YYmsrsoE+ECC69GAD07R4YcGqF4NRKSA0T3L8jY8rVbl5HUCIFuZynZEHWWpFvyMYW+9ffMfwIDAQAB".to_string(),
        user_jwt: "eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiJ9.eyJ1c2VyX2lkIjoxNjcsImV4cCI6MTU0NTMwNzIzOCwicHJvdmlkZXIiOiJFbWFpbCJ9.saxj-CcymQim6FAI6j9RKravAyG_pRAUqJcQVs1nt50x_C6wPJh5mJkeK1zIhc8OSMUxy_JAnpbQSoGzmHaVh8RbRdIxhvLij04nElnXemjhk4recPNwhqmMWkimgyu87Y3eHcTvYKpNszLGKGKfB2or8We-5Ru78Ccr8U1UeftKoYVYjZocKT7XNffs1Hegj7i2sAvoJVIrY5tsc5bSF7QsheiQDYVcl0GMgoCaGxt8w9dP33Ge0mWIrwaVXOY_kC8CAvRLEr9SCC_EwpmxnBV2xnYUuq3RcvHB_maIW4YeL_vkRWdxl6MWlHfaiJk_Sz6XaRIONlsji8hGd4gbcw".to_string(),
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
        Box::new(client.list_accounts().map_err(FailureError::from).then(|response| {
            println!("{:?}", response);
            assert!(response.is_ok());
            futures::future::ok::<(), FailureError>(())
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
