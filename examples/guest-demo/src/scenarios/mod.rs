pub mod basic_query;
pub mod live_query;
pub mod special_types;

use anyhow::Result;

pub async fn run_all() -> Result<()> {
    basic_query::run().await?;
    live_query::run().await?;
    special_types::run().await?;
    Ok(())
}
