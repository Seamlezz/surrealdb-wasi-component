use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Geometry(pub serde_json::Value);

impl Geometry {
    pub fn into_inner(self) -> serde_json::Value {
        self.0
    }
}

impl Deref for Geometry {
    type Target = serde_json::Value;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<serde_json::Value> for Geometry {
    fn from(value: serde_json::Value) -> Self {
        Self(value)
    }
}

impl From<Geometry> for serde_json::Value {
    fn from(value: Geometry) -> Self {
        value.0
    }
}

impl fmt::Display for Geometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
