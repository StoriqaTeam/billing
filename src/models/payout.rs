use std::collections::HashMap;
use std::fmt;

use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use uuid::Uuid;

use models::order_v2::OrderId;
use models::*;
use schema::order_payouts;
use schema::payouts;

#[derive(Debug, Serialize, Deserialize, FromStr, AsExpression, Clone, Copy, PartialEq, Eq, Hash, DieselTypes)]
pub struct PayoutId(Uuid);

impl PayoutId {
    pub fn new(id: Uuid) -> Self {
        PayoutId(id)
    }

    pub fn inner(&self) -> &Uuid {
        &self.0
    }

    pub fn into_inner(self) -> Uuid {
        self.0
    }

    pub fn generate() -> Self {
        PayoutId(Uuid::new_v4())
    }
}

impl fmt::Display for PayoutId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0.hyphenated()))
    }
}

#[derive(Debug, Serialize, Deserialize, FromStr, Display, AsExpression, Clone, Copy, PartialEq, Eq, Hash, DieselTypes)]
pub struct OrderPayoutId(i64);

impl OrderPayoutId {
    pub fn inner(&self) -> i64 {
        self.0
    }
}

#[derive(Clone, Debug)]
pub struct Payout {
    pub id: PayoutId,
    pub gross_amount: Amount,
    pub net_amount: Amount,
    pub target: PayoutTarget,
    pub user_id: UserId,
    pub status: PayoutStatus,
    pub order_ids: Vec<OrderId>,
}

