use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),

    #[error("amount error: {0}")]
    Amount(#[from] AmountError),
}

#[derive(Debug, Error)]
pub enum AmountError {
    #[error("amount has more than 4 decimal places: {0}")]
    TooManyDecimalPlaces(String),

    #[error("invalid amount format: {0}")]
    InvalidFormat(String),

    #[error("amount cannot be negative")]
    Negative,

    #[error("arithmetic overflow")]
    Overflow,

    #[error("insufficient funds")]
    InsufficientFunds,

    #[error("account is locked")]
    AccountLocked,
}
