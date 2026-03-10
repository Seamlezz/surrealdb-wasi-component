use anyhow::{Result, anyhow};
use serde::Deserialize;
use surrealdb_component_sdk::{RecordId, query};

#[derive(Debug, Deserialize)]
struct Person {
    id: RecordId,
    name: String,
    age: i64,
}

pub async fn run() -> Result<()> {
    query("DEFINE TABLE person_basic SCHEMALESS")
        .execute()
        .await?;

    query("CREATE person_basic:demo CONTENT { name: 'demo', age: 42 }")
        .execute()
        .await?;

    let result = query("SELECT * FROM person_basic:demo").execute().await?;

    let Some(row): Option<Person> = result.take(0)? else {
        return Err(anyhow!("expected 1 person_basic row, got 0"));
    };

    assert_eq!(row.id, RecordId::new("person_basic", "demo"));
    assert_eq!(row.name, "demo");
    assert_eq!(row.age, 42);

    Ok(())
}
