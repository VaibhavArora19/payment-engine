use crate::{error::AmountError, models::amount::Amount};

pub struct Account {
    pub client: u16,
    pub available: Amount,
    pub held: Amount,
    pub locked: bool,
}

impl Account {
    pub fn new(client: u16) -> Self {
        Self {
            client,
            available: Amount::ZERO,
            held: Amount::ZERO,
            locked: false,
        }
    }

    /// total is always computed, never stored — avoids invariant drift
    pub fn total(&self) -> Result<Amount, AmountError> {
        self.available.checked_add(self.held)
    }

    /// Credit available funds. Called on deposit.
    pub fn deposit(&mut self, amount: Amount) -> Result<(), AmountError> {
        if self.locked {
            return Err(AmountError::AccountLocked);
        }

        self.available = self.available.checked_add(amount)?;

        Ok(())
    }

    /// Debit available funds. Called on withdrawal.
    pub fn withdraw(&mut self, amount: Amount) -> Result<(), AmountError> {
        if self.locked {
            return Err(AmountError::AccountLocked);
        }
        // guard: never let available go negative
        if !self.available.is_gte(amount) {
            return Err(AmountError::InsufficientFunds);
        }
        self.available = self.available.checked_sub(amount)?;
        Ok(())
    }

    /// Move funds from available -> held. Called on dispute
    pub fn dispute(&mut self, amount: Amount) -> Result<(), AmountError> {
        if self.locked {
            return Err(AmountError::AccountLocked);
        }
        if !self.available.is_gte(amount) {
            return Err(AmountError::InsufficientFunds);
        }
        self.available = self.available.checked_sub(amount)?;
        self.held = self.held.checked_add(amount)?;
        Ok(())
    }

    /// Move funds from held → available. Called on resolve.
    pub fn resolve(&mut self, amount: Amount) -> Result<(), AmountError> {
        if self.locked {
            return Err(AmountError::AccountLocked);
        }
        if !self.held.is_gte(amount) {
            return Err(AmountError::InsufficientFunds);
        }
        self.held = self.held.checked_sub(amount)?;
        self.available = self.available.checked_add(amount)?;
        Ok(())
    }

    //TODO: Chargeback needs improvements and changes read description carefully and implement
    /// Remove held funds entirely + lock account. Called on chargeback.
    pub fn chargeback(&mut self, amount: Amount) -> Result<(), AmountError> {
        if !self.held.is_gte(amount) {
            return Err(AmountError::InsufficientFunds);
        }
        self.held = self.held.checked_sub(amount)?;
        // total drops — money is gone
        self.locked = true;
        Ok(())
    }
}
