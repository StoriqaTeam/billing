//! PaymentIntentService Services, presents CRUD operations with payment_intent
use std::sync::Arc;

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use futures::{Future, IntoFuture};
use futures_cpupool::CpuPool;
use r2d2::{ManageConnection, Pool};
use validator::{ValidationError, ValidationErrors};

use failure::Fail;

use stq_http::client::HttpClient;
use stq_types::stripe::PaymentIntentId;

use client::payments::PaymentsClient;
use client::stripe::{NewPaymentIntent as StripeClientNewPaymentIntent, StripeClient};
use controller::context::DynamicContext;
use models::invoice_v2::InvoiceId;
use models::*;
use services::accounts::AccountService;

use repos::{ReposFactory, SearchFee, SearchPaymentIntent, SearchPaymentIntentInvoice};
use services::{Error as ServiceError, ErrorContext, ErrorKind};

use controller::responses::PaymentIntentResponse;

use super::types::ServiceFutureV2;

use services::types::spawn_on_pool;

pub trait PaymentIntentService {
    /// Returns payment intent object by invoice ID
    fn get_by_invoice(&self, invoice_id: InvoiceId) -> ServiceFutureV2<Option<PaymentIntentResponse>>;
    /// Create payment intent object by fee ID
    fn create_by_fee(&self, fee_id: FeeId) -> ServiceFutureV2<PaymentIntentResponse>;
}

pub struct PaymentIntentServiceImpl<
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
    pub stripe_client: Arc<dyn StripeClient>,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
        C: HttpClient + Clone,
        PC: PaymentsClient + Clone,
        AS: AccountService + Clone,
    > PaymentIntentService for PaymentIntentServiceImpl<T, M, F, C, PC, AS>
{
    fn get_by_invoice(&self, invoice_id: InvoiceId) -> ServiceFutureV2<Option<PaymentIntentResponse>> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let payment_intent_repo = repo_factory.create_payment_intent_repo(&conn, user_id);
            debug!("Requesting payment intent by invoice id: {}", invoice_id);
            let payment_intent_invoices_repo = repo_factory.create_payment_intent_invoices_repo(&conn, user_id);

            let payment_intent_invoice = payment_intent_invoices_repo
                .get(SearchPaymentIntentInvoice::InvoiceId(invoice_id))
                .map_err(ectx!(try convert => invoice_id))?;

            if let Some(payment_intent_invoice) = payment_intent_invoice {
                payment_intent_repo
                    .get(SearchPaymentIntent::Id(payment_intent_invoice.payment_intent_id))
                    .map_err(ectx!(convert => invoice_id))
                    .and_then(|payment_intent| {
                        if let Some(value) = payment_intent {
                            PaymentIntentResponse::try_from_payment_intent(value).map(|res| Some(res))
                        } else {
                            Ok(None)
                        }
                    })
            } else {
                Ok(None)
            }
        })
    }

    fn create_by_fee(&self, fee_id: FeeId) -> ServiceFutureV2<PaymentIntentResponse> {
        let repo_factory = self.repo_factory.clone();
        let user_id = self.dynamic_context.user_id;

        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();

        let stripe_client = self.stripe_client.clone();

        let fut = spawn_on_pool(db_pool, cpu_pool, move |conn| {
            let fee_repo = repo_factory.create_fees_repo(&conn, user_id);

            let fee = fee_repo.get(SearchFee::Id(fee_id)).map_err(ectx!(try convert))?.ok_or_else(|| {
                let e = format_err!("Fee with id {} not found", fee_id);
                ectx!(try err e, ErrorKind::NotFound)
            })?;
            validate_payment_intent_create_fee(&fee)?;
            Ok(fee)
        })
        .and_then(move |fee| create_fee_payment_intent(stripe_client, fee))
        .and_then({
            let repo_factory = self.repo_factory.clone();

            let db_pool = self.db_pool.clone();
            let cpu_pool = self.cpu_pool.clone();

            move |(new_payment_intent, new_payment_intent_fee)| {
                spawn_on_pool(db_pool, cpu_pool, move |conn| {
                    let payment_intent_fees_repo = repo_factory.create_payment_intent_fees_repo(&conn, user_id);
                    let payment_intent_repo = repo_factory.create_payment_intent_repo(&conn, user_id);
                    conn.transaction(move || {
                        payment_intent_fees_repo
                            .create(new_payment_intent_fee)
                            .map_err(ectx!(try convert))?;
                        let payment_intent = payment_intent_repo.create(new_payment_intent).map_err(ectx!(try convert))?;
                        Ok(payment_intent)
                    })
                })
            }
        })
        .and_then(PaymentIntentResponse::try_from_payment_intent);

        Box::new(fut)
    }
}

