use anyhow::{Context, Result};
use serde::Deserialize;
use surrealdb_component_sdk::{LiveAction, RecordId, query, subscribe};

#[derive(Debug, Deserialize)]
struct Person {
    id: RecordId,
    name: String,
    age: i64,
}

pub async fn run() -> Result<()> {
    query("DEFINE TABLE person_live SCHEMALESS")
        .execute()
        .await?;

    let mut subscription = subscribe("LIVE SELECT * FROM person_live WHERE id = <record>$id")
        .bind("id", "person_live:demo")
        .execute()
        .await?;

    query("CREATE person_live:demo CONTENT { name: 'demo', age: 42 }")
        .execute()
        .await?;

    let create_event = subscription
        .next_event()
        .await?
        .context("live stream ended before create event")?;
    let create_payload: Person = create_event.parse()?;

    assert_eq!(create_event.action, LiveAction::Create);
    assert_eq!(create_payload.id, RecordId::new("person_live", "demo"));
    assert_eq!(create_payload.name, "demo");
    assert_eq!(create_payload.age, 42);

    query("UPDATE person_live:demo SET age = 43")
        .execute()
        .await?;

    let update_event = subscription
        .next_event()
        .await?
        .context("live stream ended before update event")?;
    let update_payload: Person = update_event.parse()?;

    assert_eq!(update_event.action, LiveAction::Update);
    assert_eq!(update_payload.id, RecordId::new("person_live", "demo"));
    assert_eq!(update_payload.name, "demo");
    assert_eq!(update_payload.age, 43);

    query("DELETE person_live:demo").execute().await?;

    let delete_event = subscription
        .next_event()
        .await?
        .context("live stream ended before delete event")?;
    let delete_payload: Person = delete_event.parse()?;

    assert_eq!(delete_event.action, LiveAction::Delete);
    assert_eq!(delete_payload.id, RecordId::new("person_live", "demo"));
    assert_eq!(delete_payload.name, "demo");
    assert_eq!(delete_payload.age, 43);

    subscription.cancel().await?;

    Ok(())
}
