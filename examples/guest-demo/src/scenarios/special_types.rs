use anyhow::{Context, Result, anyhow, ensure};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use surrealdb_component_sdk::{Bytes, Datetime, Decimal, Geometry, RecordId, Regex, Uuid, query};

type SurrealDuration = surrealdb_component_sdk::Duration;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SpecialTypesDocument {
    uuid: Uuid,
    decimal: Decimal,
    duration: SurrealDuration,
    regex: Regex,
    seen_at: Datetime,
    payload: Bytes,
    location: Geometry,
    string_record: RecordId,
    number_record: RecordId,
    uuid_record: RecordId,
    object_record: RecordId,
}

#[derive(Debug, Deserialize)]
struct StoredSpecialTypesDocument {
    id: RecordId,
    #[serde(flatten)]
    document: SpecialTypesDocument,
}

pub async fn run() -> Result<()> {
    query("DEFINE TABLE special_types SCHEMALESS")
        .execute()
        .await?;

    let expected = expected_document()?;

    query("CREATE special_types:demo CONTENT $document")
        .bind("document", &expected)
        .execute()
        .await?;

    let result = query("SELECT * FROM special_types:demo").execute().await?;

    let Some(row): Option<StoredSpecialTypesDocument> = result.take(0)? else {
        return Err(anyhow!("expected 1 special_types row, got 0"));
    };

    assert_eq!(row.id, RecordId::new("special_types", "demo"));
    assert_eq!(row.document, expected);
    ensure!(
        row.document.payload.as_ref()[1] == 0,
        "expected binary payload to survive round trip"
    );

    Ok(())
}

fn expected_document() -> Result<SpecialTypesDocument> {
    let seen_at = Utc
        .with_ymd_and_hms(2024, 3, 15, 12, 34, 56)
        .single()
        .context("failed to build test datetime")?;

    Ok(SpecialTypesDocument {
        uuid: Uuid::from("018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d"),
        decimal: Decimal::from("12.34"),
        duration: SurrealDuration::from("1h30m"),
        regex: Regex::from("^[a-z]+$"),
        seen_at: Datetime::from(seen_at),
        payload: Bytes::from(vec![1, 0, 255, 42]),
        location: Geometry::from(json!({
            "type": "Point",
            "coordinates": [4.895168, 52.370216]
        })),
        string_record: RecordId::new("person", "demo"),
        number_record: parse_record_id(json!({
            "table": "person",
            "key": 42,
        }))?,
        uuid_record: parse_record_id(json!({
            "table": "person",
            "key": {
                "$surrealdb::uuid": "018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d"
            },
        }))?,
        object_record: parse_record_id(json!({
            "table": "person",
            "key": {
                "region": "eu",
                "tenant": "demo",
                "active": true,
            },
        }))?,
    })
}

fn parse_record_id(value: serde_json::Value) -> Result<RecordId> {
    let record_id: RecordId = serde_json::from_value(value)?;
    ensure!(
        matches!(
            record_id.key,
            surrealdb_component_sdk::RecordIdKey::String(_)
                | surrealdb_component_sdk::RecordIdKey::Number(_)
                | surrealdb_component_sdk::RecordIdKey::Uuid(_)
                | surrealdb_component_sdk::RecordIdKey::Object(_)
        ),
        "unexpected record id key type"
    );
    Ok(record_id)
}
