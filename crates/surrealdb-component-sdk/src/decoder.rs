use std::any::type_name;

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_cbor::Value as CborValue;
use serde_json::{Value, json};

const MAX_DEPTH: usize = 6;
const MAX_MAP_FIELDS: usize = 12;
const MAX_ARRAY_INSPECTION: usize = 16;
const MAX_ARRAY_SHAPES: usize = 4;
const MAX_KEY_LENGTH: usize = 64;
const MAX_DIAGNOSTIC_INPUT_LENGTH: usize = 262_144;
const MAX_DIAGNOSTIC_LENGTH: usize = 8_192;

pub(crate) fn decode<D: DeserializeOwned>(bytes: &[u8], operation: &str) -> Result<D> {
    match serde_cbor::from_slice::<D>(bytes) {
        Ok(value) => Ok(value),
        Err(error) => {
            let diagnostic = diagnostic(bytes);
            Err(anyhow::anyhow!(
                "{operation} into type {}; CBOR decode error category: {:?}, byte offset: {}; CBOR shape: {diagnostic}",
                type_name::<D>(),
                error.classify(),
                error.offset(),
            ))
        }
    }
}

fn diagnostic(bytes: &[u8]) -> String {
    if bytes.len() > MAX_DIAGNOSTIC_INPUT_LENGTH {
        return json!({
            "cbor": "not_inspected",
            "reason": "payload_too_large",
            "byte_length": bytes.len(),
            "maximum_inspected_byte_length": MAX_DIAGNOSTIC_INPUT_LENGTH,
        })
        .to_string();
    }

    let value = match serde_cbor::from_slice::<CborValue>(bytes) {
        Ok(value) => value,
        Err(_) => {
            return json!({
                "cbor": "invalid",
                "byte_length": bytes.len(),
            })
            .to_string();
        }
    };

    let diagnostic = shape(&value, 0).to_string();
    if diagnostic.len() <= MAX_DIAGNOSTIC_LENGTH {
        return diagnostic;
    }

    json!({
        "type": "diagnostic",
        "truncated": true,
        "byte_length": bytes.len(),
    })
    .to_string()
}

fn shape(value: &CborValue, depth: usize) -> Value {
    if depth >= MAX_DEPTH {
        return json!({ "type": type_name_for(value), "truncated": "maximum_depth" });
    }

    match value {
        CborValue::Null => json!({ "type": "null" }),
        CborValue::Bool(_) => json!({ "type": "boolean" }),
        CborValue::Integer(_) => json!({ "type": "integer" }),
        CborValue::Float(_) => json!({ "type": "float" }),
        CborValue::Bytes(bytes) => json!({ "type": "bytes", "length": bytes.len() }),
        CborValue::Text(_) => json!({ "type": "string" }),
        CborValue::Array(items) => array_shape(items, depth),
        CborValue::Map(fields) => {
            let mut shaped_fields = Vec::new();
            for (key, value) in fields.iter().take(MAX_MAP_FIELDS) {
                let mut field = match key {
                    CborValue::Text(key) => {
                        let (key, truncated) = bounded_key(key);
                        json!({ "key": key, "shape": shape(value, depth + 1), "key_truncated": truncated })
                    }
                    key => json!({
                        "key_type": type_name_for(key),
                        "shape": shape(value, depth + 1),
                    }),
                };
                if field["key_truncated"] == Value::Bool(false) {
                    field.as_object_mut().unwrap().remove("key_truncated");
                }
                shaped_fields.push(field);
            }
            json!({
                "type": "map",
                "length": fields.len(),
                "fields": shaped_fields,
                "truncated": (fields.len() > MAX_MAP_FIELDS).then_some("map_fields"),
            })
        }
        CborValue::Tag(tag, value) => json!({
            "type": "tag",
            "tag": tag,
            "shape": shape(value, depth + 1),
        }),
        CborValue::__Hidden => json!({ "type": "unknown" }),
    }
}

fn array_shape(items: &[CborValue], depth: usize) -> Value {
    let mut shapes = Vec::new();
    for item in items.iter().take(MAX_ARRAY_INSPECTION) {
        let item_shape = shape(item, depth + 1);
        if !shapes.contains(&item_shape) {
            shapes.push(item_shape);
        }
        if shapes.len() == MAX_ARRAY_SHAPES {
            break;
        }
    }

    let truncated = if items.len() > MAX_ARRAY_INSPECTION {
        Some("array_inspection")
    } else if shapes.len() == MAX_ARRAY_SHAPES
        && items
            .iter()
            .take(MAX_ARRAY_INSPECTION)
            .any(|item| !shapes.contains(&shape(item, depth + 1)))
    {
        Some("distinct_shapes")
    } else {
        None
    };

    json!({
        "type": "array",
        "length": items.len(),
        "item_shapes": shapes,
        "truncated": truncated,
    })
}

fn bounded_key(key: &str) -> (String, bool) {
    let mut chars = key.chars();
    let bounded: String = chars.by_ref().take(MAX_KEY_LENGTH).collect();
    (bounded, chars.next().is_some())
}

