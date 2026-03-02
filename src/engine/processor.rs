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
    transactions: HashMap<u32, Transaction>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
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
            None => return Ok(()), // malformed, skip
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
        //if the transaction from same tx already exists then drop the current one
        self.transactions.entry(raw.tx).or_insert(Transaction {
            tx_id: raw.tx,
            client_id: raw.client,
            amount,
            state: TxState::Active,
        });

        Ok(())
    }

    fn withdrawal(&mut self, raw: RawTransaction) -> Result<(), AppError> {
        let amount = match raw.amount {
            Some(a) => a,
            None => return Ok(()),
        };

        let account = self
            .accounts
            .entry(raw.client)
            .or_insert_with(|| Account::new(raw.client));

        if let Err(e) = account.withdraw(amount) {
            log::warn!("withdrawal tx {} failed: {}", raw.tx, e);
        }

        //Withdrawals are not stored as they cannot be disputed.
        Ok(())
    }

    fn dispute(&mut self, raw: RawTransaction) -> Result<(), AppError> {
        let tx = match self.transactions.get_mut(&raw.tx) {
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
        tx.state = TxState::Disputed;

        let account = self
            .accounts
            .entry(raw.client)
            .or_insert_with(|| Account::new(raw.client));

        if let Err(e) = account.dispute(amount) {
            self.transactions.get_mut(&raw.tx).unwrap().state = TxState::Active;
            log::warn!("dispute tx {} account mutation failed: {}", raw.tx, e);
        }

        Ok(())
    }

    fn resolve(&mut self, raw: RawTransaction) -> Result<(), AppError> {
        let tx = match self.transactions.get_mut(&raw.tx) {
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

        let account = self
            .accounts
            .entry(raw.client)
            .or_insert_with(|| Account::new(raw.client));

        if let Err(e) = account.resolve(amount) {
            log::warn!("resolve tx {} account mutation failed: {}", raw.tx, e);
            return Ok(());
        }

        //Prune from memory as this tx can never be acted on again.
        self.transactions.remove(&raw.tx);

        Ok(())
    }

    fn chargeback(&mut self, raw: RawTransaction) -> Result<(), AppError> {
        let tx = match self.transactions.get_mut(&raw.tx) {
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

        let account = self
            .accounts
            .entry(raw.client)
            .or_insert_with(|| Account::new(raw.client));

        if let Err(e) = account.chargeback(amount) {
            log::warn!("chargeback tx {} account mutation failed: {}", raw.tx, e);
            return Ok(());
        }

        //prune from memory since this tx can never be acted on again
        self.transactions.remove(&raw.tx);

        Ok(())
    }
}
