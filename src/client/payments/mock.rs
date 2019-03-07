use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Utc};
use futures::{future, Future, IntoFuture};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use super::error::*;
use super::types::{Account, Fee, *};
use super::PaymentsClient;
use models::order_v2::ExchangeId;
use models::*;

#[derive(Clone)]
struct State {
    accounts: HashMap<Uuid, Account>,
    txs: HashMap<Uuid, TransactionsResponse>,
    rates: HashMap<Uuid, Rate>,
}

impl Default for State {
    fn default() -> Self {
        State {
            accounts: HashMap::default(),
            txs: HashMap::default(),
            rates: HashMap::default(),
        }
    }
}

#[derive(Clone)]
pub struct MockPaymentsClient {
    state: Arc<Mutex<State>>,
}

impl Default for MockPaymentsClient {
    fn default() -> Self {
        MockPaymentsClient {
            state: Arc::new(Mutex::new(State::default())),
        }
    }
}

impl PaymentsClient for MockPaymentsClient {
    fn get_account(&self, account_id: Uuid) -> Box<Future<Item = Account, Error = Error> + Send> {
        let state = self.state.clone();
        let state = state.lock().unwrap();

        let fut = (*state)
            .accounts
            .get(&account_id)
            .cloned()
            .ok_or(ErrorKind::Internal.into())
            .into_future();

        Box::new(fut)
    }

    fn list_accounts(&self) -> Box<Future<Item = Vec<Account>, Error = Error> + Send> {
        let state = self.state.clone();
        let state = state.lock().unwrap();
        let accounts = (*state).accounts.values().cloned().collect();

        Box::new(future::ok(accounts))
    }

    fn create_account(&self, input: CreateAccount) -> Box<Future<Item = Account, Error = Error> + Send> {
        let CreateAccount {
            id,
            currency,
            name,
            callback_url: _,
            ..
        } = input;

        let balance = match currency {
            // 1 000 ETH
            TureCurrency::Eth => Amount::new(1_000_000_000_000_000_000_000u128),
            // 100 000 000 STQ
            TureCurrency::Stq => Amount::new(100_000_000_000_000_000_000_000_000u128),
            // 100 BTC
            TureCurrency::Btc => Amount::new(10_000_000_000u128),
        };

        let account = Account {
            id,
            balance,
            currency,
            name: name.clone(),
            account_address: WalletAddress::new("wallet_address_".to_owned() + &name),
        };

        let state = self.state.clone();
        let mut state = state.lock().unwrap();
        (*state).accounts.insert(id, account.clone());

        Box::new(future::ok(account))
    }

    fn delete_account(&self, account_id: Uuid) -> Box<Future<Item = (), Error = Error> + Send> {
        let state = self.state.clone();
        let mut state = state.lock().unwrap();
        (*state).accounts.remove(&account_id);

        Box::new(future::ok(()))
    }

    fn get_rate(&self, input: GetRate) -> Box<Future<Item = Rate, Error = Error> + Send> {
        let GetRate {
            id,
            from,
            to,
            amount_currency: _,
            amount,
        } = input;

        let rate = match (from, to) {
            (TureCurrency::Stq, TureCurrency::Btc) => BigDecimal::from(0.00000007),
            (TureCurrency::Stq, TureCurrency::Eth) => BigDecimal::from(0.000001),
            (TureCurrency::Btc, TureCurrency::Stq) => BigDecimal::from(1.0 / 0.00000007),
            (TureCurrency::Btc, TureCurrency::Eth) => BigDecimal::from(26),
            (TureCurrency::Eth, TureCurrency::Stq) => BigDecimal::from(1.0 / 0.000001),
            (TureCurrency::Eth, TureCurrency::Btc) => BigDecimal::from(0.04),
            _ => BigDecimal::from(1),
        };

        let rate = Rate {
            id,
            from,
            to,
            amount,
            rate,
            expiration: NaiveDateTime::new(NaiveDate::from_ymd(2100, 1, 1), NaiveTime::from_hms(0, 0, 0)),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };

        let state = self.state.clone();
        let mut state = state.lock().unwrap();
        (*state).rates.insert(id, rate.clone());

        Box::new(future::ok(rate))
    }

