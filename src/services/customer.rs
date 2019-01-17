//! CustomersService Services, presents CRUD operations with customers

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use r2d2::ManageConnection;

use failure::Fail;
use futures::{future, Future, Stream};
use stripe::ParseIdError;

use stq_http::client::HttpClient;

use client::payments::PaymentsClient;
use services::accounts::AccountService;

use models::{CustomerId, DbCustomer, NewDbCustomer};
use repos::ReposFactory;
use services::{error::ErrorKind, Service};

use super::types::ServiceFutureV2;
use client::stripe::{ErrorKind as StripeErrorKind, NewCustomerWithSource, StripeClient};
use controller::requests::NewCustomerWithSourceRequest;
use controller::responses::CustomerResponse;

use services::types::spawn_on_pool;

pub trait CustomersService {
    /// Creates new customer with default payment source
    fn create_customer_with_source(&self, payload: NewCustomerWithSourceRequest) -> ServiceFutureV2<CustomerResponse>;
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > CustomersService for Service<T, M, F, C, PC, AS>
{
    fn create_customer_with_source(&self, payload: NewCustomerWithSourceRequest) -> ServiceFutureV2<CustomerResponse> {
        let repo_factory = self.static_context.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;
        let db_pool = self.static_context.db_pool.clone();
        let cpu_pool = self.static_context.cpu_pool.clone();
        let stripe_client = self.static_context.stripe_client.clone();

        match user_id {
            Some(user_id) => payload
                .card_token
                .parse()
                .map_err(|e: ParseIdError| {
                    let stripe_err: StripeErrorKind = e.into();
                    ectx!(err stripe_err => payload)
                })
                .and_then(|token| {
                    let client_payload = NewCustomerWithSource {
                        email: payload.email,
                        token,
                    };

                    stripe_client
                        .create_customer_with_source(client_payload)
                        .map_err(ectx!(convert => payload))
                        .and_then(|customer| {
                            spawn_on_pool(db_pool, cpu_pool, move |conn| {
                                let customers_repo = repo_factory.create_customers_repo(&conn, Some(user_id));

                                let new_customer = NewDbCustomer {
                                    id: CustomerId::new(customer.id),
                                    user_id: user_id,
                                    email: customer.email,
                                };

                                customers_repo.create(new_customer).map_err(ectx!(convert => new_customer))
                            })
                        })
                }),
            _ => Box::new(future::err(ectx!(try ErrorKind::Forbidden))),
        }
    }
}
