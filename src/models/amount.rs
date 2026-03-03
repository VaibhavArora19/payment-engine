use crate::error::AmountError;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Amount(i64); //ensures type safety at compile time

const PRECISION: i64 = 10_000; //10 ** 4

impl Amount {
    pub const ZERO: Amount = Amount(0);

    // Add two amounts. Returns error on overflow.
    pub fn checked_add(self, other: Amount) -> Result<Amount, AmountError> {
        self.0
            .checked_add(other.0)
            .map(Amount)
            .ok_or(AmountError::Overflow)
    }

    pub fn checked_sub(self, other: Amount) -> Result<Amount, AmountError> {
        self.0
            .checked_sub(other.0)
            .ok_or(AmountError::Overflow)
            .and_then(|result| {
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

/// Parses "1.5000" into Amount. Implements the standard FromStr trait.
impl FromStr for Amount {
    type Err = AmountError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();

        let (whole, frac) = match trimmed.split_once('.') {
            Some((w, f)) => (w, f),
            None => (trimmed, ""),
        };

        if frac.len() > 4 {
            return Err(AmountError::TooManyDecimalPlaces(s.to_string()));
        }

        let whole_part: i64 = whole
            .parse()
            .map_err(|_| AmountError::InvalidFormat(s.to_string()))?;

        let frac_padded = format!("{frac:0<4}"); //"25" -> "2500"
        let frac_part: i64 = frac_padded
            .parse()
            .map_err(|_| AmountError::InvalidFormat(s.to_string()))?;

        let raw = whole_part
            .checked_mul(PRECISION)
            .and_then(|w| w.checked_add(frac_part))
            .ok_or(AmountError::Overflow)?;

        if raw < 0 {
            return Err(AmountError::Negative);
        }

        Ok(Amount(raw))
    }
}

/// Display as a 4 decimal place string: 15000 → "1.5000"
impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let whole = self.0 / PRECISION;
        let frac = self.0 % PRECISION;
        write!(f, "{whole}.{frac:04}")
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
        s.parse::<Amount>().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amt(s: &str) -> Amount {
        s.parse().unwrap()
    }

    // --------------Parse tests-----------------
    #[test]
    fn parse_integer() {
        assert_eq!(amt("1"), Amount(10_000));
    }

    #[test]
    fn parse_one_decimal() {
        assert_eq!(amt("1.5"), Amount(15_000));
    }

    #[test]
    fn parse_four_decimals() {
        assert_eq!(amt("1.5000"), Amount(15_000));
    }

    #[test]
    fn parse_max_precision() {
        assert_eq!(amt("0.0001"), Amount(1));
    }

    #[test]
    fn parse_whitespace_trimmed() {
        assert_eq!(amt("  1.5  "), Amount(15_000));
    }

    #[test]
    fn parse_zero() {
        assert_eq!(amt("0.0"), Amount::ZERO);
    }

    #[test]
    fn parse_too_many_decimals() {
        let err = "1.00001".parse::<Amount>().unwrap_err();
        assert!(matches!(err, AmountError::TooManyDecimalPlaces(_)));
    }

    #[test]
    fn parse_invalid_format() {
        let err = "abc".parse::<Amount>().unwrap_err();
        assert!(matches!(err, AmountError::InvalidFormat(_)));
    }

    #[test]
    fn parse_negative_rejected() {
        let err = "-1.0".parse::<Amount>().unwrap_err();
        assert!(matches!(err, AmountError::Negative));
    }

    // --------------Display tests-----------------

    #[test]
    fn display_round_trip() {
        assert_eq!(amt("1.5000").to_string(), "1.5000");
    }

    #[test]
    fn display_zero_pads_fraction() {
        assert_eq!(amt("1.5").to_string(), "1.5000");
    }

    #[test]
    fn display_zero() {
        assert_eq!(Amount::ZERO.to_string(), "0.0000");
    }

    // --------------Addition-----------------

    #[test]
    fn add_two_amounts() {
        assert_eq!(amt("1.0").checked_add(amt("2.0")).unwrap(), amt("3.0"));
    }

    #[test]
    fn add_overflow() {
        let max = Amount(i64::MAX);
        let err = max.checked_add(Amount(1)).unwrap_err();
        assert!(matches!(err, AmountError::Overflow));
    }

    // --------------Subtraction-----------------

    #[test]
    fn sub_exact() {
        assert_eq!(amt("5.0").checked_sub(amt("3.0")).unwrap(), amt("2.0"));
    }

    #[test]
    fn sub_to_zero() {
        assert_eq!(amt("1.0").checked_sub(amt("1.0")).unwrap(), Amount::ZERO);
    }

    #[test]
    fn sub_insufficient_funds() {
        let err = amt("1.0").checked_sub(amt("2.0")).unwrap_err();
        assert!(matches!(err, AmountError::InsufficientFunds));
    }

    // --------------Comparison test-----------------

    #[test]
    fn gte_greater() {
        assert!(amt("2.0").is_gte(amt("1.0")));
    }

    #[test]
    fn gte_equal() {
        assert!(amt("1.0").is_gte(amt("1.0")));
    }

    #[test]
    fn gte_less() {
        assert!(!amt("1.0").is_gte(amt("2.0")));
    }
}