    fn refresh_rate(&self, id: ExchangeId) -> Box<Future<Item = RateRefresh, Error = Error> + Send> {
        let id = id.inner().clone();

        let state = self.state.clone();
        let state = state.lock().unwrap();

        let fut = (*state)
            .rates
            .get(&id)
            .cloned()
            .ok_or(ErrorKind::Internal.into())
            .into_future()
            .map(|rate| RateRefresh { rate, is_new_rate: false });

        Box::new(fut)
    }

    fn get_fees(&self, input: GetFees) -> Box<Future<Item = FeesResponse, Error = Error> + Send> {
        let GetFees {
            currency,
            account_address: _,
        } = input;

        Box::new(future::ok(FeesResponse {
            currency,
            fees: vec![
                Fee {
                    value: BigDecimal::from(1),
                    estimated_time: 3600,
                },
                Fee {
                    value: BigDecimal::from(100),
                    estimated_time: 60,
                },
                Fee {
                    value: BigDecimal::from(10000),
                    estimated_time: 1,
                },
            ],
        }))
    }

    fn get_transaction(&self, tx_id: Uuid) -> Box<Future<Item = Option<TransactionsResponse>, Error = Error> + Send> {
        let state = self.state.clone();
        let state = state.lock().unwrap();

        Box::new(future::ok((*state).txs.get(&tx_id).cloned()))
    }

    fn create_external_transaction(&self, input: CreateExternalTransaction) -> Box<Future<Item = (), Error = Error> + Send> {
        let CreateExternalTransaction {
            id,
            from,
            to,
            amount,
            currency,
            fee,
        } = input;

        let tx = TransactionsResponse {
            id,
            from: vec![TransactionAddressInfo {
                account_id: Some(from),
                owner_name: None,
                blockchain_address: String::default(),
            }],
            to: TransactionAddressInfo {
                account_id: None,
                owner_name: None,
                blockchain_address: to.into_inner(),
            },
            from_value: amount.to_string(),
            from_currency: currency.clone(),
            to_value: amount.to_string(),
            to_currency: currency.clone(),
            fee: fee.to_string(),
            status: "completed".to_owned(),
        };

        let state = self.state.clone();
        let mut state = state.lock().unwrap();
        (*state).txs.insert(id, tx.clone());

        Box::new(future::ok(()))
    }

    fn create_internal_transaction(&self, input: CreateInternalTransaction) -> Box<Future<Item = (), Error = Error> + Send> {
        let validation_err = |msg| ErrorKind::Validation(json!(msg)).into();

        let result_fn = move || {
            let CreateInternalTransaction { id, from, to, amount } = input;

            let state = self.state.clone();
            let mut state = state.lock().unwrap();

            let mut from_acct = (*state)
                .accounts
                .get(&from)
                .cloned()
                .ok_or(validation_err("missing 'from' account"))?;
            let mut to_acct = (*state)
                .accounts
                .get(&from)
                .cloned()
                .ok_or(validation_err("missing 'to' account"))?;

            let currency = if from_acct.currency != to_acct.currency {
                return Err(validation_err("accounts have different currencies"));
            } else {
                from_acct.currency
            };

            from_acct.balance = from_acct
                .balance
                .checked_sub(amount)
                .ok_or(validation_err("insufficient funds in 'from' account"))?;
            to_acct.balance = to_acct.balance.checked_add(amount).ok_or(ErrorKind::Internal)?;

            let tx = TransactionsResponse {
                id,
                from: vec![TransactionAddressInfo {
                    account_id: Some(from),
                    owner_name: None,
                    blockchain_address: from_acct.account_address.clone().into_inner(),
                }],
                to: TransactionAddressInfo {
                    account_id: Some(to),
                    owner_name: None,
                    blockchain_address: to_acct.account_address.clone().into_inner(),
                },
                from_value: amount.to_string(),
                from_currency: currency.clone(),
                to_value: amount.to_string(),
                to_currency: currency.clone(),
                fee: Amount::zero().to_string(),
                status: "completed".to_owned(),
            };

            (*state).accounts.insert(from, from_acct);
            (*state).accounts.insert(to, to_acct);
            (*state).txs.insert(id, tx.clone());

            Ok(())
        };

        Box::new(result_fn().into_future())
    }
}
