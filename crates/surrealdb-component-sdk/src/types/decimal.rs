use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize};

const DECIMAL_TAG: &str = "$surrealdb::decimal";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
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
