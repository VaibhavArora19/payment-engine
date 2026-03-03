use serde::Deserialize;

use crate::models::amount::Amount;

/// Raw row deserialized directly from CSV.
/// Amount is Option because dispute/resolve/chargeback have no amount column.
#[derive(Debug, Deserialize)]
pub struct RawTransaction {
    #[serde(rename = "type")]
    pub tx_type: TxType,
    pub client: u16,
    pub tx: u32,
    pub amount: Option<Amount>,
}

/// Processed transaction stored in the engine's hashmap.
/// Only deposits are stored (needed for dispute lookups).
/// Fields ordered largest-to-smallest alignment to eliminate padding: 16 bytes total.
/// 1 bytes padding to reach next 8-byte boundary
#[derive(Debug, Clone)]
pub struct Transaction {
    pub amount: Amount,
    pub tx_id: u32,
    pub client_id: u16,
    pub state: TxState,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TxType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TxState {
    Active,   // normal, nothing happening
    Disputed, // under dispute, amount is held
}

const _: () = assert!(std::mem::size_of::<Transaction>() == 16);
// RawTransaction: Option<Amount> is 16 bytes (no niche in i64), dominates layout.
// Reordering fields gives no benefit — size is 24 bytes either way.
const _: () = assert!(std::mem::size_of::<RawTransaction>() == 24);
