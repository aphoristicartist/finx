use serde::{Deserialize, Serialize};

use crate::{Interval, Symbol, UtcDateTime, ValidationError};

/// Canonical instrument class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    Equity,
    Etf,
    Index,
    Crypto,
    Forex,
    Fund,
    Other,
}

/// Canonical instrument metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instrument {
    pub symbol: Symbol,
    pub name: String,
    pub exchange: Option<String>,
    pub currency: String,
    pub asset_class: AssetClass,
    pub is_active: bool,
}

impl Instrument {
    pub fn new(
        symbol: Symbol,
        name: impl Into<String>,
        exchange: Option<String>,
        currency: impl AsRef<str>,
        asset_class: AssetClass,
        is_active: bool,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            symbol,
            name: name.into(),
            exchange,
            currency: validate_currency_code(currency.as_ref())?,
            asset_class,
            is_active,
        })
    }
}

/// Canonical top-of-book quote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: Symbol,
    pub price: f64,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub volume: Option<u64>,
    pub currency: String,
    pub as_of: UtcDateTime,
}

impl Quote {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: Symbol,
        price: f64,
        bid: Option<f64>,
        ask: Option<f64>,
        volume: Option<u64>,
        currency: impl AsRef<str>,
        as_of: UtcDateTime,
    ) -> Result<Self, ValidationError> {
        validate_non_negative("price", price)?;
        validate_optional_non_negative("bid", bid)?;
        validate_optional_non_negative("ask", ask)?;

        Ok(Self {
            symbol,
            price,
            bid,
            ask,
            volume,
            currency: validate_currency_code(currency.as_ref())?,
            as_of,
        })
    }
}

/// OHLCV bar record for a given interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    pub ts: UtcDateTime,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: Option<u64>,
    pub vwap: Option<f64>,
}

impl Bar {
    pub fn new(
        ts: UtcDateTime,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: Option<u64>,
        vwap: Option<f64>,
    ) -> Result<Self, ValidationError> {
        validate_non_negative("open", open)?;
        validate_non_negative("high", high)?;
        validate_non_negative("low", low)?;
        validate_non_negative("close", close)?;
        validate_optional_non_negative("vwap", vwap)?;

        if high < low {
            return Err(ValidationError::InvalidBarRange);
        }

        if open < low || open > high || close < low || close > high {
            return Err(ValidationError::InvalidBarBounds);
        }

        Ok(Self {
            ts,
            open,
            high,
            low,
            close,
            volume,
            vwap,
        })
    }
}

/// Series wrapper used by bar endpoints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BarSeries {
    pub symbol: Symbol,
    pub interval: Interval,
    pub bars: Vec<Bar>,
}

impl BarSeries {
    pub fn new(symbol: Symbol, interval: Interval, bars: Vec<Bar>) -> Self {
        Self {
            symbol,
            interval,
            bars,
        }
    }
}

/// Canonical fundamentals snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fundamental {
    pub symbol: Symbol,
    pub as_of: UtcDateTime,
    pub market_cap: Option<f64>,
    pub pe_ratio: Option<f64>,
    pub dividend_yield: Option<f64>,
}

impl Fundamental {
    pub fn new(
        symbol: Symbol,
        as_of: UtcDateTime,
        market_cap: Option<f64>,
        pe_ratio: Option<f64>,
        dividend_yield: Option<f64>,
    ) -> Result<Self, ValidationError> {
        validate_optional_non_negative("market_cap", market_cap)?;
        validate_optional_finite("pe_ratio", pe_ratio)?;
        validate_optional_non_negative("dividend_yield", dividend_yield)?;

        Ok(Self {
            symbol,
            as_of,
            market_cap,
            pe_ratio,
            dividend_yield,
        })
    }
}

/// Canonical corporate action type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorporateActionType {
    Dividend,
    Split,
    Spinoff,
    Merger,
    RightsIssue,
    Other,
}

/// Canonical corporate action event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorporateAction {
    pub symbol: Symbol,
    pub action_type: CorporateActionType,
    pub ex_date: UtcDateTime,
    pub pay_date: Option<UtcDateTime>,
    pub value: Option<f64>,
    pub currency: Option<String>,
}

impl CorporateAction {
    pub fn new(
        symbol: Symbol,
        action_type: CorporateActionType,
        ex_date: UtcDateTime,
        pay_date: Option<UtcDateTime>,
        value: Option<f64>,
        currency: Option<&str>,
    ) -> Result<Self, ValidationError> {
        validate_optional_non_negative("value", value)?;

        Ok(Self {
            symbol,
            action_type,
            ex_date,
            pay_date,
            value,
            currency: currency.map(validate_currency_code).transpose()?,
        })
    }
}

/// Validate and normalize currency to uppercase 3-letter code.
pub fn validate_currency_code(input: &str) -> Result<String, ValidationError> {
    let normalized = input.trim().to_ascii_uppercase();
    let is_valid = normalized.len() == 3 && normalized.chars().all(|ch| ch.is_ascii_alphabetic());

    if !is_valid {
        return Err(ValidationError::InvalidCurrency {
            value: input.to_owned(),
        });
    }

    Ok(normalized)
}

fn validate_non_negative(field: &'static str, value: f64) -> Result<(), ValidationError> {
    if !value.is_finite() {
        return Err(ValidationError::NonFiniteValue { field });
    }
    if value < 0.0 {
        return Err(ValidationError::NegativeValue { field });
    }
    Ok(())
}

fn validate_optional_non_negative(
    field: &'static str,
    value: Option<f64>,
) -> Result<(), ValidationError> {
    if let Some(value) = value {
        validate_non_negative(field, value)?;
    }
    Ok(())
}

fn validate_optional_finite(
    field: &'static str,
    value: Option<f64>,
) -> Result<(), ValidationError> {
    if let Some(value) = value {
        if !value.is_finite() {
            return Err(ValidationError::NonFiniteValue { field });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_currency() {
        assert_eq!(
            validate_currency_code("usd").expect("must normalize"),
            "USD"
        );
        assert!(matches!(
            validate_currency_code("USDT"),
            Err(ValidationError::InvalidCurrency { .. })
        ));
    }

    #[test]
    fn rejects_invalid_bar_bounds() {
        let ts = UtcDateTime::parse("2024-01-01T00:00:00Z").expect("timestamp");
        let err = Bar::new(ts, 10.0, 12.0, 9.0, 12.5, Some(10), None).expect_err("must fail");
        assert!(matches!(err, ValidationError::InvalidBarBounds));
    }
}
