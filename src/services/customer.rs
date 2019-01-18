//! CustomersService Services, presents CRUD operations with customers

use std::sync::Arc;

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};

use failure::Fail;
use futures::{future, Future, IntoFuture};
use stripe::{ParseIdError, PaymentSource};

use stq_http::client::HttpClient;

use client::payments::PaymentsClient;
use client::stripe::StripeClient;
use services::accounts::AccountService;

use models::{CustomerId, DbCustomer, NewDbCustomer};
use repos::{ReposFactory, SearchCustomer};
use services::error::{ErrorContext, ErrorKind};

use super::types::ServiceFutureV2;
use client::stripe::{ErrorKind as StripeErrorKind, NewCustomerWithSource};
use controller::context::DynamicContext;
use controller::requests::NewCustomerWithSourceRequest;
use controller::responses::{Card, CustomerResponse};

use services::types::spawn_on_pool;

pub trait CustomersService {
    /// Creates new customer with default payment source
    fn create_customer_with_source(&self, payload: NewCustomerWithSourceRequest) -> ServiceFutureV2<CustomerResponse>;

    /// Getting customer for current user
    fn get_customer(&self) -> ServiceFutureV2<Option<CustomerResponse>>;
}

pub struct CustomersServiceImpl<
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
    C: HttpClient + Clone,
    PC: PaymentsClient + Clone,
    AS: AccountService + Clone,
> {
    pub db_pool: Pool<M>,
    pub cpu_pool: CpuPool,
    pub repo_factory: F,
    pub stripe_client: Arc<dyn StripeClient>,
    pub dynamic_context: DynamicContext<C, PC, AS>,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > CustomersService for CustomersServiceImpl<T, M, F, C, PC, AS>
{
    fn create_customer_with_source(&self, payload: NewCustomerWithSourceRequest) -> ServiceFutureV2<CustomerResponse> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let stripe_client = self.stripe_client.clone();

        let fut = match user_id {
            Some(user_id) => future::Either::A(
                payload
                    .card_token
                    .parse()
                    .map_err(|e: ParseIdError| {
                        let stripe_err: StripeErrorKind = e.into();
                        ectx!(err stripe_err, ErrorKind::Internal)
                    })
                    .into_future()
                    .and_then(move |token| {
                        let payload_cloned = payload.clone();
                        let client_payload = NewCustomerWithSource {
                            email: payload_cloned.email,
                            token,
                        };

                        stripe_client
                            .create_customer_with_source(client_payload)
                            .map_err(ectx!(convert => payload))
                    })
                    .and_then(move |customer| {
                        spawn_on_pool(db_pool, cpu_pool, move |conn| {
                            let customers_repo = repo_factory.create_customers_repo(&conn, Some(user_id));

                            let new_customer = NewDbCustomer {
                                id: CustomerId::new(customer.id.clone()),
                                user_id: user_id,
                                email: customer.email.clone(),
                            };

                            customers_repo
                                .create(new_customer.clone())
                                .map_err(ectx!(convert => new_customer))
                                .map(move |db_customer| CustomerResponse {
                                    id: db_customer.id,
                                    user_id: db_customer.user_id,
                                    email: db_customer.email,
                                    cards: get_customer_cards(customer.sources.data),
                                })
                        })
                    }),
            ),
            _ => future::Either::B(future::err(ectx!(err ErrorContext::Unauthorized, ErrorKind::Forbidden))),
        };

        Box::new(fut)
    }

    fn get_customer(&self) -> ServiceFutureV2<Option<CustomerResponse>> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let stripe_client = self.stripe_client.clone();

        let fut = match user_id {
            Some(user_id) => future::Either::A(
                spawn_on_pool(db_pool, cpu_pool, move |conn| {
                    let customers_repo = repo_factory.create_customers_repo(&conn, Some(user_id));

                    customers_repo
                        .get(SearchCustomer::UserId(user_id))
                        .map_err(ectx!(convert => user_id))
                })
                .and_then(move |db_customer| {
                    db_customer.map(|value| {
                        let db_customer_id = value.id.clone();
                        stripe_client
                            .get_customer(value.id.clone())
                            .map_err(ectx!(convert => db_customer_id))
                            .map(move |customer| {
                                let DbCustomer { id, user_id, email, .. } = value;

                                CustomerResponse {
                                    id,
                                    user_id,
                                    email,
                                    cards: get_customer_cards(customer.sources.data),
                                }
                            })
                    })
                }),
            ),
            _ => future::Either::B(future::err(ectx!(err ErrorContext::Unauthorized, ErrorKind::Forbidden))),
        };

        Box::new(fut)
    }
}

fn get_customer_cards(elements: Vec<PaymentSource>) -> Vec<Card> {
    elements
        .into_iter()
        .filter_map(|data_element| match data_element {
            PaymentSource::Card(card) => Some(card.into()),
            PaymentSource::BankAccount(_) => {
                warn!("cannot get source for variant PaymentSource::BankAccount");
                None
            }
            PaymentSource::Source(_) => {
                warn!("cannot get source for variant PaymentSource::Source");
                None
            }
        })
        .collect()
}
