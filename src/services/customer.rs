//! CustomersService Services, presents CRUD operations with customers

use std::str::FromStr;
use std::sync::Arc;

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};

use failure::Fail;
use futures::{future, Future, IntoFuture};
use stripe::{CardTokenId, ParseIdError, PaymentSource, TokenId};

use stq_http::client::HttpClient;

use client::payments::PaymentsClient;
use client::stripe::StripeClient;
use services::accounts::AccountService;

use models::{CustomerId, DbCustomer, NewDbCustomer};
use repos::{ReposFactory, SearchCustomer};
use services::error::{Error, ErrorContext, ErrorKind};

use super::types::ServiceFutureV2;
use client::stripe::{ErrorKind as StripeErrorKind, NewCustomerWithSource, UpdateCustomer};
use controller::context::DynamicContext;
use controller::requests::{NewCustomerWithSourceRequest, UpdateCustomerRequest};
use controller::responses::{Card, CustomerResponse};

use services::types::spawn_on_pool;

pub trait CustomersService {
    /// Creates new customer with default payment source
    fn create_customer_with_source(&self, payload: NewCustomerWithSourceRequest) -> ServiceFutureV2<CustomerResponse>;

    /// Getting customer for current user
    fn get_customer(&self) -> ServiceFutureV2<Option<CustomerResponse>>;

    /// Delete customer for current user
    fn delete(&self, payload: CustomerId) -> ServiceFutureV2<()>;

    /// Update customer for current user
    fn update(&self, payload: UpdateCustomerRequest) -> ServiceFutureV2<CustomerResponse>;
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
        let repo_factory2 = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let db_pool2 = self.db_pool.clone();
        let cpu_pool2 = self.cpu_pool.clone();
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
                    if db_customer.is_some() {
                        let e = format_err!("Stripe Customer already exists for user_id {}", user_id);
                        future::Either::A(future::err(ectx!(err e, ErrorKind::Internal)))
                    } else {
                        future::Either::B(
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
                                    spawn_on_pool(db_pool2, cpu_pool2, move |conn| {
                                        let customers_repo = repo_factory2.create_customers_repo(&conn, Some(user_id));

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
                        )
                    }
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

    fn delete(&self, customer_id: CustomerId) -> ServiceFutureV2<()> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let stripe_client = self.stripe_client.clone();
        let customer_id_cloned = customer_id.clone();

        let fut = stripe_client
            .delete_customer(customer_id.clone())
            .map_err(ectx!(convert => customer_id_cloned))
            .and_then(move |deleted_customer| {
                spawn_on_pool(db_pool, cpu_pool, move |conn| {
                    let customers_repo = repo_factory.create_customers_repo(&conn, user_id);

                    if deleted_customer.deleted {
                        customers_repo
                            .delete(customer_id)
                            .map_err(ectx!(convert => deleted_customer.id))
                            .map(|_| ())
                    } else {
                        let e = format_err!("Cannot delete customer in stripe with id: {:?}", customer_id);
                        Err(ectx!(err e, ErrorKind::Internal))
                    }
                })
            });

        Box::new(fut)
    }

    fn update(&self, payload: UpdateCustomerRequest) -> ServiceFutureV2<CustomerResponse> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        let stripe_client = self.stripe_client.clone();

        let fut = user_id
            .ok_or_else(|| {
                let e = format_err!("No user was provided");
                ectx!(err e, ErrorKind::Forbidden)
            })
            .into_future()
            .and_then({
                let payload = payload.clone();
                move |user_id| {
                    spawn_on_pool(db_pool, cpu_pool, move |conn| {
                        let customers_repo = repo_factory.create_customers_repo(&conn, Some(user_id));
                        let customer = customers_repo
                            .get(SearchCustomer::UserId(user_id))
                            .map_err(ectx!(try convert => user_id))?
                            .ok_or_else(|| {
                                let e = format_err!("Customer for user {} not found", user_id);
                                ectx!(try err e, ErrorKind::NotFound)
                            })?;
                        let update_customer = customers_repo
                            .update(customer.id, payload.clone().into())
                            .map_err(ectx!(try convert => user_id))?;

                        Ok(update_customer)
                    })
                }
            })
            .and_then(move |customer| {
                try_from_request(payload)
                    .into_future()
                    .and_then({
                        let customer_id = customer.id.clone();
                        move |input| stripe_client.update_customer(customer_id, input).map_err(ectx!(convert))
                    })
                    .map(move |stripe_customer| (customer, stripe_customer))
            })
            .and_then(|(db_customer, stripe_customer)| {
                let DbCustomer { id, user_id, email, .. } = db_customer;

                Ok(CustomerResponse {
                    id,
                    user_id,
                    email,
                    cards: get_customer_cards(stripe_customer.sources.data),
                })
            });

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

fn try_from_request(payload: UpdateCustomerRequest) -> Result<UpdateCustomer, Error> {
    let token = match payload.card_token {
        Some(card_token) => Some(TokenId::Card(CardTokenId::from_str(&card_token).map_err(|_| {
            let e = format_err!("Could not counvert {} to stripe CardTokenId", card_token);
            ectx!(try err e, ErrorKind::Internal)
        })?)),
        None => None,
    };
    Ok(UpdateCustomer {
        email: payload.email,
        token,
    })
}
