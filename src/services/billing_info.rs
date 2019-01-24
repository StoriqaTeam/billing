//! BillingInfo Service, presents operations with billing info resource
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};
use validator::{ValidationError, ValidationErrors};

use failure::Fail;

use stq_http::client::HttpClient;
use stq_types::{BillingType, InternationalBillingId, RussiaBillingId, StoreId};

use client::payments::PaymentsClient;
use services::accounts::AccountService;

use models::*;
use repos::{InternationalBillingInfoRepo, ReposFactory, RussiaBillingInfoRepo, StoreBillingTypeRepo};
use services::error::{Error as ServiceError, ErrorContext, ErrorKind};

use super::types::ServiceFutureV2;
use controller::context::DynamicContext;

use services::types::spawn_on_pool;

pub trait BillingInfoService {
    fn create_international_billing_info(&self, payload: NewInternationalBillingInfo) -> ServiceFutureV2<InternationalBillingInfo>;
    fn update_international_billing_info(
        &self,
        id: InternationalBillingId,
        payload: UpdateInternationalBillingInfo,
    ) -> ServiceFutureV2<InternationalBillingInfo>;
    fn create_russia_billing_info(&self, payload: NewRussiaBillingInfo) -> ServiceFutureV2<RussiaBillingInfo>;
    fn update_russia_billing_info(&self, id: RussiaBillingId, payload: UpdateRussiaBillingInfo) -> ServiceFutureV2<RussiaBillingInfo>;
}

pub struct BillingInfoServiceImpl<
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
    pub dynamic_context: DynamicContext<C, PC, AS>,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > BillingInfoService for BillingInfoServiceImpl<T, M, F, C, PC, AS>
{
    fn create_international_billing_info(&self, payload: NewInternationalBillingInfo) -> ServiceFutureV2<InternationalBillingInfo> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let store_billing_type_repo = repo_factory.create_store_billing_type_repo(&conn, user_id);
            let international_billing_info_repo = repo_factory.create_international_billing_info_repo(&conn, user_id);
            let russia_billing_info_repo = repo_factory.create_russia_billing_info_repo(&conn, user_id);
            conn.transaction(move || {
                let store_id = payload.store_id;

                validate_create_international_billing_info(&*international_billing_info_repo, &payload)?;
                update_store_billing_type_to_international(&*store_billing_type_repo, &*russia_billing_info_repo, store_id)?;

                let created_info = international_billing_info_repo.create(payload).map_err(ectx!(try convert))?;
                Ok(created_info)
            })
        })
    }

    fn update_international_billing_info(
        &self,
        id: InternationalBillingId,
        payload: UpdateInternationalBillingInfo,
    ) -> ServiceFutureV2<InternationalBillingInfo> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let international_billing_info_repo = repo_factory.create_international_billing_info_repo(&conn, user_id);

            let updated = international_billing_info_repo
                .update(InternationalBillingInfoSearch::by_id(id), payload)
                .map_err(ectx!(try convert))?;

            Ok(updated)
        })
    }

    fn create_russia_billing_info(&self, payload: NewRussiaBillingInfo) -> ServiceFutureV2<RussiaBillingInfo> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let store_billing_type_repo = repo_factory.create_store_billing_type_repo(&conn, user_id);
            let international_billing_info_repo = repo_factory.create_international_billing_info_repo(&conn, user_id);
            let russia_billing_info_repo = repo_factory.create_russia_billing_info_repo(&conn, user_id);
            conn.transaction(move || {
                let store_id = payload.store_id;

                validate_create_russia_billing_info(&*russia_billing_info_repo, &payload)?;
                update_store_billing_type_to_russia(&*store_billing_type_repo, &*international_billing_info_repo, store_id)?;

                let created_info = russia_billing_info_repo.create(payload).map_err(ectx!(try convert))?;
                Ok(created_info)
            })
        })
    }

    fn update_russia_billing_info(&self, id: RussiaBillingId, payload: UpdateRussiaBillingInfo) -> ServiceFutureV2<RussiaBillingInfo> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let russia_billing_info_repo = repo_factory.create_russia_billing_info_repo(&conn, user_id);

            let updated = russia_billing_info_repo
                .update(RussiaBillingInfoSearch::by_id(id), payload)
                .map_err(ectx!(try convert))?;

            Ok(updated)
        })
    }
}

