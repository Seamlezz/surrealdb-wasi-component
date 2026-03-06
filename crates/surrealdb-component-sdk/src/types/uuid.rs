use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize};

const UUID_TAG: &str = "$surrealdb::uuid";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
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
