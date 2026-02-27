use anyhow::{Context, Result};
use std::env;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use surrealdb_host_adapter::SurrealHostAdapter;
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};

#[tokio::main]
async fn main() -> Result<()> {
    let component_path = env::args()
        .nth(1)
        .context("Usage: host-wasmtime <path-to-component.wasm>")?;

    let db_url = env::var("SURREAL_DB_URL").unwrap_or_else(|_| "http://127.0.0.1:8000".to_string());
    let db_user = env::var("SURREAL_DB_USER").unwrap_or_else(|_| "root".to_string());
    let db_pass = env::var("SURREAL_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let db_ns = env::var("SURREAL_DB_NS").unwrap_or_else(|_| "app".to_string());
    let db_name = env::var("SURREAL_DB_NAME").unwrap_or_else(|_| "app".to_string());

    let db: Surreal<Any> = Surreal::init();
    db.connect(&db_url)
        .await
        .context("failed to connect to SurrealDB")?;
    db.signin(Root {
        username: db_user,
        password: db_pass,
    })
    .await
    .context("failed to sign in to SurrealDB")?;
    db.use_ns(&db_ns)
        .use_db(&db_name)
        .await
        .context("failed to select namespace and database")?;

    let adapter = SurrealHostAdapter::new(db);

    let mut config = Config::new();
    config.wasm_component_model(true);

    let engine = Engine::new(&config)?;

    let component = Component::from_file(&engine, &component_path)?;

    let mut linker = Linker::new(&engine);

    surrealdb_host_adapter::bindings::Adapter::add_to_linker::<_, wasmtime::component::HasSelf<_>>(
        &mut linker,
        |state| state,
    )?;

    let mut store = Store::new(&engine, adapter);
    surrealdb_host_adapter::bindings::Adapter::instantiate_async(&mut store, &component, &linker)
        .await?;

    println!(
        "Successfully loaded and instantiated component: {}",
        component_path
    );
    println!("Connected to SurrealDB at {}", db_url);
    println!("Using namespace: {}, database: {}", db_ns, db_name);

    Ok(())
}