fn validate_create_international_billing_info(
    repo: &InternationalBillingInfoRepo,
    payload: &NewInternationalBillingInfo,
) -> Result<(), ServiceError> {
    let existing_info = repo
        .get(InternationalBillingInfoSearch::by_store_id(payload.store_id))
        .map_err(ectx!(try convert => payload))?;

    if existing_info.is_some() {
        let mut errors = ValidationErrors::new();
        let mut error = ValidationError::new("International billing info already exists");
        error.message = Some(format!("International billing info already exists for \"{}\"", payload.store_id,).into());
        errors.add("billing_info", error);
        return Err(ectx!(err ErrorContext::BillingInfo ,ErrorKind::Validation(serde_json::to_value(errors).unwrap_or_default())));
    }

    Ok(())
}

fn validate_create_russia_billing_info(repo: &RussiaBillingInfoRepo, payload: &NewRussiaBillingInfo) -> Result<(), ServiceError> {
    let existing_info = repo
        .get(RussiaBillingInfoSearch::by_store_id(payload.store_id))
        .map_err(ectx!(try convert => payload))?;

    if existing_info.is_some() {
        let mut errors = ValidationErrors::new();
        let mut error = ValidationError::new("Russia billing info already exists");
        error.message = Some(format!("Russia billing info already exists for \"{}\"", payload.store_id,).into());
        errors.add("billing_info", error);
        return Err(ectx!(err ErrorContext::BillingInfo ,ErrorKind::Validation(serde_json::to_value(errors).unwrap_or_default())));
    }

    Ok(())
}

fn update_store_billing_type_to_international(
    store_billing_type_repo: &StoreBillingTypeRepo,
    russia_billing_info_repo: &RussiaBillingInfoRepo,
    store_id: StoreId,
) -> Result<(), ServiceError> {
    let billing_type = store_billing_type_repo
        .get(StoreBillingTypeSearch::by_store_id(store_id))
        .map_err(ectx!(try convert))?;

    match billing_type {
        None => {
            store_billing_type_repo
                .create(NewStoreBillingType {
                    store_id,
                    billing_type: BillingType::International,
                })
                .map_err(ectx!(try convert))?;
        }

        Some(StoreBillingType {
            billing_type: BillingType::International,
            ..
        }) => {
            //do nothing
        }

        Some(StoreBillingType {
            billing_type: BillingType::Russia,
            ..
        }) => {
            russia_billing_info_repo
                .delete(RussiaBillingInfoSearch::by_store_id(store_id))
                .map_err(ectx!(try convert))?;
            store_billing_type_repo
                .update(
                    StoreBillingTypeSearch::by_store_id(store_id),
                    UpdateStoreBillingType {
                        billing_type: Some(BillingType::International),
                        ..Default::default()
                    },
                )
                .map_err(ectx!(try convert))?;
        }
    }

    Ok(())
}

fn update_store_billing_type_to_russia(
    store_billing_type_repo: &StoreBillingTypeRepo,
    international_billing_info_repo: &InternationalBillingInfoRepo,
    store_id: StoreId,
) -> Result<(), ServiceError> {
    let billing_type = store_billing_type_repo
        .get(StoreBillingTypeSearch::by_store_id(store_id))
        .map_err(ectx!(try convert))?;

    match billing_type {
        None => {
            store_billing_type_repo
                .create(NewStoreBillingType {
                    store_id,
                    billing_type: BillingType::Russia,
                })
                .map_err(ectx!(try convert))?;
        }

        Some(StoreBillingType {
            billing_type: BillingType::Russia,
            ..
        }) => {
            //do nothing
        }

        Some(StoreBillingType {
            billing_type: BillingType::International,
            ..
        }) => {
            international_billing_info_repo
                .delete(InternationalBillingInfoSearch::by_store_id(store_id))
                .map_err(ectx!(try convert))?;
            store_billing_type_repo
                .update(
                    StoreBillingTypeSearch::by_store_id(store_id),
                    UpdateStoreBillingType {
                        billing_type: Some(BillingType::Russia),
                        ..Default::default()
                    },
                )
                .map_err(ectx!(try convert))?;
        }
    }

    Ok(())
}
