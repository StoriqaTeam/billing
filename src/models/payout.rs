use chrono::NaiveDateTime;

use models::order_v2::OrderId;
use models::*;
use schema::payouts;

#[derive(Clone, Debug, Serialize)]
pub struct Payout {
    pub order_id: OrderId,
    pub amount: Amount,
    pub target: PayoutTarget,
    pub completed_at: NaiveDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PayoutTarget {
    CryptoWallet(CryptoWalletPayoutTarget),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CryptoWalletPayoutTarget {
    pub user_wallet_id: UserWalletId,
    pub blockchain_fee: Amount,
}

#[derive(Clone, Debug, Serialize, Deserialize, Queryable, Insertable)]
#[table_name = "payouts"]
pub struct RawPayout {
    pub order_id: OrderId,
    pub amount: Amount,
    pub completed_at: NaiveDateTime,
    pub payout_target_type: RawPayoutTargetType,
    pub user_wallet_id: Option<UserWalletId>,
    pub blockchain_fee: Option<Amount>,
}

impl From<Payout> for RawPayout {
    fn from(payout: Payout) -> Self {
        let Payout {
            order_id,
            amount,
            target,
            completed_at,
        } = payout;

        match target {
            PayoutTarget::CryptoWallet(target) => {
                let CryptoWalletPayoutTarget {
                    user_wallet_id,
                    blockchain_fee,
                } = target;

                Self {
                    order_id,
                    amount,
                    completed_at,
                    payout_target_type: RawPayoutTargetType::CryptoWallet,
                    user_wallet_id: Some(user_wallet_id),
                    blockchain_fee: Some(blockchain_fee),
                }
            }
        }
    }
}

#[derive(Clone, Debug, Fail, Serialize)]
#[fail(display = "invalid DB representation of the Payout domain object")]
pub struct RawPayoutIntoDomainError;

impl RawPayout {
    pub fn try_into_domain(self) -> Result<Payout, RawPayoutIntoDomainError> {
        let RawPayout {
            order_id,
            amount,
            completed_at,
            payout_target_type,
            user_wallet_id,
            blockchain_fee,
        } = self;

        let target = match (payout_target_type, user_wallet_id, blockchain_fee) {
            (RawPayoutTargetType::CryptoWallet, Some(user_wallet_id), Some(blockchain_fee)) => {
                Ok(PayoutTarget::CryptoWallet(CryptoWalletPayoutTarget {
                    user_wallet_id,
                    blockchain_fee,
                }))
            }
            _ => Err(RawPayoutIntoDomainError),
        }?;

        Ok(Payout {
            order_id,
            amount,
            target,
            completed_at,
        })
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Hash, DieselTypes)]
#[serde(rename_all = "snake_case")]
pub enum RawPayoutTargetType {
    CryptoWallet,
}

#[derive(Clone, Debug)]
pub struct PayoutAccess {
    pub order_id: OrderId,
}

impl From<&Payout> for PayoutAccess {
    fn from(payout: &Payout) -> PayoutAccess {
        PayoutAccess {
            order_id: payout.order_id.clone(),
        }
    }
}