fn type_name_for(value: &CborValue) -> &'static str {
    match value {
        CborValue::Null => "null",
        CborValue::Bool(_) => "boolean",
        CborValue::Integer(_) => "integer",
        CborValue::Float(_) => "float",
        CborValue::Bytes(_) => "bytes",
        CborValue::Text(_) => "string",
        CborValue::Array(_) => "array",
        CborValue::Map(_) => "map",
        CborValue::Tag(_, _) => "tag",
        CborValue::__Hidden => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_cbor::Value as CborValue;

    use super::*;

    #[derive(Debug, Deserialize, Serialize)]
    struct Expected {
        expected: String,
    }

    fn failed_diagnostic(value: &CborValue) -> String {
        failed_diagnostic_as::<Expected>(value)
    }

    fn failed_diagnostic_as<D: DeserializeOwned>(value: &CborValue) -> String {
        let bytes = serde_cbor::to_vec(value).unwrap();
        match decode::<D>(&bytes, "failed to decode test") {
            Ok(_) => panic!("expected decoding to fail"),
            Err(error) => format!("{error:#}"),
        }
    }

    #[test]
    fn describes_nested_shapes_without_scalar_values() {
        let value = CborValue::Map(
            [(
                CborValue::Text("users".into()),
                CborValue::Array(vec![CborValue::Map(
                    [
                        (
                            CborValue::Text("name".into()),
                            CborValue::Text("secret".into()),
                        ),
                        (CborValue::Text("active".into()), CborValue::Bool(true)),
                        (CborValue::Text("score".into()), CborValue::Integer(42)),
                        (
                            CborValue::Text("blob".into()),
                            CborValue::Bytes(vec![1, 2, 3]),
                        ),
                    ]
                    .into(),
                )]),
            )]
            .into(),
        );

        let error = failed_diagnostic(&value);
        assert!(error.contains("users"));
        assert!(error.contains("name"));
        assert!(error.contains("\"length\":3"));
        assert!(error.contains("item_shapes"));
        assert!(!error.contains("secret"));
        assert!(!error.contains("42"));
        assert!(!error.contains("true"));
    }

    #[test]
    fn reports_nontext_key_types_without_key_values() {
        let value = CborValue::Map([(CborValue::Integer(987654321), CborValue::Null)].into());
        let error = failed_diagnostic(&value);
        assert!(error.contains("\"key_type\":\"integer\""));
        assert!(!error.contains("987654321"));
    }

    #[test]
    fn excludes_values_from_typed_decode_errors() {
        let cases = [
            CborValue::Integer(987654321),
            CborValue::Text("private-value".into()),
            CborValue::Bool(true),
            CborValue::Float(12345.6789),
        ];

        for value in cases {
            let error = failed_diagnostic_as::<Vec<String>>(&value);
            assert!(!error.contains("987654321"));
            assert!(!error.contains("private-value"));
            assert!(!error.contains("true"));
            assert!(!error.contains("12345.6789"));
        }
    }

    #[test]
    fn skips_shape_inspection_for_large_payloads() {
        let bytes = vec![0; MAX_DIAGNOSTIC_INPUT_LENGTH + 1];
        let text = diagnostic(&bytes);
        assert!(text.contains("payload_too_large"));
        assert!(text.contains(&bytes.len().to_string()));
        assert!(text.len() <= MAX_DIAGNOSTIC_LENGTH);
    }

    #[test]
    fn makes_limits_explicit_and_bounds_output() {
        let fields = (0..30)
            .map(|index| {
                (
                    CborValue::Text(format!("{}-{index}", "x".repeat(100))),
                    CborValue::Null,
                )
            })
            .collect();
        let text = diagnostic(&serde_cbor::to_vec(&CborValue::Map(fields)).unwrap());
        assert!(text.len() <= MAX_DIAGNOSTIC_LENGTH);
        assert!(text.contains("map_fields"));
        assert!(text.contains("key_truncated"));
        assert!(!text.contains(&"x".repeat(65)));
    }

    #[test]
    fn marks_array_inspection_and_depth_limits() {
        let array = CborValue::Array(vec![CborValue::Null; MAX_ARRAY_INSPECTION + 1]);
        assert!(shape(&array, 0).to_string().contains("array_inspection"));

        let mut nested = CborValue::Null;
        for _ in 0..=MAX_DEPTH {
            nested = CborValue::Array(vec![nested]);
        }
        assert!(shape(&nested, 0).to_string().contains("maximum_depth"));
    }

    #[test]
    fn invalid_cbor_reports_only_validity_and_length() {
        let text = diagnostic(&[0xff]);
        assert_eq!(text, r#"{"byte_length":1,"cbor":"invalid"}"#);
    }

    #[test]
    fn successful_decode_has_no_diagnostic_path() {
        let bytes = serde_cbor::to_vec(&Expected {
            expected: "ok".into(),
        })
        .unwrap();
        let value: Expected = decode(&bytes, "failed to decode test").unwrap();
        assert_eq!(value.expected, "ok");
    }
}
