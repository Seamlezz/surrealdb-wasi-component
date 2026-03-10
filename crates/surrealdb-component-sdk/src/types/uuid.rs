use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::types::tagged_scalar::serialize_tagged_scalar;

const UUID_TAG: &str = "$surrealdb::uuid";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Uuid(pub String);

impl Uuid {
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for Uuid {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<Uuid> for String {
    fn from(value: Uuid) -> Self {
        value.0
    }
}

impl From<String> for Uuid {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Uuid {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl Serialize for Uuid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_tagged_scalar(serializer, UUID_TAG, &self.0)
    }
}

impl<'de> Deserialize<'de> for Uuid {
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
            Repr::String(value) => Ok(Uuid(value)),
            Repr::Tagged(mut values) => {
                if let Some(value) = values.remove(UUID_TAG) {
                    return Ok(Uuid(value));
                }
                if let Some(value) = values.remove("uuid") {
                    return Ok(Uuid(value));
                }
                if values.len() == 1 {
                    let (_, value) = values.into_iter().next().expect("single entry map");
                    return Ok(Uuid(value));
                }

                Err(serde::de::Error::custom("invalid uuid representation"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Uuid;

    #[test]
    fn serializes_as_tagged_map() {
        let value =
            serde_json::to_value(Uuid::from("018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d")).unwrap();
        assert_eq!(
            value,
            json!({"$surrealdb::uuid": "018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d"})
        );
    }
}
