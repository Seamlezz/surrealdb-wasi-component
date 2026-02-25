use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct RecordId {
    pub tb: String,
    pub id: RecordIdKey,
}

impl RecordId {
    pub fn new(tb: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            tb: tb.into(),
            id: RecordIdKey::String(id.into()),
        }
    }
}

impl fmt::Display for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.tb, self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum RecordIdKey {
    Number(i64),
    String(String),
    Uuid(String),
    Array(Vec<RecordIdValue>),
    Object(HashMap<String, RecordIdValue>),
}

impl Hash for RecordIdKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Self::Number(value) => value.hash(state),
            Self::String(value) => value.hash(state),
            Self::Uuid(value) => value.hash(state),
            Self::Array(value) => value.hash(state),
            Self::Object(value) => {
                for (key, val) in value {
                    key.hash(state);
                    val.hash(state);
                }
            }
        }
    }
}

impl fmt::Display for RecordIdKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(value) => write!(f, "{value}"),
            Self::String(value) => write!(f, "{value}"),
            Self::Uuid(value) => write!(f, "{value}"),
            Self::Array(values) => {
                write!(f, "[")?;
                for (index, item) in values.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Self::Object(values) => {
                write!(f, "{{")?;
                for (index, (key, value)) in values.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{key}\": {value}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum RecordIdValue {
    Null,
    Bool(bool),
    Number(i64),
    Float(f64),
    String(String),
    Array(Vec<RecordIdValue>),
    Object(HashMap<String, RecordIdValue>),
}

impl Eq for RecordIdValue {}

impl Hash for RecordIdValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Self::Null => {}
            Self::Bool(value) => value.hash(state),
            Self::Number(value) => value.hash(state),
            Self::Float(value) => value.to_bits().hash(state),
            Self::String(value) => value.hash(state),
            Self::Array(value) => value.hash(state),
            Self::Object(value) => {
                for (key, val) in value {
                    key.hash(state);
                    val.hash(state);
                }
            }
        }
    }
}

impl fmt::Display for RecordIdValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::Number(value) => write!(f, "{value}"),
            Self::Float(value) => write!(f, "{value}"),
            Self::String(value) => write!(f, "\"{value}\""),
            Self::Array(values) => {
                write!(f, "[")?;
                for (index, item) in values.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Self::Object(values) => {
                write!(f, "{{")?;
                for (index, (key, value)) in values.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{key}\": {value}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

impl<'de> Deserialize<'de> for RecordIdValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RecordIdValueVisitor;

        impl<'de> Visitor<'de> for RecordIdValueVisitor {
            type Value = RecordIdValue;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a valid record id value")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(RecordIdValue::Null)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(RecordIdValue::Null)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                Deserialize::deserialize(deserializer)
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(RecordIdValue::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                Ok(RecordIdValue::Number(value))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RecordIdValue::Number(value as i64))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E> {
                Ok(RecordIdValue::Float(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RecordIdValue::String(value.to_string()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                Ok(RecordIdValue::String(value))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element()? {
                    values.push(value);
                }
                Ok(RecordIdValue::Array(values))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = HashMap::new();
                while let Some((key, value)) = map.next_entry()? {
                    values.insert(key, value);
                }
                Ok(RecordIdValue::Object(values))
            }
        }

        deserializer.deserialize_any(RecordIdValueVisitor)
    }
}

impl<'de> Deserialize<'de> for RecordIdKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum TaggedValue {
            String(String),
            Number(i64),
            Uuid(String),
            Array(Vec<RecordIdValue>),
            Object(HashMap<String, RecordIdValue>),
            Bool(bool),
            Float(f64),
            Null,
        }

        impl TaggedValue {
            fn into_record_value(self) -> RecordIdValue {
                match self {
                    Self::String(v) => RecordIdValue::String(v),
                    Self::Number(v) => RecordIdValue::Number(v),
                    Self::Uuid(v) => RecordIdValue::String(v),
                    Self::Array(v) => RecordIdValue::Array(v),
                    Self::Object(v) => RecordIdValue::Object(v),
                    Self::Bool(v) => RecordIdValue::Bool(v),
                    Self::Float(v) => RecordIdValue::Float(v),
                    Self::Null => RecordIdValue::Null,
                }
            }
        }

        struct RecordIdKeyVisitor;

        impl<'de> Visitor<'de> for RecordIdKeyVisitor {
            type Value = RecordIdKey;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a valid record id key")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RecordIdKey::String(value.to_string()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                Ok(RecordIdKey::String(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                Ok(RecordIdKey::Number(value))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RecordIdKey::Number(value as i64))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element::<RecordIdValue>()? {
                    values.push(value);
                }
                Ok(RecordIdKey::Array(values))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut fields: HashMap<String, TaggedValue> = HashMap::new();
                while let Some((key, value)) = map.next_entry()? {
                    fields.insert(key, value);
                }

                if fields.len() == 1 {
                    let (key, value) = fields.into_iter().next().expect("single entry map");
                    return Ok(match (key.as_str(), value) {
                        ("String", TaggedValue::String(v)) => RecordIdKey::String(v),
                        ("Number", TaggedValue::Number(v)) => RecordIdKey::Number(v),
                        ("Integer", TaggedValue::Number(v)) => RecordIdKey::Number(v),
                        ("Uuid", TaggedValue::Uuid(v)) => RecordIdKey::Uuid(v),
                        ("Array", TaggedValue::Array(v)) => RecordIdKey::Array(v),
                        ("Object", TaggedValue::Object(v)) => RecordIdKey::Object(v),
                        (tag, other) => {
                            let mut object = HashMap::new();
                            object.insert(tag.to_string(), other.into_record_value());
                            RecordIdKey::Object(object)
                        }
                    });
                }

                let mapped = fields
                    .into_iter()
                    .map(|(k, v)| (k, v.into_record_value()))
                    .collect();
                Ok(RecordIdKey::Object(mapped))
            }
        }

        deserializer.deserialize_any(RecordIdKeyVisitor)
    }
}

impl<'de> Deserialize<'de> for RecordId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RecordIdInner {
            tb: String,
            id: RecordIdKey,
        }

        let inner = RecordIdInner::deserialize(deserializer)?;
        Ok(Self {
            tb: inner.tb,
            id: inner.id,
        })
    }
}

impl From<i64> for RecordIdKey {
    fn from(value: i64) -> Self {
        Self::Number(value)
    }
}

impl From<String> for RecordIdKey {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for RecordIdKey {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<Vec<RecordIdValue>> for RecordIdKey {
    fn from(value: Vec<RecordIdValue>) -> Self {
        Self::Array(value)
    }
}

impl From<HashMap<String, RecordIdValue>> for RecordIdKey {
    fn from(value: HashMap<String, RecordIdValue>) -> Self {
        Self::Object(value)
    }
}
