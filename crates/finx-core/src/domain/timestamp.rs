use std::fmt::{Display, Formatter};

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

use crate::ValidationError;

/// RFC3339 timestamp guaranteed to be UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcDateTime(OffsetDateTime);

impl UtcDateTime {
    pub fn now() -> Self {
        Self(OffsetDateTime::now_utc())
    }

    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        let parsed = OffsetDateTime::parse(input, &Rfc3339).map_err(|_| {
            ValidationError::TimestampNotUtc {
                value: input.to_owned(),
            }
        })?;

        Self::from_offset_datetime(parsed).map_err(|_| ValidationError::TimestampNotUtc {
            value: input.to_owned(),
        })
    }

    pub fn from_offset_datetime(value: OffsetDateTime) -> Result<Self, ValidationError> {
        if value.offset() != UtcOffset::UTC {
            return Err(ValidationError::TimestampNotUtc {
                value: value
                    .format(&Rfc3339)
                    .unwrap_or_else(|_| String::from("<unformattable>")),
            });
        }

        Ok(Self(value))
    }

    pub fn into_inner(self) -> OffsetDateTime {
        self.0
    }

    pub fn format_rfc3339(self) -> String {
        self.0
            .format(&Rfc3339)
            .expect("UtcDateTime must be RFC3339 formattable")
    }
}

impl Display for UtcDateTime {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.format_rfc3339())
    }
}

impl Serialize for UtcDateTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.format_rfc3339())
    }
}

impl<'de> Deserialize<'de> for UtcDateTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_utc_timestamp() {
        let parsed = UtcDateTime::parse("2024-01-01T00:00:00Z").expect("must parse");
        assert_eq!(parsed.format_rfc3339(), "2024-01-01T00:00:00Z");
    }

    #[test]
    fn rejects_non_utc_timestamp() {
        let err = UtcDateTime::parse("2024-01-01T01:00:00+01:00").expect_err("must fail");
        assert!(matches!(err, ValidationError::TimestampNotUtc { .. }));
    }
}
