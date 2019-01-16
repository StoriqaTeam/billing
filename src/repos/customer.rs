use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::sql_types::Bool;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;

use repos::legacy_acl::*;

use models::authorization::*;
use models::UserId;
use models::{CustomerId, CustomersAccess, DbCustomer, NewDbCustomer, UpdateDbCustomer};

use schema::customers::dsl as CustomersDsl;

use super::acl;
use super::error::*;
use super::types::RepoResultV2;

type CustomersRepoAcl = Box<Acl<Resource, Action, Scope, FailureError, CustomersAccess>>;

#[derive(Debug)]
pub enum SearchCustomer {
    Id(CustomerId),
    UserId(UserId),
}

pub struct CustomersRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
    pub acl: CustomersRepoAcl,
}

pub trait CustomersRepo {
    fn get(&self, search: SearchCustomer) -> RepoResultV2<Option<DbCustomer>>;

    fn create(&self, payload: NewDbCustomer) -> RepoResultV2<DbCustomer>;

    fn update(&self, id: CustomerId, payload: UpdateDbCustomer) -> RepoResultV2<DbCustomer>;

    fn delete(&self, id: CustomerId) -> RepoResultV2<Option<DbCustomer>>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CustomersRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T, acl: CustomersRepoAcl) -> Self {
        Self { db_conn, acl }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CustomersRepo for CustomersRepoImpl<'a, T> {
    fn get(&self, search: SearchCustomer) -> RepoResultV2<Option<DbCustomer>> {
        debug!("Getting a customer by search term: {:?}", search);

        let search_exp: Box<BoxableExpression<CustomersDsl::customers, _, SqlType = Bool>> = match search {
            SearchCustomer::Id(customer_id) => Box::new(CustomersDsl::id.eq(customer_id)),
            SearchCustomer::UserId(user_id) => Box::new(CustomersDsl::user_id.eq(user_id)),
        };

        let query = CustomersDsl::customers.filter(search_exp);

        query
            .get_result(self.db_conn)
            .optional()
            .map_err(|e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, ErrorSource::Diesel, error_kind)
            })
            .and_then(|customer: Option<DbCustomer>| {
                if let Some(ref customer) = customer {
                    acl::check(&*self.acl, Resource::Customer, Action::Read, self, Some(&customer.into()))
                        .map_err(ectx!(try ErrorKind::Forbidden))?;
                };
                Ok(customer)
            })
    }

    fn create(&self, payload: NewDbCustomer) -> RepoResultV2<DbCustomer> {
        debug!("Create a customer with ID: {}", payload.id);
        acl::check(&*self.acl, Resource::Customer, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::insert_into(CustomersDsl::customers).values(&payload);

        command.get_result::<DbCustomer>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn update(&self, id_arg: CustomerId, payload: UpdateDbCustomer) -> RepoResultV2<DbCustomer> {
        debug!("Updating a customer with ID: {}", id_arg);
        acl::check(&*self.acl, Resource::Customer, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let filter = CustomersDsl::customers.filter(CustomersDsl::id.eq(&id_arg));

        let query = diesel::update(filter).set(&payload);
        query.get_result::<DbCustomer>(self.db_conn).map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }

    fn delete(&self, id_arg: CustomerId) -> RepoResultV2<Option<DbCustomer>> {
        debug!("Deleting a customer with ID: {}", id_arg);
        acl::check(&*self.acl, Resource::Customer, Action::Write, self, None).map_err(ectx!(try ErrorKind::Forbidden))?;

        let command = diesel::delete(CustomersDsl::customers.filter(CustomersDsl::id.eq(id_arg)));

        command.get_result::<DbCustomer>(self.db_conn).optional().map_err(|e| {
            let error_kind = ErrorKind::from(&e);
            ectx!(err e, ErrorSource::Diesel, error_kind)
        })
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> CheckScope<Scope, CustomersAccess>
    for CustomersRepoImpl<'a, T>
{
    fn is_in_scope(&self, user_id: stq_types::UserId, scope: &Scope, obj: Option<&CustomersAccess>) -> bool {
        match *scope {
            Scope::All => true,
            Scope::Owned => {
                if let Some(customers_access) = obj {
                    customers_access.user_id == user_id
                } else {
                    false
                }
            }
        }
    }
}
