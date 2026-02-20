use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::ValidationError;

const MAX_SYMBOL_LEN: usize = 15;

/// Normalized market symbol/ticker.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Symbol(String);

impl Symbol {
    /// Parse and normalize a symbol to uppercase.
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::EmptySymbol);
        }

        let normalized = trimmed.to_ascii_uppercase();
        let len = normalized.chars().count();
        if len > MAX_SYMBOL_LEN {
            return Err(ValidationError::SymbolTooLong {
                len,
                max: MAX_SYMBOL_LEN,
            });
        }

        if let Some(first) = normalized.chars().next() {
            if !first.is_ascii_alphabetic() {
                return Err(ValidationError::SymbolInvalidStart { ch: first });
            }
        }

        for (index, ch) in normalized.chars().enumerate() {
            let valid = ch.is_ascii_alphanumeric() || ch == '.' || ch == '-';
            if !valid {
                return Err(ValidationError::SymbolInvalidChar { ch, index });
            }
        }

        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<String> for Symbol {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl TryFrom<&str> for Symbol {
    type Error = ValidationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<Symbol> for String {
    fn from(value: Symbol) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_normalizes_symbol() {
        let parsed = Symbol::parse(" aapl ").expect("symbol should parse");
        assert_eq!(parsed.as_str(), "AAPL");
    }

    #[test]
    fn rejects_invalid_start() {
        let err = Symbol::parse("1AAPL").expect_err("must fail");
        assert!(matches!(err, ValidationError::SymbolInvalidStart { .. }));
    }

    #[test]
    fn rejects_invalid_chars() {
        let err = Symbol::parse("AAPL$").expect_err("must fail");
        assert!(matches!(err, ValidationError::SymbolInvalidChar { .. }));
    }
}