fn validate_payment_intent_create_fee(fee: &Fee) -> Result<(), ServiceError> {
    match &fee.status {
        illegal_status @ FeeStatus::Paid | illegal_status @ FeeStatus::Fail => {
            let mut errors = ValidationErrors::new();
            let mut error = ValidationError::new("Can not create payment intent");
            error.message = Some(format!("Can not create payment intent with fee status \"{:?}\"", illegal_status).into());
            errors.add("fee_id", error);
            return Err(ectx!(err ErrorContext::FeeState ,ErrorKind::Validation(serde_json::to_value(errors).unwrap_or_default())));
        }
        FeeStatus::NotPaid => {
            //do nothing
        }
    }

    Ok(())
}

fn create_fee_payment_intent(stripe_client: Arc<dyn StripeClient>, fee: Fee) -> ServiceFutureV2<(NewPaymentIntent, NewPaymentIntentFee)> {
    let fee_id = fee.id;
    let fut = payment_intent_create_params(fee)
        .into_future()
        .and_then(move |payment_intent_creation| {
            stripe_client
                .create_payment_intent(payment_intent_creation)
                .map_err(ectx!(convert => fee_id))
        })
        .and_then(move |stripe_payment_intent| new_payment_intent(fee_id, stripe_payment_intent));

    Box::new(fut)
}

fn payment_intent_create_params(fee: Fee) -> Result<StripeClientNewPaymentIntent, ServiceError> {
    Ok(StripeClientNewPaymentIntent {
        allowed_source_types: vec![stripe::PaymentIntentSourceType::Card],
        amount: fee.amount.into(),
        currency: fee.currency.try_into_stripe_currency().map_err(|_| {
            let e = format_err!("Fee with id {} - could not convet currency: {}", fee.id, fee.currency);
            ectx!(try err e, ErrorKind::Internal)
        })?,
        capture_method: Some(stripe::CaptureMethod::Manual),
    })
}

fn new_payment_intent(
    fee_id: FeeId,
    stripe_payment_intent: stripe::PaymentIntent,
) -> Result<(NewPaymentIntent, NewPaymentIntentFee), ServiceError> {
    let payment_intent = NewPaymentIntent {
        id: PaymentIntentId(stripe_payment_intent.id.clone()),
        amount: stripe_payment_intent.amount.into(),
        amount_received: stripe_payment_intent.amount_received.into(),
        client_secret: stripe_payment_intent.client_secret,
        currency: Currency::try_from_stripe_currency(stripe_payment_intent.currency).map_err({
            let e = format_err!(
                "Payment intent for invoice with ID: {} can not convert currency: {}",
                fee_id,
                stripe_payment_intent.currency,
            );
            move |_| ectx!(try err e, ErrorKind::Internal)
        })?,
        last_payment_error_message: stripe_payment_intent.last_payment_error.map(|err| format!("{:?}", err)),
        receipt_email: stripe_payment_intent.receipt_email,
        charge_id: stripe_payment_intent
            .charges
            .data
            .into_iter()
            .next()
            .map(|charge| ChargeId::new(charge.id)),
        status: stripe_payment_intent.status.into(),
    };

    let payment_intent_invoice = NewPaymentIntentFee {
        fee_id,
        payment_intent_id: PaymentIntentId(stripe_payment_intent.id),
    };

    Ok((payment_intent, payment_intent_invoice))
}
