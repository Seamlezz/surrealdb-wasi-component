use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::types::tagged_scalar::serialize_tagged_scalar;

const UUID_TAG: &str = "$surrealdb::uuid";

// =============================================================================
// Display helpers
// =============================================================================

/// Returns true if the string is a "simple" identifier that can be displayed
/// without backtick quoting. Simple means: only alphanumeric + underscore,
/// and not parseable as an i64 (to avoid ambiguity with numeric IDs).
fn is_simple_id(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return false;
    }
    s.parse::<i64>().is_err()
}

/// Format a string as a record ID key: bare if simple, backtick-quoted otherwise.
fn fmt_string_key(s: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if is_simple_id(s) {
        write!(f, "{s}")
    } else {
        write!(f, "`{s}`")
    }
}

/// Format a string as a record ID value: single-quoted with internal single quotes escaped.
fn fmt_string_value(s: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "'")?;
    for c in s.chars() {
        if c == '\'' {
            write!(f, "\\'")?;
        } else {
            write!(f, "{c}")?;
        }
    }
    write!(f, "'")
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct RecordId {
    pub table: String,
    pub key: RecordIdKey,
}

impl RecordId {
    pub fn new(table: impl Into<String>, key: impl Into<RecordIdKey>) -> Self {
        Self {
            table: table.into(),
            key: key.into(),
        }
    }
}

