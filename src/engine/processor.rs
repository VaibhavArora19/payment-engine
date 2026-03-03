use crate::error::AppError;
use crate::models::account::Account;
use crate::models::transaction::{RawTransaction, Transaction, TxState, TxType};
use std::collections::HashMap;

pub struct Engine {
    /// Bounded by u16 — max 65,535 entries, always fits in RAM.
    accounts: HashMap<u16, Account>,
    /// Only holds deposits that are Active or Disputed.
    /// Withdrawals are never stored as they cannot be disputed.
    /// Entries are removed on Resolve/Chargeback so only the live
    /// disputable surface stays in memory at any point.
    deposits: HashMap<u32, Transaction>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            deposits: HashMap::new(),
        }
    }

    /// Process a single raw transaction streamed from CSV.
    /// Soft errors are logged and skipped. Fatal errors bubble up.
    pub fn process(&mut self, raw: RawTransaction) -> Result<(), AppError> {
        match raw.tx_type {
            TxType::Deposit => self.deposit(raw),
            TxType::Withdrawal => self.withdrawal(raw),
            TxType::Dispute => self.dispute(raw),
            TxType::Resolve => self.resolve(raw),
            TxType::Chargeback => self.chargeback(raw),
        }
    }

    /// Returns iterator over all accounts for output.
    pub fn accounts(self) -> impl Iterator<Item = Account> {
        self.accounts.into_values()
    }

    fn deposit(&mut self, raw: RawTransaction) -> Result<(), AppError> {
        let amount = match raw.amount {
            Some(a) => a,
            None => {
                log::warn!("deposit tx {} has no amount, skipping", raw.tx);
                return Ok(());
            }
        };

        let account = self
            .accounts
            .entry(raw.client)
            .or_insert_with(|| Account::new(raw.client));

        if let Err(e) = account.deposit(amount) {
            log::warn!("deposit tx {} failed: {}", raw.tx, e);
            return Ok(());
        }

        //Only deposits are stored since they are the only type that can be disputed
        self.deposits.insert(
            raw.tx,
            Transaction {
                tx_id: raw.tx,
                client_id: raw.client,
                amount,
                state: TxState::Active,
            },
        );

        Ok(())
    }

    fn withdrawal(&mut self, raw: RawTransaction) -> Result<(), AppError> {
        let amount = match raw.amount {
            Some(a) => a,
            None => {
                log::warn!("withdrawal tx {} has no amount, skipping", raw.tx);
                return Ok(());
            }
        };

        //Do not add withdrawal for unknown client
        let Some(account) = self.accounts.get_mut(&raw.client) else {
            log::warn!("withdrawal tx {} for unknown client, skipping", raw.tx);
            return Ok(());
        };

        if let Err(e) = account.withdraw(amount) {
            log::warn!("withdrawal tx {} failed: {}", raw.tx, e);
            return Ok(());
        }

        Ok(())
    }

    fn dispute(&mut self, raw: RawTransaction) -> Result<(), AppError> {
        let tx = match self.deposits.get_mut(&raw.tx) {
            Some(t) => t,
            None => {
                log::warn!("dispute tx {} not found, skipping", raw.tx);
                return Ok(());
            }
        };

        if tx.client_id != raw.client {
            log::warn!("dispute tx {} client mismatch, skipping", raw.tx);
            return Ok(());
        }

        if tx.state != TxState::Active {
            log::warn!("dispute tx {} is not active, skipping", raw.tx);
            return Ok(());
        }

        let amount = tx.amount;

        //Skip if the client is missing
        let Some(account) = self.accounts.get_mut(&raw.client) else {
            log::warn!("dispute tx {} for unknown client, skipping", raw.tx);
            return Ok(());
        };

        // Mark disputed only after we know the account exists
        if let Err(e) = account.dispute(amount) {
            log::warn!("dispute tx {} account mutation failed: {}", raw.tx, e);
            return Ok(());
        }

        self.deposits.get_mut(&raw.tx).unwrap().state = TxState::Disputed;

        Ok(())
    }

    fn resolve(&mut self, raw: RawTransaction) -> Result<(), AppError> {
        let tx = match self.deposits.get_mut(&raw.tx) {
            Some(t) => t,
            None => {
                log::warn!("resolve tx {} not found, skipping", raw.tx);
                return Ok(());
            }
        };

        if tx.client_id != raw.client {
            log::warn!("resolve tx {} client mismatch, skipping", raw.tx);
            return Ok(());
        }

        if tx.state != TxState::Disputed {
            log::warn!("resolve tx {} is not disputed, skipping", raw.tx);
            return Ok(());
        }

        let amount = tx.amount;

        //Skip if the client is missing
        let Some(account) = self.accounts.get_mut(&raw.client) else {
            log::warn!("resolve tx {} for unknown client, skipping", raw.tx);
            return Ok(());
        };

        if let Err(e) = account.resolve(amount) {
            log::warn!("resolve tx {} account mutation failed: {}", raw.tx, e);
            return Ok(());
        }

        //Prune from memory as this tx can never be acted on again.
        self.deposits.remove(&raw.tx);

        Ok(())
    }

    fn chargeback(&mut self, raw: RawTransaction) -> Result<(), AppError> {
        let tx = match self.deposits.get_mut(&raw.tx) {
            Some(t) => t,
            None => {
                log::warn!("chargeback tx {} not found, skipping", raw.tx);
                return Ok(());
            }
        };

        if tx.client_id != raw.client {
            log::warn!("chargeback tx {} client mismatch, skipping", raw.tx);
            return Ok(());
        }

        if tx.state != TxState::Disputed {
            log::warn!("chargeback tx {} is not disputed, skipping", raw.tx);
            return Ok(());
        }

        let amount = tx.amount;

        //Skip if the client is missing
        let Some(account) = self.accounts.get_mut(&raw.client) else {
            log::warn!("chargeback tx {} for unknown client, skipping", raw.tx);
            return Ok(());
        };

        if let Err(e) = account.chargeback(amount) {
            log::warn!("chargeback tx {} account mutation failed: {}", raw.tx, e);
            return Ok(());
        }

        //prune from memory since this tx can never be acted on again
        self.deposits.remove(&raw.tx);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        amount::Amount,
        transaction::{RawTransaction, TxState, TxType},
    };

    fn amt(s: &str) -> Amount {
        s.parse().unwrap()
    }

    fn raw_deposit(client: u16, tx: u32, amount: &str) -> RawTransaction {
        RawTransaction {
            tx_type: TxType::Deposit,
            client,
            tx,
            amount: Some(amt(amount)),
        }
    }

    fn raw_withdrawal(client: u16, tx: u32, amount: &str) -> RawTransaction {
        RawTransaction {
            tx_type: TxType::Withdrawal,
            client,
            tx,
            amount: Some(amt(amount)),
        }
    }

    fn raw_dispute(client: u16, tx: u32) -> RawTransaction {
        RawTransaction {
            tx_type: TxType::Dispute,
            client,
            tx,
            amount: None,
        }
    }

    fn raw_resolve(client: u16, tx: u32) -> RawTransaction {
        RawTransaction {
            tx_type: TxType::Resolve,
            client,
            tx,
            amount: None,
        }
    }

    fn raw_chargeback(client: u16, tx: u32) -> RawTransaction {
        RawTransaction {
            tx_type: TxType::Chargeback,
            client,
            tx,
            amount: None,
        }
    }

    // --------------- Deposit ---------------

    #[test]
    fn deposit_creates_account_and_credits_funds() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();

        let account = engine.accounts.get(&1).unwrap();
        assert_eq!(account.available, amt("100.0"));
        assert_eq!(account.held, Amount::ZERO);
    }

    #[test]
    fn deposit_stores_transaction_as_active() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();

        assert_eq!(engine.deposits[&1].state, TxState::Active);
    }


    #[test]
    fn deposit_with_no_amount_skipped() {
        let mut engine = Engine::new();
        engine
            .process(RawTransaction {
                tx_type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: None,
            })
            .unwrap();

        assert!(!engine.accounts.contains_key(&1));
    }

    // --------------- Withdrawal ---------------

    #[test]
    fn withdrawal_debits_available() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_withdrawal(1, 2, "40.0")).unwrap();

        assert_eq!(engine.accounts[&1].available, amt("60.0"));
    }

    #[test]
    fn withdrawal_insufficient_funds_skipped() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "10.0")).unwrap();
        engine.process(raw_withdrawal(1, 2, "50.0")).unwrap();

        assert_eq!(engine.accounts[&1].available, amt("10.0"));
    }

    #[test]
    fn withdrawal_not_stored_in_deposits() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_withdrawal(1, 2, "40.0")).unwrap();

        assert!(!engine.deposits.contains_key(&2));
    }

    #[test]
    fn withdrawal_for_unknown_client_does_not_create_account() {
        let mut engine = Engine::new();
        engine.process(raw_withdrawal(99, 1, "50.0")).unwrap();

        assert!(!engine.accounts.contains_key(&99));
    }

    // --------------- Dispute ---------------

    #[test]
    fn dispute_moves_funds_to_held() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_dispute(1, 1)).unwrap();

        let account = &engine.accounts[&1];
        assert_eq!(account.available, Amount::ZERO);
        assert_eq!(account.held, amt("100.0"));
        assert_eq!(engine.deposits[&1].state, TxState::Disputed);
    }

    #[test]
    fn dispute_unknown_tx_skipped() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_dispute(1, 99)).unwrap();

        assert_eq!(engine.accounts[&1].available, amt("100.0"));
        assert_eq!(engine.accounts[&1].held, Amount::ZERO);
    }

    #[test]
    fn dispute_client_mismatch_skipped() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_dispute(2, 1)).unwrap(); // client 2 disputes tx owned by client 1

        assert_eq!(engine.accounts[&1].available, amt("100.0"));
        assert_eq!(engine.deposits[&1].state, TxState::Active);
    }

    #[test]
    fn dispute_on_already_disputed_tx_skipped() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_dispute(1, 1)).unwrap();
        engine.process(raw_dispute(1, 1)).unwrap(); // second dispute on same tx

        // held should not be doubled
        assert_eq!(engine.accounts[&1].held, amt("100.0"));
    }

    // --------------- Resolve ---------------

    #[test]
    fn resolve_returns_funds_to_available() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_dispute(1, 1)).unwrap();
        engine.process(raw_resolve(1, 1)).unwrap();

        let account = &engine.accounts[&1];
        assert_eq!(account.available, amt("100.0"));
        assert_eq!(account.held, Amount::ZERO);
    }

    #[test]
    fn resolve_prunes_transaction_from_memory() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_dispute(1, 1)).unwrap();
        engine.process(raw_resolve(1, 1)).unwrap();

        assert!(!engine.deposits.contains_key(&1));
    }

    #[test]
    fn resolve_on_non_disputed_tx_skipped() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_resolve(1, 1)).unwrap(); // tx is Active, not Disputed

        assert_eq!(engine.accounts[&1].available, amt("100.0"));
        assert!(engine.deposits.contains_key(&1)); // not pruned
    }

    // --------------- Chargeback ---------------

    #[test]
    fn chargeback_removes_held_and_locks_account() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_dispute(1, 1)).unwrap();
        engine.process(raw_chargeback(1, 1)).unwrap();

        let account = &engine.accounts[&1];
        assert_eq!(account.held, Amount::ZERO);
        assert_eq!(account.available, Amount::ZERO);
        assert!(account.locked);
    }

    #[test]
    fn chargeback_prunes_transaction_from_memory() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_dispute(1, 1)).unwrap();
        engine.process(raw_chargeback(1, 1)).unwrap();

        assert!(!engine.deposits.contains_key(&1));
    }

    #[test]
    fn chargeback_on_non_disputed_tx_skipped() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_chargeback(1, 1)).unwrap(); // tx is Active, not Disputed

        assert!(!engine.accounts[&1].locked);
        assert!(engine.deposits.contains_key(&1)); // not pruned
    }

    #[test]
    fn deposit_after_chargeback_skipped() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_dispute(1, 1)).unwrap();
        engine.process(raw_chargeback(1, 1)).unwrap();

        engine.process(raw_deposit(1, 2, "50.0")).unwrap(); // account is locked, this should throw error

        let account = &engine.accounts[&1];
        assert_eq!(account.available, Amount::ZERO);
        assert!(account.locked);
    }

    // --------------- Withdrawal error paths ---------------

    #[test]
    fn withdrawal_with_no_amount_skipped() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine
            .process(RawTransaction {
                tx_type: TxType::Withdrawal,
                client: 1,
                tx: 2,
                amount: None,
            })
            .unwrap();

        assert_eq!(engine.accounts[&1].available, amt("100.0")); // unchanged
    }

    #[test]
    fn withdrawal_on_locked_account_skipped() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_dispute(1, 1)).unwrap();
        engine.process(raw_chargeback(1, 1)).unwrap(); // locks account

        engine.process(raw_withdrawal(1, 2, "10.0")).unwrap();

        assert_eq!(engine.accounts[&1].available, Amount::ZERO); // unchanged
    }

    // --------------- Dispute error paths ---------------

    #[test]
    fn dispute_on_locked_account_rolls_back_tx_state() {
        // two deposits for same client ad chargeback one to lock account
        // then try to dispute the other active tx
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap(); // tx 1 — will stay Active
        engine.process(raw_deposit(1, 2, "50.0")).unwrap(); // tx 2 — will be chargedback
        engine.process(raw_dispute(1, 2)).unwrap();
        engine.process(raw_chargeback(1, 2)).unwrap(); // account now locked, tx 2 pruned

        // tx 1 is still Active in the map dispute will fail with AccountLocked
        engine.process(raw_dispute(1, 1)).unwrap();

        // tx 1 state will be active
        assert_eq!(engine.deposits[&1].state, TxState::Active);
        assert_eq!(engine.accounts[&1].held, Amount::ZERO);
    }

    // --------------- Resolve error paths ---------------
    #[test]
    fn resolve_unknown_tx_skipped() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();

        engine.process(raw_resolve(1, 99)).unwrap(); // tx 99 does not exist

        assert_eq!(engine.accounts[&1].available, amt("100.0"));
    }

    #[test]
    fn resolve_client_mismatch_skipped() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_dispute(1, 1)).unwrap();

        engine.process(raw_resolve(2, 1)).unwrap(); // client 2 resolves tx owned by client 1

        assert_eq!(engine.deposits[&1].state, TxState::Disputed);
        assert_eq!(engine.accounts[&1].held, amt("100.0"));
    }

    // --------------- Chargeback error paths ---------------

    #[test]
    fn chargeback_unknown_tx_skipped() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();

        engine.process(raw_chargeback(1, 99)).unwrap(); // tx 99 doesn't exist

        assert!(!engine.accounts[&1].locked);
    }

    #[test]
    fn chargeback_client_mismatch_skipped() {
        let mut engine = Engine::new();
        engine.process(raw_deposit(1, 1, "100.0")).unwrap();
        engine.process(raw_dispute(1, 1)).unwrap();

        engine.process(raw_chargeback(2, 1)).unwrap(); // client 2 charges back tx owned by client 1

        assert_eq!(engine.deposits[&1].state, TxState::Disputed); // still disputed
        assert!(!engine.accounts[&1].locked);
    }
}
