# Wasmtime integration example

This example shows how to wire the SurrealDB host adapter into a Wasmtime runtime.

## Prerequisites

1. Start a SurrealDB instance:
   ```bash
   docker run --rm -p 8000:8000 surrealdb/surrealdb:latest start --user root --pass root
   ```

2. Build a guest component that uses the SurrealDB SDK:
   ```bash
   cargo build -p guest-demo --target wasm32-wasip2
   ```

3. Convert the WASM to a component:
   ```bash
   wasm-tools component new target/wasm32-wasip2/debug/guest_demo.wasm -o guest-demo.component.wasm
   ```

## Running the host

```bash
# With defaults (connects to localhost:8000, namespace/db: app)
cargo run -p host-wasmtime -- guest-demo.component.wasm

# With custom connection settings
SURREAL_DB_URL=http://127.0.0.1:8000 \
SURREAL_DB_USER=root \
SURREAL_DB_PASS=root \
SURREAL_DB_NS=app \
SURREAL_DB_NAME=app \
cargo run -p host-wasmtime -- guest-demo.component.wasm
```

## Complete wiring example

```rust
use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use surrealdb_host_adapter::{SurrealHostAdapter, add_surreal_to_linker};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};

async fn run() -> Result<()> {
    let db: Surreal<Any> = Surreal::init();
    db.connect("http://127.0.0.1:8000").await?;
    db.signin(Root {
        username: "root",
        password: "root",
    })
    .await?;
    db.use_ns("app").use_db("app").await?;

    let adapter = SurrealHostAdapter::new(db);

    let mut config = Config::new();
    config
        .wasm_component_model(true)
        .async_support(true);

    let engine = Engine::new(&config)?;
    let component = Component::from_file(&engine, "guest-demo.component.wasm")?;

    let mut linker = Linker::new(&engine);
    add_surreal_to_linker(&mut linker, |state| state)?;

    let mut store = Store::new(&engine, adapter);
    let _instance = linker.instantiate_async(&mut store, &component).await?;

    println!("Component instantiated successfully");

    Ok(())
}
```

## Key wiring points

1. **Create the adapter first**: `SurrealHostAdapter::new(db)` wraps the SurrealDB client.
2. **Build Engine with component model**: `config.wasm_component_model(true).async_support(true)`.
3. **Register bindings with stable adapter API**: `add_surreal_to_linker(linker, |state| state)?` passes the store state directly.
4. **Create Store with adapter**: `Store::new(&engine, adapter)` makes the adapter available to the component.
5. **Use async instantiation**: `instantiate_async` is required for async host imports.

## Guest component requirements

Guest components must import the `seamlezz:surrealdb` world. Make sure your component includes the SurrealDB SDK and calls `query().bind().execute()` to interact with the database.

If your component also imports WASI APIs, add WASI host functions to the linker before instantiation.
