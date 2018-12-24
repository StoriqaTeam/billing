extern crate billing_lib;
extern crate diesel;
extern crate failure;
extern crate futures;
extern crate uuid;

use diesel::pg::PgConnection;
use diesel::Connection;

use billing_lib::models::{invoice_v2::*, Amount, Currency, UserId};
use billing_lib::repos::{legacy_acl::SystemACL, InvoicesV2Repo, InvoicesV2RepoImpl};

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
fn invoices_v2_repo_crud_happy() {
    let system_acl = Box::new(SystemACL::default());

    let invoice_id = InvoiceId::generate();
    let new_invoice = NewInvoice {
        id: invoice_id,
        account_id: None,
        buyer_currency: Currency::Stq,
        amount_captured: Amount::new(0),
        buyer_user_id: UserId::new(1),
        wallet_address: None,
    };

    let created_invoice = {
        let new_invoice = new_invoice.clone();
        let system_acl = system_acl.clone();
        with_test_db_conn(move |conn| InvoicesV2RepoImpl::new(conn, system_acl).create(new_invoice)).unwrap()
    };
    assert_eq!(new_invoice.id, created_invoice.id);

    let existing_invoice = {
        let new_invoice = new_invoice.clone();
        let system_acl = system_acl.clone();
        with_test_db_conn(move |conn| InvoicesV2RepoImpl::new(conn, system_acl).get(new_invoice.id)).unwrap()
    };
    assert_eq!(Some(new_invoice.id), existing_invoice.map(|a| a.id));

    let deleted_invoice = {
        let new_invoice = new_invoice.clone();
        let system_acl = system_acl.clone();
        with_test_db_conn(move |conn| {
            let repo = InvoicesV2RepoImpl::new(conn, system_acl);
            repo.delete(new_invoice.id)
        })
        .unwrap()
    };
    assert_eq!(Some(new_invoice.id), deleted_invoice.map(|a| a.id));
}
