mod bindings;

use anyhow::Result;
use surrealdb_component_sdk::{query, subscribe};

pub struct GuestDemo;

impl bindings::exports::seamlezz::surrealdb_host_adapter::demo::Guest for GuestDemo {
    async fn run() -> Result<(), String> {
        run_demo().await.map_err(|error| format!("{error:#}"))
    }
}

pub async fn run_demo() -> Result<()> {
    query("DEFINE TABLE person SCHEMALESS").execute().await?;

    let subscription = subscribe("LIVE SELECT * FROM person WHERE id = $id")
        .bind("id", "person:demo")
        .execute()
        .await?;

    query("CREATE person:demo CONTENT { id: person:demo, name: 'demo', age: 42 }")
        .execute()
        .await?;

    query("UPDATE person:demo SET age = 43").execute().await?;

    query("DELETE person:demo").execute().await?;

    subscription.cancel().await?;

    let result = query("SELECT * FROM person WHERE id = $id")
        .bind("id", "person:demo")
        .execute()
        .await?;

    let _row: serde_json::Value = result.parse(0)?;
    Ok(())
}
