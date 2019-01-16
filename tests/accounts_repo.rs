extern crate billing_lib;
extern crate diesel;
extern crate failure;
extern crate futures;
extern crate uuid;

use diesel::pg::PgConnection;
use diesel::Connection;
use uuid::Uuid;

use billing_lib::models::{AccountId, NewAccount, TureCurrency};
use billing_lib::repos::{legacy_acl::SystemACL, AccountsRepo, AccountsRepoImpl};

fn with_test_db_conn<F, T>(f: F) -> T
where
    F: FnOnce(&PgConnection) -> T,
{
    let config = billing_lib::config::Config::new().unwrap();
    let database_url = config.server.database.parse::<String>().unwrap();
    let db_conn = PgConnection::establish(&database_url).unwrap();

    f(&db_conn)
}

#[test]
fn accounts_repo_crud_happy() {
    let system_acl = Box::new(SystemACL::default());

    let new_account = NewAccount {
        id: AccountId::new(Uuid::new_v4()),
        currency: TureCurrency::Stq,
        is_pooled: false,
        wallet_address: Some("0x0".to_string().into()),
    };

    let created_account = {
        let new_account = new_account.clone();
        let system_acl = system_acl.clone();
        with_test_db_conn(move |conn| AccountsRepoImpl::new(conn, system_acl).create(new_account)).unwrap()
    };
    assert_eq!(new_account.id, created_account.id);

    let existing_account = {
        let new_account = new_account.clone();
        let system_acl = system_acl.clone();
        with_test_db_conn(move |conn| AccountsRepoImpl::new(conn, system_acl).get(new_account.id)).unwrap()
    };
    assert_eq!(Some(new_account.id), existing_account.map(|a| a.id));

    let nonexisting_account = {
        let system_acl = system_acl.clone();
        with_test_db_conn(move |conn| AccountsRepoImpl::new(conn, system_acl).get(AccountId::new(Uuid::nil()))).unwrap()
    };
    assert_eq!(None, nonexisting_account.map(|a| a.id));

    let deleted_account = {
        let new_account = new_account.clone();
        let system_acl = system_acl.clone();
        with_test_db_conn(move |conn| {
            let repo = AccountsRepoImpl::new(conn, system_acl);
            repo.delete(new_account.id)
        })
        .unwrap()
    };
    assert_eq!(Some(new_account.id), deleted_account.map(|a| a.id));
}
