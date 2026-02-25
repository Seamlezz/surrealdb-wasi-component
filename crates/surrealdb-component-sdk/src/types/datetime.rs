use std::fmt;
use std::ops::Deref;

use chrono::{DateTime, TimeZone, Utc};
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Datetime(pub DateTime<Utc>);

impl Datetime {
    pub fn into_inner(self) -> DateTime<Utc> {
        self.0
    }
}

impl Deref for Datetime {
    type Target = DateTime<Utc>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for Datetime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_rfc3339())
    }
}

impl From<Datetime> for DateTime<Utc> {
    fn from(value: Datetime) -> Self {
        value.0
    }
}

impl From<DateTime<Utc>> for Datetime {
    fn from(value: DateTime<Utc>) -> Self {
        Self(value)
    }
}

impl From<Datetime> for prost_types::Timestamp {
    fn from(value: Datetime) -> Self {
        Self {
            seconds: value.0.timestamp(),
            nanos: value.0.timestamp_subsec_nanos() as i32,
        }
    }
}

impl From<Datetime> for Option<prost_types::Timestamp> {
    fn from(value: Datetime) -> Self {
        Some(value.into())
    }
}

struct DatetimeVisitor;

impl Visitor<'_> for DatetimeVisitor {
    type Value = Datetime;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a RFC3339 datetime string or unix timestamp")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        DateTime::parse_from_rfc3339(value)
            .map(|dt| Datetime(dt.with_timezone(&Utc)))
            .map_err(Error::custom)
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Utc.timestamp_opt(value, 0)
            .single()
            .map(Datetime)
            .ok_or_else(|| Error::custom("invalid unix timestamp"))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_i64(value as i64)
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let seconds = value.trunc() as i64;
        let nanos = ((value.fract().abs()) * 1_000_000_000.0) as u32;
        Utc.timestamp_opt(seconds, nanos)
            .single()
            .map(Datetime)
            .ok_or_else(|| Error::custom("invalid floating point unix timestamp"))
    }
}

impl<'de> Deserialize<'de> for Datetime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DatetimeVisitor)
    }
}
