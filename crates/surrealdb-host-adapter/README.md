# surrealdb-host-adapter

Rust host adapter that implements the `seamlezz:surrealdb` WIT host side and forwards guest component queries to SurrealDB.

## What this crate does

1. Exposes `SurrealHostAdapter` for runtime state.
2. Implements generated WIT host bindings for `call.query`.
3. Decodes guest CBOR parameters into JSON values for SurrealDB.
4. Executes statement sets against SurrealDB.
5. Encodes each statement result back to CBOR for guest consumption.

## Core API

```rust
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb_host_adapter::SurrealHostAdapter;

let db: Surreal<Any> = Surreal::init();
let adapter = SurrealHostAdapter::new(db);
```

`SurrealHostAdapter` is the host state type used by the generated bindings. Query execution is provided through the generated `Host` trait implementation, not through a public inherent `query` method.

## Runtime Wiring Example

```rust
use anyhow::Result;
use std::env;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use surrealdb_host_adapter::SurrealHostAdapter;
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};

async fn run() -> Result<()> {
    let component_path = env::args().nth(1).expect("component path");

    let db: Surreal<Any> = Surreal::init();
    db.connect("http://127.0.0.1:8000").await?;
    db.signin(Root { username: "root".to_string(), password: "root".to_string() }).await?;
    db.use_ns("app").use_db("app").await?;

    let adapter = SurrealHostAdapter::new(db);

    let mut config = Config::new();
    config.wasm_component_model(true);

    let engine = Engine::new(&config)?;
    let component = Component::from_file(&engine, component_path)?;

    let mut linker = Linker::new(&engine);
    surrealdb_host_adapter::bindings::Adapter::add_to_linker::<_, wasmtime::component::HasSelf<_>>(
        &mut linker,
        |state| state,
    )?;

    let mut store = Store::new(&engine, adapter);
    surrealdb_host_adapter::bindings::Adapter::instantiate_async(&mut store, &component, &linker)
        .await?;

    Ok(())
}
```

## Wiring sequence

1. **Create and configure SurrealDB client**: Connect, authenticate, select namespace/database.
2. **Wrap client in adapter**: `SurrealHostAdapter::new(db)`.
3. **Configure Engine**: Enable component model.
4. **Build Linker**: Create `Linker::new(&engine)`.
5. **Register bindings**: Call `bindings::Adapter::add_to_linker(&mut linker, |state| state)?`.
6. **Create Store with adapter**: `Store::new(&engine, adapter)` passes the adapter as store state.
7. **Instantiate component**: Call `bindings::Adapter::instantiate_async(&mut store, &component, &linker).await?`.

## Data Conversion Model

1. Guest parameters arrive as CBOR bytes.
2. Adapter converts CBOR values to JSON values.
3. Adapter binds ordered parameters into SurrealDB query execution.
4. Adapter takes each SurrealDB statement result and serializes it to CBOR bytes.
5. Statement errors are returned as `String` values without CBOR payload.

## Error behavior

1. Parameter decode failures return contextual errors with the offending key.
2. SurrealDB execution failures bubble up as adapter errors.
3. Per statement extraction errors are captured per index and returned in the statement result list.

## Runnable example

See `examples/host-wasmtime` for a complete CLI example that accepts component path and environment variables for DB connection.

## Related docs

1. Workspace overview: `README.md`
2. Guest SDK: `crates/surrealdb-component-sdk/README.md`
3. Host integration note: `docs/wasmtime-example.md`
