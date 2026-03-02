use thiserror::{Error};

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
    InsufficientFunds
}