use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::types::tagged_scalar::serialize_tagged_scalar;

const DURATION_TAG: &str = "$surrealdb::duration";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Duration(pub String);

impl Duration {
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for Duration {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<Duration> for String {
    fn from(value: Duration) -> Self {
        value.0
    }
}

impl From<String> for Duration {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Duration {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl Serialize for Duration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_tagged_scalar(serializer, DURATION_TAG, &self.0)
    }
}

impl<'de> Deserialize<'de> for Duration {
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
            Repr::String(value) => Ok(Duration(value)),
            Repr::Tagged(mut values) => {
                if let Some(value) = values.remove(DURATION_TAG) {
                    return Ok(Duration(value));
                }
                if let Some(value) = values.remove("duration") {
                    return Ok(Duration(value));
                }
                if values.len() == 1 {
                    let (_, value) = values.into_iter().next().expect("single entry map");
                    return Ok(Duration(value));
                }

                Err(serde::de::Error::custom("invalid duration representation"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Duration;

    #[test]
    fn serializes_as_tagged_map() {
        let value = serde_json::to_value(Duration::from("1h30m")).unwrap();
        assert_eq!(value, json!({"$surrealdb::duration": "1h30m"}));
    }
}