impl Payout {
    pub fn currency(&self) -> Currency {
        match self.target {
            PayoutTarget::CryptoWallet(ref target) => Currency::from(target.currency),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PayoutStatus {
    Processing {
        initiated_at: NaiveDateTime,
    },
    Completed {
        initiated_at: NaiveDateTime,
        completed_at: NaiveDateTime,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PayoutTarget {
    CryptoWallet(CryptoWalletPayoutTarget),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CryptoWalletPayoutTarget {
    pub currency: TureCurrency,
    pub wallet_address: WalletAddress,
    pub blockchain_fee: Amount,
}

#[derive(Clone, Debug, Serialize, Deserialize, Queryable, Insertable)]
#[table_name = "payouts"]
pub struct RawPayout {
    pub id: PayoutId,
    pub currency: Currency,
    pub gross_amount: Amount,
    pub net_amount: Amount,
    pub user_id: UserId,
    pub initiated_at: NaiveDateTime,
    pub completed_at: Option<NaiveDateTime>,
    pub payout_target_type: RawPayoutTargetType,
    pub wallet_address: Option<WalletAddress>,
    pub blockchain_fee: Option<Amount>,
}

impl PartialEq for RawPayout {
    fn eq(&self, other: &RawPayout) -> bool {
        self.id == other.id
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Queryable)]
pub struct RawOrderPayout {
    pub id: OrderPayoutId,
    pub order_id: OrderId,
    pub payout_id: PayoutId,
}

#[derive(Clone, Debug, Serialize, Deserialize, Insertable)]
#[table_name = "order_payouts"]
pub struct RawNewOrderPayout {
    pub order_id: OrderId,
    pub payout_id: PayoutId,
}

#[derive(Clone, Debug)]
pub struct RawPayoutRecords {
    pub raw_payout: RawPayout,
    pub raw_order_payouts: Vec<RawOrderPayout>,
}

#[derive(Clone, Debug, Fail, Serialize)]
#[fail(display = "invalid DB representation of the Payout domain object")]
pub struct RawPayoutRecordsMappingError;

impl RawPayoutRecords {
    pub fn try_into_domain(self) -> Result<Payout, RawPayoutRecordsMappingError> {
        let RawPayoutRecords {
            raw_payout:
                RawPayout {
                    id: payout_id,
                    currency,
                    gross_amount,
                    net_amount,
                    user_id,
                    initiated_at,
                    completed_at,
                    payout_target_type,
                    wallet_address,
                    blockchain_fee,
                },
            raw_order_payouts,
        } = self;

        let target = match (currency.classify(), payout_target_type, wallet_address, blockchain_fee) {
            (CurrencyChoice::Crypto(currency), RawPayoutTargetType::CryptoWallet, Some(wallet_address), Some(blockchain_fee)) => {
                Ok(PayoutTarget::CryptoWallet(CryptoWalletPayoutTarget {
                    currency,
                    wallet_address,
                    blockchain_fee,
                }))
            }
            _ => Err(RawPayoutRecordsMappingError),
        }?;

        let order_payouts_payout_id = raw_order_payouts.iter().next().map(|record| record.payout_id);
        let order_ids = match order_payouts_payout_id {
            Some(order_payouts_payout_id) => {
                let all_same_payout_id = raw_order_payouts.iter().all(|record| record.payout_id == order_payouts_payout_id);
                if all_same_payout_id {
                    Ok(raw_order_payouts.into_iter().map(|record| record.order_id).collect())
                } else {
                    Err(RawPayoutRecordsMappingError)
                }
            }
            None => Ok(vec![]),
        }?;

        let status = match completed_at {
            None => PayoutStatus::Processing { initiated_at },
            Some(completed_at) => PayoutStatus::Completed {
                initiated_at,
                completed_at,
            },
        };

        Ok(Payout {
            id: payout_id,
            gross_amount,
            net_amount,
            target,
            user_id,
            status,
            order_ids,
        })
    }
}

#[derive(Clone, Debug)]
pub struct RawNewPayoutRecords {
    pub raw_new_payout: RawPayout,
    pub raw_new_order_payouts: Vec<RawNewOrderPayout>,
}

impl From<Payout> for RawNewPayoutRecords {
    fn from(payout: Payout) -> Self {
        let Payout {
            id,
            gross_amount,
            net_amount,
            target,
            user_id,
            status,
            order_ids,
        } = payout;

        let raw_new_payout = match target {
            PayoutTarget::CryptoWallet(target) => {
                let CryptoWalletPayoutTarget {
                    currency,
                    wallet_address,
                    blockchain_fee,
                } = target;

                let (initiated_at, completed_at) = match status {
                    PayoutStatus::Processing { initiated_at } => (initiated_at, None),
                    PayoutStatus::Completed {
                        initiated_at,
                        completed_at,
                    } => (initiated_at, Some(completed_at)),
                };

                RawPayout {
                    id,
                    currency: currency.into(),
                    gross_amount,
                    net_amount,
                    user_id,
                    initiated_at,
                    completed_at,
                    payout_target_type: RawPayoutTargetType::CryptoWallet,
                    wallet_address: Some(wallet_address),
                    blockchain_fee: Some(blockchain_fee),
                }
            }
        };

        let raw_new_order_payouts = order_ids
            .into_iter()
            .map(|order_id| RawNewOrderPayout { payout_id: id, order_id })
            .collect();

        RawNewPayoutRecords {
            raw_new_payout,
            raw_new_order_payouts,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Hash, DieselTypes)]
#[serde(rename_all = "snake_case")]
pub enum RawPayoutTargetType {
    CryptoWallet,
}

#[derive(Clone, Debug)]
pub struct PayoutAccess {
    pub user_id: UserId,
}

impl From<&Payout> for PayoutAccess {
    fn from(payout: &Payout) -> PayoutAccess {
        PayoutAccess {
            user_id: payout.user_id.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrdersForPayout {
    pub currency: TureCurrency,
    pub orders: Vec<OrderForPayout>,
}

#[derive(Debug, Clone)]
pub struct OrderForPayout {
    pub order_id: OrderId,
    pub total_amount: Amount,
}

#[derive(Debug, Clone)]
pub struct PayoutsByOrderIds {
    pub payouts: HashMap<OrderId, Payout>,
    pub order_ids_without_payout: Vec<OrderId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balances {
    pub currencies: HashMap<Currency, BigDecimal>,
}

impl Balances {
    pub fn new(currencies: HashMap<Currency, BigDecimal>) -> Self {
        Self { currencies }
    }
}
