mod error;
mod types;

use futures::{Future, IntoFuture};
use stripe::{CaptureParams, Charge, ChargeParams, Customer, CustomerParams, PaymentSourceParams};

use self::types::*;
use config;
use models::*;

pub use self::error::*;

pub trait StripeClient: Send + Sync + 'static {
    fn create_customer(&self, input: NewCustomer) -> Box<Future<Item = Customer, Error = Error> + Send>;

    fn create_customer_with_source(&self, input: NewCustomerWithSource) -> Box<Future<Item = Customer, Error = Error> + Send>;

    fn get_customer(&self, customer_id: CustomerId) -> Box<Future<Item = Customer, Error = Error> + Send>;

    fn create_charge(&self, input: NewCharge) -> Box<Future<Item = Charge, Error = Error> + Send>;

    fn get_charge(&self, charge_id: ChargeId) -> Box<Future<Item = Charge, Error = Error> + Send>;

    fn capture_charge(&self, charge_id: ChargeId) -> Box<Future<Item = Charge, Error = Error> + Send>;

    fn refund(&self, input: NewRefund) -> Box<Future<Item = Refund, Error = Error> + Send>;

    fn create_payout(&self, input: NewPayOut) -> Box<Future<Item = PayOut, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct StripeClientImpl {
    public_key: String,
    secret_key: String,
    client: stripe::Client,
}

impl StripeClientImpl {
    pub fn create_from_config(config: config::Config) -> Self {
        let secret_key = config.stripe.secret_key.clone();
        let client = stripe::Client::new(secret_key.clone());
        Self {
            public_key: config.stripe.public_key.clone(),
            secret_key,
            client,
        }
    }
}

impl StripeClient for StripeClientImpl {
    fn create_customer(&self, input: NewCustomer) -> Box<Future<Item = Customer, Error = Error> + Send> {
        Box::new(
            Customer::create(
                &self.client,
                CustomerParams {
                    email: Some(&input.email),
                    ..Default::default()
                },
            )
            .map_err(From::from)
            .into_future(),
        )
    }
    fn create_customer_with_source(&self, input: NewCustomerWithSource) -> Box<Future<Item = Customer, Error = Error> + Send> {
        Box::new(
            Customer::create(
                &self.client,
                CustomerParams {
                    email: Some(&input.email),
                    source: Some(PaymentSourceParams::Token(input.token)),
                    ..Default::default()
                },
            )
            .map_err(From::from)
            .into_future(),
        )
    }
    fn get_customer(&self, customer_id: CustomerId) -> Box<Future<Item = Customer, Error = Error> + Send> {
        Box::new(
            Customer::retrieve(&self.client, &customer_id.inner())
                .map_err(From::from)
                .into_future(),
        )
    }
    fn create_charge(&self, input: NewCharge) -> Box<Future<Item = Charge, Error = Error> + Send> {
        Box::new(
            input
                .currency
                .convert()
                .and_then(|currency| {
                    Charge::create(
                        &self.client,
                        ChargeParams {
                            amount: Some(input.amount.inner() as u64),
                            currency: Some(currency),
                            customer: Some(input.customer_id.inner()),
                            ..Default::default()
                        },
                    )
                    .map_err(From::from)
                })
                .into_future(),
        )
    }
    fn get_charge(&self, charge_id: ChargeId) -> Box<Future<Item = Charge, Error = Error> + Send> {
        Box::new(Charge::retrieve(&self.client, &charge_id.inner()).map_err(From::from).into_future())
    }
    fn capture_charge(&self, charge_id: ChargeId) -> Box<Future<Item = Charge, Error = Error> + Send> {
        Box::new(
            Charge::capture(&self.client, &charge_id.inner(), CaptureParams { ..Default::default() })
                .map_err(From::from)
                .into_future(),
        )
    }
    fn refund(&self, _input: NewRefund) -> Box<Future<Item = Refund, Error = Error> + Send> {
        unimplemented!()
    }
    fn create_payout(&self, _input: NewPayOut) -> Box<Future<Item = PayOut, Error = Error> + Send> {
        unimplemented!()
    }
}
