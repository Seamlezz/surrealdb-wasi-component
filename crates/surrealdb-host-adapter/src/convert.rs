use std::collections::BTreeMap;

use anyhow::{bail, Result};
use serde_cbor::Value as CborValue;
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use surrealdb_types::{
    Number as SurrealNumber, RecordIdKey as SurrealRecordIdKey, Value as SurrealValue,
};

const UUID_TAG: &str = "$surrealdb::uuid";
const DURATION_TAG: &str = "$surrealdb::duration";
const DECIMAL_TAG: &str = "$surrealdb::decimal";
const REGEX_TAG: &str = "$surrealdb::regex";

pub fn cbor_slice_to_json(bytes: &[u8]) -> Result<JsonValue> {
    let value: CborValue = serde_cbor::from_slice(bytes)?;
    cbor_to_json(value)
}

pub fn cbor_to_json(value: CborValue) -> Result<JsonValue> {
    Ok(match value {
        CborValue::Null => JsonValue::Null,
        CborValue::Bool(v) => JsonValue::Bool(v),
        CborValue::Integer(v) => {
            if v >= i64::MIN as i128 && v <= i64::MAX as i128 {
                JsonValue::Number(JsonNumber::from(v as i64))
            } else if v >= 0 && v <= u64::MAX as i128 {
                JsonValue::Number(JsonNumber::from(v as u64))
            } else {
                bail!("unsupported integer representation")
            }
        }
        CborValue::Float(v) => {
            let Some(number) = JsonNumber::from_f64(v) else {
                bail!("invalid float value")
            };
            JsonValue::Number(number)
        }
        CborValue::Bytes(v) => JsonValue::Array(
            v.into_iter()
                .map(|byte| JsonValue::Number(byte.into()))
                .collect(),
        ),
        CborValue::Text(v) => JsonValue::String(v),
        CborValue::Array(values) => JsonValue::Array(
            values
                .into_iter()
                .map(cbor_to_json)
                .collect::<Result<Vec<_>>>()?,
        ),
        CborValue::Map(values) => {
            let mut object = JsonMap::new();
            for (key, value) in values {
                let key = match key {
                    CborValue::Text(text) => text,
                    CborValue::Integer(integer) => integer.to_string(),
                    _ => bail!("unsupported map key type"),
                };
                object.insert(key, cbor_to_json(value)?);
            }
            JsonValue::Object(object)
        }
        CborValue::Tag(_, value) => cbor_to_json(*value)?,
        _ => bail!("unsupported cbor value"),
    })
}

pub fn surreal_to_cbor_bytes(value: SurrealValue) -> Result<Vec<u8>> {
    let normalized = surreal_to_cbor(value)?;
    Ok(serde_cbor::to_vec(&normalized)?)
}

fn surreal_to_cbor(value: SurrealValue) -> Result<CborValue> {
    Ok(match value {
        SurrealValue::None | SurrealValue::Null => CborValue::Null,
        SurrealValue::Bool(v) => CborValue::Bool(v),
        SurrealValue::Number(v) => number_to_cbor(v),
        SurrealValue::String(v) => CborValue::Text(v),
        SurrealValue::Bytes(v) => CborValue::Bytes(v.into_inner().to_vec()),
        SurrealValue::Duration(v) => tagged_text(DURATION_TAG, v.to_string()),
        SurrealValue::Datetime(v) => CborValue::Text(v.to_string()),
        SurrealValue::Uuid(v) => tagged_text(UUID_TAG, v.to_string()),
        SurrealValue::Geometry(v) => serde_cbor::value::to_value(v)?,
        SurrealValue::Table(v) => CborValue::Text(v.to_string()),
        SurrealValue::RecordId(v) => {
            let mut map = BTreeMap::new();
            map.insert(
                CborValue::Text("table".to_string()),
                CborValue::Text(v.table.to_string()),
            );
            map.insert(
                CborValue::Text("key".to_string()),
                record_id_key_to_cbor(v.key)?,
            );
            CborValue::Map(map)
        }
        SurrealValue::File(v) => serde_cbor::value::to_value(v)?,
        SurrealValue::Range(v) => serde_cbor::value::to_value(v)?,
        SurrealValue::Regex(v) => tagged_text(REGEX_TAG, v.regex().as_str().to_string()),
        SurrealValue::Array(v) => CborValue::Array(
            v.into_iter()
                .map(surreal_to_cbor)
                .collect::<Result<Vec<_>>>()?,
        ),
        SurrealValue::Object(v) => {
            let mut entries: BTreeMap<String, SurrealValue> = BTreeMap::new();
            for (key, value) in v {
                entries.insert(key, value);
            }

            let mut object = BTreeMap::new();
            for (key, value) in entries {
                object.insert(CborValue::Text(key), surreal_to_cbor(value)?);
            }
            CborValue::Map(object)
        }
        SurrealValue::Set(v) => CborValue::Array(
            v.into_iter()
                .map(surreal_to_cbor)
                .collect::<Result<Vec<_>>>()?,
        ),
    })
}

fn number_to_cbor(value: SurrealNumber) -> CborValue {
    match value {
        SurrealNumber::Int(v) => CborValue::Integer(v as i128),
        SurrealNumber::Float(v) => CborValue::Float(v),
        SurrealNumber::Decimal(v) => tagged_text(DECIMAL_TAG, v.to_string()),
    }
}

fn record_id_key_to_cbor(value: SurrealRecordIdKey) -> Result<CborValue> {
    Ok(match value {
        SurrealRecordIdKey::Number(v) => CborValue::Integer(v as i128),
        SurrealRecordIdKey::String(v) => CborValue::Text(v),
        SurrealRecordIdKey::Uuid(v) => tagged_text(UUID_TAG, v.to_string()),
        SurrealRecordIdKey::Array(v) => CborValue::Array(
            v.into_iter()
                .map(surreal_to_cbor)
                .collect::<Result<Vec<_>>>()?,
        ),
        SurrealRecordIdKey::Object(v) => {
            let mut entries: BTreeMap<String, SurrealValue> = BTreeMap::new();
            for (key, value) in v {
                entries.insert(key, value);
            }

            let mut object = BTreeMap::new();
            for (key, value) in entries {
                object.insert(CborValue::Text(key), surreal_to_cbor(value)?);
            }
            CborValue::Map(object)
        }
        SurrealRecordIdKey::Range(v) => serde_cbor::value::to_value(v)?,
    })
}

fn tagged_text(tag: &str, value: String) -> CborValue {
    let mut map = BTreeMap::new();
    map.insert(CborValue::Text(tag.to_string()), CborValue::Text(value));
    CborValue::Map(map)
}

pub fn ordered_params(params: Vec<(String, JsonValue)>) -> BTreeMap<String, JsonValue> {
    params.into_iter().collect()
}
