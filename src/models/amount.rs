use std::fmt;
use crate::error::AmountError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Amount(i64); //ensures type safety at compile time

const PRECISION: i64 = 10_000; //10 ** 4

impl Amount {
    pub const ZERO: Amount = Amount(0);

    //Parses the amount from a string -> "1.5000"
    pub fn from_str(s: &str) -> Result<Self, AmountError> {
        let trimmed = s.trim();

        
        let (whole, frac) = match trimmed.split_once(".") {
            Some((w, f)) => (w, f),
            None => (trimmed, "")
        };

        if frac.len() > 4 {
            return Err(AmountError::TooManyDecimalPlaces(s.to_string()));
        }

        let whole_part: i64 = whole.parse().map_err(|_| AmountError::InvalidFormat(s.to_string()))?;

        let frac_padded = format!("{:0<4}", frac); //"25" -> "2500"
        let frac_part: i64 = frac_padded.parse().map_err(|_| AmountError::InvalidFormat(s.to_string()))?;


        let raw = whole_part.checked_mul(PRECISION).and_then(|w| w.checked_add(frac_part)).ok_or(AmountError::Overflow)?;

        if raw < 0 {
            return Err(AmountError::Negative);
        }

        Ok(Amount(raw))
    }

    // Add two amounts. Returns error on overflow.
    pub fn checked_add(self, other: Amount) -> Result<Amount, AmountError> {
        self.0.checked_add(other.0).map(Amount).ok_or(AmountError::Overflow)
    }

    pub fn checked_sub(self, other: Amount) -> Result<Amount, AmountError> {
        self.0.checked_sub(other.0).ok_or(AmountError::Overflow).and_then(|result| {
            if result < 0 {
                Err(AmountError::InsufficientFunds)
            } else {
                Ok(Amount(result))
            }
        })

    }

    pub fn is_gte(self, other: Amount) -> bool {
        self.0 >= other.0
    }
    
}

/// Display as a 4 decimal place string: 15000 → "1.5000"
impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let whole = self.0 / PRECISION;
        let frac = self.0 % PRECISION;
        write!(f, "{}.{:04}", whole, frac)
    }
}

/// Serde serialization — output as "1.5000" string in CSV
impl serde::Serialize for Amount {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// Serde deserialization — parse "1.5000" from CSV input
impl<'de> serde::Deserialize<'de> for Amount {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Amount::from_str(&s).map_err(serde::de::Error::custom)
    }
}