use anyhow::Result;
use surrealdb_component_sdk::query;

fn main() {}

pub async fn run_demo() -> Result<()> {
    let result = query("SELECT * FROM person WHERE id = $id")
        .bind("id", "person:demo")
        .execute()
        .await?;

    let _rows: Vec<serde_json::Value> = result.parse(0)?;
    Ok(())
}
