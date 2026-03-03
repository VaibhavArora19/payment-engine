use crate::{error::AmountError, models::amount::Amount};

/// Fields ordered largest-to-smallest alignment: 24 bytes total, no wasted padding.
/// 5 bytes padding to reach next 8-byte boundary
pub struct Account {
    pub available: Amount,
    pub held: Amount,
    pub client: u16,
    pub locked: bool,
}

const _: () = assert!(std::mem::size_of::<Account>() == 24);

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

    //Debit available funds. Called on withdrawal
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

    //Move funds from available -> held. Called on dispute
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

    //Move funds from held → available. Called on resolve
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

    //Remove held funds entirely + lock account. Called on chargeback.
    pub fn chargeback(&mut self, amount: Amount) -> Result<(), AmountError> {
        if !self.held.is_gte(amount) {
            return Err(AmountError::InsufficientFunds);
        }
        self.held = self.held.checked_sub(amount)?;
        //lock the account
        self.locked = true;
        Ok(())
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    fn amt(s: &str) -> Amount {
        s.parse().unwrap()
    }

    fn create_account() -> Account {
        Account::new(1)
    }

    #[test]
    fn deposit_increases_available() {
        let mut account = create_account();
        let amount = amt("1.0");

        account.deposit(amount).unwrap();

        assert_eq!(account.available, amount);
        assert_eq!(account.held, Amount::ZERO);
    }

    #[test]
    fn deposit_on_lock_account_fails() {
        let mut account = create_account();
        account.locked = true;

        let amount = amt("1.0");
        let err = account.deposit(amount).unwrap_err();

        assert!(matches!(err, AmountError::AccountLocked));
    }

    #[test]
    fn deposit_accumulates() {
        let mut account = create_account();
        let amount = amt("1.0");

        account.deposit(amount).unwrap();
        account.deposit(amount).unwrap();

        assert_eq!(account.available, amt("2.0"));
    }

    #[test]
    fn deposit_overflow_fails() {
        let mut account = create_account();

        account.deposit(Amount::MAX).unwrap();
        let err = account.deposit(amt("1.0")).unwrap_err();

        assert!(matches!(err, AmountError::Overflow));
    }

    #[test]
    fn withdraw_decreases_available() {
        let mut account = create_account();
        let amount_1 = amt("2.0");
        let amount_2 = amt("1.0");

        account.deposit(amount_1).unwrap();

        account.withdraw(amount_2).unwrap();

        assert_eq!(account.available, amount_2);
        assert_eq!(account.held, Amount::ZERO);
    }

    #[test]
    fn withdraw_on_locked_account_fails() {
        let mut account = create_account();
        account.deposit(amt("10.0")).unwrap();
        account.locked = true;

        let err = account.withdraw(amt("1.0")).unwrap_err();

        assert!(matches!(err, AmountError::AccountLocked));
    }

    #[test]
    fn withdraw_insufficient_funds_fails() {
        let mut account = create_account();
        account.deposit(amt("1.0")).unwrap();

        let err = account.withdraw(amt("2.0")).unwrap_err();

        assert!(matches!(err, AmountError::InsufficientFunds));
    }

    #[test]
    fn dispute_moves_funds_to_held() {
        let mut account = create_account();
        account.deposit(amt("5.0")).unwrap();

        account.dispute(amt("2.0")).unwrap();

        assert_eq!(account.available, amt("3.0"));
        assert_eq!(account.held, amt("2.0"));
    }

    #[test]
    fn dispute_on_locked_account_fails() {
        let mut account = create_account();
        account.deposit(amt("5.0")).unwrap();
        account.locked = true;

        let err = account.dispute(amt("1.0")).unwrap_err();

        assert!(matches!(err, AmountError::AccountLocked));
    }

    #[test]
    fn dispute_insufficient_funds_fails() {
        let mut account = create_account();
        account.deposit(amt("1.0")).unwrap();

        let err = account.dispute(amt("2.0")).unwrap_err();

        assert!(matches!(err, AmountError::InsufficientFunds));
    }

    #[test]
    fn resolve_moves_funds_back_to_available() {
        let mut account = create_account();
        account.deposit(amt("5.0")).unwrap();
        account.dispute(amt("2.0")).unwrap();

        account.resolve(amt("2.0")).unwrap();

        assert_eq!(account.available, amt("5.0"));
        assert_eq!(account.held, Amount::ZERO);
    }

    #[test]
    fn resolve_on_locked_account_fails() {
        let mut account = create_account();
        account.deposit(amt("5.0")).unwrap();
        account.dispute(amt("2.0")).unwrap();
        account.locked = true;

        let err = account.resolve(amt("2.0")).unwrap_err();

        assert!(matches!(err, AmountError::AccountLocked));
    }

    #[test]
    fn resolve_insufficient_held_fails() {
        let mut account = create_account();
        account.deposit(amt("5.0")).unwrap();
        account.dispute(amt("1.0")).unwrap();

        let err = account.resolve(amt("2.0")).unwrap_err();

        assert!(matches!(err, AmountError::InsufficientFunds));
    }

    #[test]
    fn chargeback_removes_held_and_locks_account() {
        let mut account = create_account();
        account.deposit(amt("5.0")).unwrap();
        account.dispute(amt("2.0")).unwrap();

        account.chargeback(amt("2.0")).unwrap();

        assert_eq!(account.held, Amount::ZERO);
        assert_eq!(account.available, amt("3.0"));
        assert!(account.locked);
    }

    #[test]
    fn chargeback_insufficient_held_fails() {
        let mut account = create_account();
        account.deposit(amt("5.0")).unwrap();
        account.dispute(amt("1.0")).unwrap();

        let err = account.chargeback(amt("2.0")).unwrap_err();

        assert!(matches!(err, AmountError::InsufficientFunds));
    }
}
