mod error;
mod types;
pub use self::types::{NewPaymentIntent, *};

use futures::Future;
use futures::IntoFuture;
use stripe::{
    CaptureParams, Charge, ChargeParams, Currency as StripeCurrency, Customer, CustomerParams, Deleted, Metadata, PaymentIntent,
    PaymentIntentCreateParams, PaymentSourceParams, Payout, PayoutParams, Refund, RefundParams,
};

use config;
use models::order_v2::OrderId;
use models::*;
use stq_types::stripe::PaymentIntentId;

pub use self::error::*;

pub trait StripeClient: Send + Sync + 'static {
    fn create_customer(&self, input: NewCustomer) -> Box<Future<Item = Customer, Error = Error> + Send>;

    fn create_customer_with_source(&self, input: NewCustomerWithSource) -> Box<Future<Item = Customer, Error = Error> + Send>;

    fn get_customer(&self, customer_id: CustomerId) -> Box<Future<Item = Customer, Error = Error> + Send>;

    fn delete_customer(&self, customer_id: CustomerId) -> Box<Future<Item = Deleted, Error = Error> + Send>;

    fn update_customer(&self, customer_id: CustomerId, input: UpdateCustomer) -> Box<Future<Item = Customer, Error = Error> + Send>;

    fn create_charge(&self, input: NewCharge) -> Box<Future<Item = Charge, Error = Error> + Send>;

    fn get_charge(&self, charge_id: ChargeId) -> Box<Future<Item = Charge, Error = Error> + Send>;

    fn capture_charge(&self, charge_id: ChargeId, amount: Amount) -> Box<Future<Item = Charge, Error = Error> + Send>;

    fn refund(&self, charge_id: ChargeId, amount: Amount, order_id: OrderId) -> Box<Future<Item = Refund, Error = Error> + Send>;

    fn create_payout(
        &self,
        amount: Amount,
        currency: StripeCurrency,
        order_id: OrderId,
    ) -> Box<Future<Item = Payout, Error = Error> + Send>;

    fn create_payment_intent(&self, input: NewPaymentIntent) -> Box<Future<Item = PaymentIntent, Error = Error> + Send>;

    fn cancel_payment_intent(&self, payment_intent_id: PaymentIntentId) -> Box<Future<Item = PaymentIntent, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct StripeClientImpl {
    public_key: String,
    secret_key: String,
    client: stripe::async::Client,
}

impl StripeClientImpl {
    pub fn create_from_config(config: &config::Config) -> Self {
        let secret_key = config.stripe.secret_key.clone();
        let client = stripe::async::Client::new(secret_key.clone());
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
            .map_err(From::from),
        )
    }

    fn create_customer_with_source(&self, input: NewCustomerWithSource) -> Box<Future<Item = Customer, Error = Error> + Send> {
        Box::new(
            Customer::create(
                &self.client,
                CustomerParams {
                    email: input.email.as_ref().map(|s| s.as_str()),
                    source: Some(PaymentSourceParams::Token(input.token)),
                    ..Default::default()
                },
            )
            .map_err(From::from),
        )
    }

    fn get_customer(&self, customer_id: CustomerId) -> Box<Future<Item = Customer, Error = Error> + Send> {
        Box::new(Customer::retrieve(&self.client, &customer_id.inner()).map_err(From::from))
    }

    fn delete_customer(&self, customer_id: CustomerId) -> Box<Future<Item = Deleted, Error = Error> + Send> {
        Box::new(Customer::delete(&self.client, &customer_id.inner()).map_err(From::from))
    }

    fn update_customer(&self, customer_id: CustomerId, input: UpdateCustomer) -> Box<Future<Item = Customer, Error = Error> + Send> {
        let customer_params = CustomerParams {
            email: input.email.as_ref().map(|e| e.as_ref()),
            source: input.token.map(|token| PaymentSourceParams::Token(token)),
            ..Default::default()
        };
        Box::new(Customer::update(&self.client, &customer_id.inner(), customer_params).map_err(From::from))
    }

    fn create_charge(&self, input: NewCharge) -> Box<Future<Item = Charge, Error = Error> + Send> {
        let client = self.client.clone();

        let fut = input.currency.convert().into_future().and_then(move |currency| {
            Charge::create(
                &client,
                ChargeParams {
                    amount: Some(input.amount.inner() as u64),
                    currency: Some(currency),
                    customer: Some(input.customer_id.inner()),
                    ..Default::default()
                },
            )
            .map_err(From::from)
        });
        Box::new(fut)
    }
    fn get_charge(&self, charge_id: ChargeId) -> Box<Future<Item = Charge, Error = Error> + Send> {
        Box::new(Charge::retrieve(&self.client, &charge_id.inner()).map_err(From::from))
    }
    fn capture_charge(&self, charge_id: ChargeId, amount: Amount) -> Box<Future<Item = Charge, Error = Error> + Send> {
        Box::new(
            Charge::capture(
                &self.client,
                &charge_id.inner(),
                CaptureParams {
                    amount: Some(amount.inner() as u64),
                    ..Default::default()
                },
            )
            .map_err(From::from),
        )
    }
    fn refund(&self, charge_id: ChargeId, amount: Amount, order_id: OrderId) -> Box<Future<Item = Refund, Error = Error> + Send> {
        let mut metadata = Metadata::new();
        metadata.insert("order_id".to_string(), format!("{}", order_id));
        Box::new(
            Refund::create(
                &self.client,
                RefundParams {
                    charge: &charge_id.inner(),
                    amount: Some(amount.inner() as u64),
                    metadata,
                    reason: None,
                    refund_application_fee: None,
                    reverse_transfer: None,
                },
            )
            .map_err(From::from),
        )
    }
    fn create_payout(
        &self,
        amount: Amount,
        currency: StripeCurrency,
        order_id: OrderId,
    ) -> Box<Future<Item = Payout, Error = Error> + Send> {
        let mut metadata = Metadata::new();
        metadata.insert("order_id".to_string(), format!("{}", order_id));
        Box::new(
            Payout::create(
                &self.client,
                PayoutParams {
                    amount: amount.inner() as u64,
                    metadata: Some(metadata),
                    currency,
                    ..Default::default()
                },
            )
            .map_err(From::from),
        )
    }

    fn create_payment_intent(&self, input: NewPaymentIntent) -> Box<Future<Item = PaymentIntent, Error = Error> + Send> {
        let params = PaymentIntentCreateParams {
            allowed_source_types: input.allowed_source_types,
            amount: input.amount,
            currency: input.currency,
            capture_method: input.capture_method,
            ..Default::default()
        };
        Box::new(PaymentIntent::create(&self.client, params).map_err(From::from))
    }

    fn cancel_payment_intent(&self, payment_intent_id: PaymentIntentId) -> Box<Future<Item = PaymentIntent, Error = Error> + Send> {
        Box::new(
            PaymentIntent::cancel(&self.client, &payment_intent_id.0, stripe::PaymentIntentCancelParams::default()).map_err(From::from),
        )
    }
}
