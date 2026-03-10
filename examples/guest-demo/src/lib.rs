mod bindings;
mod scenarios;

use anyhow::{Context, Result};

pub struct GuestDemo;

impl bindings::exports::seamlezz::surrealdb_host_adapter::demo::Guest for GuestDemo {
    async fn run() -> Result<(), String> {
        run_demo().await.map_err(|error| format!("{error:#}"))
    }
}

pub async fn run_demo() -> Result<()> {
    scenarios::run_all()
        .await
        .context("guest demo scenarios failed")
}
