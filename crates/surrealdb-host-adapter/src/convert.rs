use std::collections::BTreeMap;
use std::str::FromStr;

use anyhow::{Result, bail};
use rust_decimal::Decimal as RustDecimal;
use serde_cbor::Value as CborValue;
use surrealdb_types::{
    Array as SurrealArray, Bytes as SurrealBytes, Duration as SurrealDuration,
    Number as SurrealNumber, Object as SurrealObject, RecordId as SurrealRecordId,
    RecordIdKey as SurrealRecordIdKey, Regex as SurrealRegex, SurrealValue as _,
    Uuid as SurrealUuid, Value as SurrealValue,
};

const UUID_TAG: &str = "$surrealdb::uuid";
const DURATION_TAG: &str = "$surrealdb::duration";
const DECIMAL_TAG: &str = "$surrealdb::decimal";
const REGEX_TAG: &str = "$surrealdb::regex";

pub fn cbor_slice_to_surreal(bytes: &[u8]) -> Result<SurrealValue> {
    let value: CborValue = serde_cbor::from_slice(bytes)?;
    cbor_to_surreal(value)
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

fn cbor_to_surreal(value: CborValue) -> Result<SurrealValue> {
    Ok(match value {
        CborValue::Null => SurrealValue::Null,
        CborValue::Bool(value) => SurrealValue::Bool(value),
        CborValue::Integer(value) => {
            if value < i64::MIN as i128 || value > i64::MAX as i128 {
                bail!("unsupported integer representation")
            }
            SurrealValue::Number(SurrealNumber::Int(value as i64))
        }
        CborValue::Float(value) => SurrealValue::Number(SurrealNumber::Float(value)),
        CborValue::Bytes(value) => SurrealValue::Bytes(SurrealBytes::from(value)),
        CborValue::Text(value) => SurrealValue::String(value),
        CborValue::Array(values) => SurrealValue::Array(
            values
                .into_iter()
                .map(cbor_to_surreal)
                .collect::<Result<SurrealArray>>()?,
        ),
        CborValue::Map(values) => map_to_surreal(values)?,
        CborValue::Tag(_, value) => cbor_to_surreal(*value)?,
        _ => bail!("unsupported cbor value"),
    })
}

fn map_to_surreal(values: BTreeMap<CborValue, CborValue>) -> Result<SurrealValue> {
    if values.len() == 1 {
        let (key, value) = values.into_iter().next().expect("single entry map");
        if let CborValue::Text(tag) = key {
            return tagged_scalar_to_surreal(tag, value);
        }
        let mut object = SurrealObject::new();
        object.insert(map_key_to_string(key)?, cbor_to_surreal(value)?);
        return Ok(SurrealValue::Object(object));
    }

    let mut object = SurrealObject::new();
    for (key, value) in values {
        object.insert(map_key_to_string(key)?, cbor_to_surreal(value)?);
    }

    if let (Some(SurrealValue::String(table)), Some(key)) = (object.get("table"), object.get("key"))
    {
        if let Ok(key) = SurrealRecordIdKey::from_value(key.clone()) {
            return Ok(SurrealValue::RecordId(SurrealRecordId::new(
                table.clone(),
                key,
            )));
        }
    }

    Ok(SurrealValue::Object(object))
}

fn tagged_scalar_to_surreal(tag: String, value: CborValue) -> Result<SurrealValue> {
    let CborValue::Text(value) = value else {
        bail!("invalid tagged scalar value")
    };

    Ok(match tag.as_str() {
        UUID_TAG => SurrealValue::Uuid(SurrealUuid::try_from(value)?),
        DURATION_TAG => SurrealValue::Duration(SurrealDuration::from_str(&value)?),
        DECIMAL_TAG => SurrealValue::Number(SurrealNumber::Decimal(RustDecimal::from_str(&value)?)),
        REGEX_TAG => SurrealValue::Regex(SurrealRegex::from_str(&value)?),
        _ => {
            let mut object = SurrealObject::new();
            object.insert(tag, value);
            SurrealValue::Object(object)
        }
    })
}

fn map_key_to_string(key: CborValue) -> Result<String> {
    match key {
        CborValue::Text(text) => Ok(text),
        CborValue::Integer(integer) => Ok(integer.to_string()),
        _ => bail!("unsupported map key type"),
    }
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

pub fn ordered_params(params: Vec<(String, SurrealValue)>) -> BTreeMap<String, SurrealValue> {
    params.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use surrealdb_types::{RecordId, RecordIdKey, Value};

    use super::cbor_slice_to_surreal;

    #[test]
    fn decodes_tagged_scalars_into_native_values() {
        let bytes = serde_cbor::to_vec(
            &json!({"$surrealdb::uuid": "018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d"}),
        )
        .unwrap();
        let value = cbor_slice_to_surreal(&bytes).unwrap();

        match value {
            Value::Uuid(uuid) => {
                assert_eq!(uuid.to_string(), "018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d");
            }
            other => panic!("expected uuid, got {other:?}"),
        }
    }

    #[test]
    fn decodes_tagged_record_id_keys_into_native_values() {
        let bytes = serde_cbor::to_vec(&json!({
            "table": "person",
            "key": {"$surrealdb::uuid": "018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d"}
        }))
        .unwrap();
        let value = cbor_slice_to_surreal(&bytes).unwrap();

        match value {
            Value::RecordId(RecordId { table, key }) => {
                assert_eq!(table.to_string(), "person");
                assert!(matches!(key, RecordIdKey::Uuid(_)));
            }
            other => panic!("expected record id, got {other:?}"),
        }
    }
}
