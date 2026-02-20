use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ValidationError;

/// Canonical provider identifiers used in metadata and envelopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Yahoo,
    Polygon,
    Alphavantage,
    Alpaca,
}

impl ProviderId {
    pub const ALL: [Self; 4] = [Self::Yahoo, Self::Polygon, Self::Alphavantage, Self::Alpaca];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Yahoo => "yahoo",
            Self::Polygon => "polygon",
            Self::Alphavantage => "alphavantage",
            Self::Alpaca => "alpaca",
        }
    }
}

impl Display for ProviderId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ProviderId {
    type Err = ValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "yahoo" => Ok(Self::Yahoo),
            "polygon" => Ok(Self::Polygon),
            "alphavantage" => Ok(Self::Alphavantage),
            "alpaca" => Ok(Self::Alpaca),
            other => Err(ValidationError::InvalidSource {
                value: other.to_owned(),
            }),
        }
    }
}
