use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::types::tagged_scalar::serialize_tagged_scalar;

const REGEX_TAG: &str = "$surrealdb::regex";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Regex(pub String);

impl Regex {
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for Regex {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for Regex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<Regex> for String {
    fn from(value: Regex) -> Self {
        value.0
    }
}

impl From<String> for Regex {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Regex {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl Serialize for Regex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_tagged_scalar(serializer, REGEX_TAG, &self.0)
    }
}

impl<'de> Deserialize<'de> for Regex {
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
            Repr::String(value) => Ok(Regex(value)),
            Repr::Tagged(mut values) => {
                if let Some(value) = values.remove(REGEX_TAG) {
                    return Ok(Regex(value));
                }
                if let Some(value) = values.remove("regex") {
                    return Ok(Regex(value));
                }
                if values.len() == 1 {
                    let (_, value) = values.into_iter().next().expect("single entry map");
                    return Ok(Regex(value));
                }

                Err(serde::de::Error::custom("invalid regex representation"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Regex;

    #[test]
    fn serializes_as_tagged_map() {
        let value = serde_json::to_value(Regex::from("^[a-z]+$")).unwrap();
        assert_eq!(value, json!({"$surrealdb::regex": "^[a-z]+$"}));
    }
}
