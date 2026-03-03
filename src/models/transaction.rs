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
/// Only deposits and withdrawals are stored (needed for dispute lookups).
#[derive(Debug, Clone)]
pub struct Transaction {
    pub tx_id: u32,
    pub client_id: u16,
    pub amount: Amount,
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
