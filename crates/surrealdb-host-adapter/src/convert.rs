use std::collections::BTreeMap;

use anyhow::{Result, bail};
use serde_cbor::Value as CborValue;
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use surrealdb::Value as SurrealValue;

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
    let mut bytes = Vec::new();
    ciborium::into_writer(&value, &mut bytes)?;
    Ok(bytes)
}

pub fn ordered_params(params: Vec<(String, JsonValue)>) -> BTreeMap<String, JsonValue> {
    params.into_iter().collect()
}
