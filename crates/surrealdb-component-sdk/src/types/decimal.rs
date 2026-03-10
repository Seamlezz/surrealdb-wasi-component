use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::types::tagged_scalar::serialize_tagged_scalar;

const DECIMAL_TAG: &str = "$surrealdb::decimal";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Decimal(pub String);

impl Decimal {
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for Decimal {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<Decimal> for String {
    fn from(value: Decimal) -> Self {
        value.0
    }
}

impl From<String> for Decimal {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Decimal {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl Serialize for Decimal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_tagged_scalar(serializer, DECIMAL_TAG, &self.0)
    }
}

impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            String(String),
            Tagged(HashMap<String, String>),
        }

        let repr = Repr::deserialize(deserializer)?;
        match repr {
            Repr::String(value) => Ok(Decimal(value)),
            Repr::Tagged(mut values) => {
                if let Some(value) = values.remove(DECIMAL_TAG) {
                    return Ok(Decimal(value));
                }
                if let Some(value) = values.remove("decimal") {
                    return Ok(Decimal(value));
                }
                if values.len() == 1 {
                    let (_, value) = values.into_iter().next().expect("single entry map");
                    return Ok(Decimal(value));
                }

                Err(serde::de::Error::custom("invalid decimal representation"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Decimal;

    #[test]
    fn serializes_as_tagged_map() {
        let value = serde_json::to_value(Decimal::from("12.34dec")).unwrap();
        assert_eq!(value, json!({"$surrealdb::decimal": "12.34dec"}));
    }
}
