use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ValidationError;

/// Supported time bucket intervals for bar data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Interval {
    #[serde(rename = "1m")]
    OneMinute,
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "15m")]
    FifteenMinutes,
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "1d")]
    OneDay,
}

impl Interval {
    pub const ALL: [Self; 5] = [
        Self::OneMinute,
        Self::FiveMinutes,
        Self::FifteenMinutes,
        Self::OneHour,
        Self::OneDay,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneMinute => "1m",
            Self::FiveMinutes => "5m",
            Self::FifteenMinutes => "15m",
            Self::OneHour => "1h",
            Self::OneDay => "1d",
        }
    }
}

impl Display for Interval {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Interval {
    type Err = ValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "1m" => Ok(Self::OneMinute),
            "5m" => Ok(Self::FiveMinutes),
            "15m" => Ok(Self::FifteenMinutes),
            "1h" => Ok(Self::OneHour),
            "1d" => Ok(Self::OneDay),
            other => Err(ValidationError::InvalidInterval {
                value: other.to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_interval() {
        let interval = Interval::from_str("1d").expect("must parse");
        assert_eq!(interval, Interval::OneDay);
    }

    #[test]
    fn rejects_invalid_interval() {
        let err = Interval::from_str("2h").expect_err("must fail");
        assert!(matches!(err, ValidationError::InvalidInterval { .. }));
    }
}
