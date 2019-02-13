use bigdecimal::BigDecimal;

use client::payments;
use models::order_v2::{OrderId, StoreId};
use models::*;

#[derive(Debug, Clone, Deserialize)]
pub struct CalculatePayoutPayload {
    pub store_id: StoreId,
    pub currency: TureCurrency,
    pub wallet_address: WalletAddress,
}

#[derive(Debug, Clone)]
pub struct CalculatedPayoutExcludingFees {
    pub order_ids: Vec<OrderId>,
    pub currency: TureCurrency,
    pub gross_amount: Amount,
}

#[derive(Debug, Clone, Serialize)]
pub struct CalculatedPayoutOutput {
    pub order_ids: Vec<OrderId>,
    pub currency: TureCurrency,
    pub gross_amount: BigDecimal,
    pub blockchain_fee_options: Vec<BlockchainFeeOption>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockchainFeeOption {
    pub value: BigDecimal,
    pub estimated_time_seconds: u64,
}

impl BlockchainFeeOption {
    pub fn from_payments_fee(currency: TureCurrency, fee: payments::Fee) -> Self {
        let payments::Fee { value, estimated_time } = fee;

        Self {
            value: value.to_super_unit(currency.into()),
            estimated_time_seconds: estimated_time,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetPayoutsPayload {
    pub order_ids: Vec<OrderId>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PayOutToSellerPayload {
    pub order_ids: Vec<OrderId>,
    pub payment_details: PaymentDetails,
}

#[derive(Debug, Clone, Deserialize)]
pub enum PaymentDetails {
    Crypto(CryptoPaymentDetails),
}

#[derive(Debug, Clone, Deserialize)]
pub struct CryptoPaymentDetails {
    pub wallet_currency: TureCurrency,
    pub wallet_address: WalletAddress,
    pub blockchain_fee: BigDecimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct PayoutOutput {
    pub id: PayoutId,
    pub gross_amount: BigDecimal,
    pub net_amount: BigDecimal,
    pub target: PayoutTarget,
    pub user_id: UserId,
    pub status: PayoutStatus,
    pub order_ids: Vec<OrderId>,
}

impl From<Payout> for PayoutOutput {
    fn from(payout: Payout) -> Self {
        let currency = payout.currency();

        let Payout {
            id,
            gross_amount,
            net_amount,
            target,
            user_id,
            status,
            order_ids,
        } = payout;

        Self {
            id,
            gross_amount: gross_amount.to_super_unit(currency),
            net_amount: net_amount.to_super_unit(currency),
            target,
            user_id,
            status,
            order_ids,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PayoutOutputWithOrderId {
    pub order_id: OrderId,
    pub payout: PayoutOutput,
}

#[derive(Debug, Clone, Serialize)]
pub struct PayoutsByOrderIdsOutput {
    pub payouts: Vec<PayoutOutputWithOrderId>,
    pub order_ids_without_payout: Vec<OrderId>,
}

impl From<PayoutsByOrderIds> for PayoutsByOrderIdsOutput {
    fn from(payouts_by_order_ids: PayoutsByOrderIds) -> PayoutsByOrderIdsOutput {
        let PayoutsByOrderIds {
            payouts,
            order_ids_without_payout,
        } = payouts_by_order_ids;

        let payouts = payouts
            .into_iter()
            .map(|(order_id, payout)| PayoutOutputWithOrderId {
                order_id,
                payout: PayoutOutput::from(payout),
            })
            .collect();

        PayoutsByOrderIdsOutput {
            payouts,
            order_ids_without_payout,
        }
    }
}
