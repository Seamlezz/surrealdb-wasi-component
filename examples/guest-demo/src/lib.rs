mod bindings;

use anyhow::{Context, Result};
use surrealdb_component_sdk::{query, subscribe};

pub struct GuestDemo;

impl bindings::exports::seamlezz::surrealdb_host_adapter::demo::Guest for GuestDemo {
    async fn run() -> Result<(), String> {
        run_demo().await.map_err(|error| format!("{error:#}"))
    }
}

pub async fn run_demo() -> Result<()> {
    query("DEFINE TABLE person SCHEMALESS").execute().await?;

    let mut subscription = subscribe("LIVE SELECT * FROM person WHERE id = <record>$id")
        .bind("id", "person:demo")
        .execute()
        .await?;

    query("CREATE person:demo CONTENT { id: person:demo, name: 'demo', age: 42 }")
        .execute()
        .await?;

    let create_event = subscription
        .next_event()
        .await?
        .context("live stream ended before create event")?;

    let create_payload: serde_json::Value = create_event.parse()?;
    assert_eq!(
        create_event.action,
        surrealdb_component_sdk::LiveAction::Create
    );
    assert_person_id(&create_payload["id"]);
    assert_eq!(create_payload["name"], "demo");
    assert_eq!(create_payload["age"], 42);

    query("UPDATE person:demo SET age = 43").execute().await?;

    let update_event = subscription
        .next_event()
        .await?
        .context("live stream ended before update event")?;

    let update_payload: serde_json::Value = update_event.parse()?;
    assert_eq!(
        update_event.action,
        surrealdb_component_sdk::LiveAction::Update
    );
    assert_person_id(&update_payload["id"]);
    assert_eq!(update_payload["name"], "demo");
    assert_eq!(update_payload["age"], 43);

    let result = query("SELECT * FROM person WHERE id = <record>$id")
        .bind("id", "person:demo")
        .execute()
        .await?;

    let Some(row): Option<serde_json::Value> = result.take(0)? else {
        return Err(anyhow::anyhow!("expected 1 row, got 0"));
    };
    assert_person_id(&row["id"]);
    assert_eq!(row["name"], "demo");
    assert_eq!(row["age"], 43);

    query("DELETE person:demo").execute().await?;

    let delete_event = subscription
        .next_event()
        .await?
        .context("live stream ended before delete event")?;

    let delete_payload: serde_json::Value = delete_event.parse()?;
    assert_eq!(
        delete_event.action,
        surrealdb_component_sdk::LiveAction::Delete
    );
    assert_person_id(&delete_payload["id"]);

    subscription.cancel().await?;

    Ok(())
}

fn assert_person_id(value: &serde_json::Value) {
    assert_eq!(value["table"], "person");
    assert_eq!(value["key"], "demo");
}