impl fmt::Display for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_string_key(&self.table, f)?;
        write!(f, ":")?;
        fmt::Display::fmt(&self.key, f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordIdKey {
    Number(i64),
    String(String),
    Uuid(String),
    Array(Vec<RecordIdValue>),
    Object(HashMap<String, RecordIdValue>),
}

impl RecordIdKey {
    pub fn number(value: i64) -> Self {
        Self::Number(value)
    }

    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    pub fn uuid(value: impl Into<String>) -> Self {
        Self::Uuid(value.into())
    }

    pub fn array<I>(values: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<RecordIdValue>,
    {
        Self::Array(values.into_iter().map(Into::into).collect())
    }

    pub fn object<I, K, V>(values: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<RecordIdValue>,
    {
        Self::Object(
            values
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        )
    }
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

impl Serialize for RecordIdKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Number(value) => value.serialize(serializer),
            Self::String(value) => value.serialize(serializer),
            Self::Uuid(value) => serialize_tagged_scalar(serializer, UUID_TAG, value),
            Self::Array(value) => value.serialize(serializer),
            Self::Object(value) => value.serialize(serializer),
        }
    }
}

impl fmt::Display for RecordIdKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(value) => write!(f, "{value}"),
            Self::String(value) => fmt_string_key(value, f),
            Self::Uuid(value) => write!(f, "u'{value}'"),
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
                    fmt_string_key(key, f)?;
                    write!(f, ": {value}")?;
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

impl RecordIdValue {
    pub fn null() -> Self {
        Self::Null
    }

    pub fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    pub fn number(value: i64) -> Self {
        Self::Number(value)
    }

    pub fn float(value: f64) -> Self {
        Self::Float(value)
    }

    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    pub fn array<I>(values: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<RecordIdValue>,
    {
        Self::Array(values.into_iter().map(Into::into).collect())
    }

    pub fn object<I, K, V>(values: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<RecordIdValue>,
    {
        Self::Object(
            values
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        )
    }
}

impl From<bool> for RecordIdValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for RecordIdValue {
    fn from(value: i64) -> Self {
        Self::Number(value)
    }
}

impl From<f64> for RecordIdValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<String> for RecordIdValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for RecordIdValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<Vec<RecordIdValue>> for RecordIdValue {
    fn from(value: Vec<RecordIdValue>) -> Self {
        Self::Array(value)
    }
}

impl From<HashMap<String, RecordIdValue>> for RecordIdValue {
    fn from(value: HashMap<String, RecordIdValue>) -> Self {
        Self::Object(value)
    }
}

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
            Self::Null => write!(f, "NONE"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::Number(value) => write!(f, "{value}"),
            Self::Float(value) => write!(f, "{value}f"),
            Self::String(value) => fmt_string_value(value, f),
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
                    fmt_string_key(key, f)?;
                    write!(f, ": {value}")?;
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
                        ("$surrealdb::uuid", TaggedValue::String(v)) => RecordIdKey::Uuid(v),
                        ("uuid", TaggedValue::String(v)) => RecordIdKey::Uuid(v),
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
        struct CanonicalRecordId {
            table: String,
            key: RecordIdKey,
        }

        #[derive(Deserialize)]
        struct LegacyRecordId {
            tb: String,
            id: RecordIdKey,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RecordIdInner {
            Canonical(CanonicalRecordId),
            Legacy(LegacyRecordId),
        }

        match RecordIdInner::deserialize(deserializer)? {
            RecordIdInner::Canonical(inner) => Ok(Self {
                table: inner.table,
                key: inner.key,
            }),
            RecordIdInner::Legacy(inner) => Ok(Self {
                table: inner.tb,
                key: inner.id,
            }),
        }
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

impl From<crate::types::Uuid> for RecordIdKey {
    fn from(value: crate::types::Uuid) -> Self {
        Self::Uuid(value.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{RecordId, RecordIdKey, RecordIdValue};

    #[test]
    fn serializes_uuid_keys_as_tagged_maps() {
        let value = serde_json::to_value(RecordIdKey::Uuid(
            "018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d".to_string(),
        ))
        .unwrap();

        assert_eq!(
            value,
            json!({"$surrealdb::uuid": "018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d"})
        );
    }

    #[test]
    fn record_id_new_accepts_string_key() {
        let value = serde_json::to_value(RecordId::new("person", "demo")).unwrap();
        assert_eq!(value, json!({"table": "person", "key": "demo"}));
    }

    #[test]
    fn record_id_new_accepts_number_key() {
        let value = serde_json::to_value(RecordId::new("person", 42_i64)).unwrap();
        assert_eq!(value, json!({"table": "person", "key": 42}));
    }

    #[test]
    fn record_id_new_accepts_uuid_key() {
        let value = serde_json::to_value(RecordId::new(
            "person",
            RecordIdKey::uuid("018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d"),
        ))
        .unwrap();
        assert_eq!(
            value,
            json!({
                "table": "person",
                "key": {"$surrealdb::uuid": "018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d"}
            })
        );
    }

    #[test]
    fn record_id_new_accepts_array_key() {
        let value = serde_json::to_value(RecordId::new(
            "person",
            RecordIdKey::array([RecordIdValue::string("tenant-a"), RecordIdValue::number(42)]),
        ))
        .unwrap();
        assert_eq!(value, json!({"table": "person", "key": ["tenant-a", 42]}));
    }

    #[test]
    fn record_id_new_accepts_object_key() {
        let value = serde_json::to_value(RecordId::new(
            "person",
            RecordIdKey::object([
                ("tenant", RecordIdValue::string("demo")),
                ("shard", RecordIdValue::number(1)),
            ]),
        ))
        .unwrap();
        assert_eq!(
            value,
            json!({
                "table": "person",
                "key": {"tenant": "demo", "shard": 1}
            })
        );
    }

    #[test]
    fn deserializes_canonical_record_id_with_string_key() {
        let value: RecordId =
            serde_json::from_value(json!({"table": "person", "key": "demo"})).unwrap();
        assert_eq!(value.table, "person");
        assert_eq!(value.key, RecordIdKey::String("demo".to_string()));
    }

    #[test]
    fn deserializes_canonical_record_id_with_number_key() {
        let value: RecordId =
            serde_json::from_value(json!({"table": "person", "key": 42})).unwrap();
        assert_eq!(value.table, "person");
        assert_eq!(value.key, RecordIdKey::Number(42));
    }

    #[test]
    fn deserializes_canonical_record_id_with_uuid_key() {
        let value: RecordId = serde_json::from_value(json!({
            "table": "person",
            "key": {"$surrealdb::uuid": "018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d"}
        }))
        .unwrap();
        assert_eq!(value.table, "person");
        assert_eq!(
            value.key,
            RecordIdKey::Uuid("018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d".to_string())
        );
    }

    #[test]
    fn deserializes_canonical_record_id_with_array_key() {
        let value: RecordId = serde_json::from_value(json!({
            "table": "person",
            "key": ["tenant-a", 42]
        }))
        .unwrap();
        assert_eq!(value.table, "person");
        assert_eq!(
            value.key,
            RecordIdKey::Array(vec![
                RecordIdValue::String("tenant-a".to_string()),
                RecordIdValue::Number(42),
            ])
        );
    }

    #[test]
    fn deserializes_canonical_record_id_with_object_key() {
        let value: RecordId = serde_json::from_value(json!({
            "table": "person",
            "key": {"tenant": "demo", "shard": 1}
        }))
        .unwrap();
        assert_eq!(value.table, "person");
        assert!(matches!(value.key, RecordIdKey::Object(_)));
    }

    #[test]
    fn deserializes_legacy_record_id() {
        let value: RecordId =
            serde_json::from_value(json!({"tb": "person", "id": "demo"})).unwrap();
        assert_eq!(value.table, "person");
        assert_eq!(value.key, RecordIdKey::String("demo".to_string()));
    }

    #[test]
    fn record_id_value_from_impls() {
        assert_eq!(RecordIdValue::from(true), RecordIdValue::Bool(true));
        assert_eq!(RecordIdValue::from(42_i64), RecordIdValue::Number(42));
        assert_eq!(RecordIdValue::from(3.14_f64), RecordIdValue::Float(3.14));
        assert_eq!(
            RecordIdValue::from("hello"),
            RecordIdValue::String("hello".to_string())
        );
        assert_eq!(
            RecordIdValue::from("hello".to_string()),
            RecordIdValue::String("hello".to_string())
        );
    }
}
