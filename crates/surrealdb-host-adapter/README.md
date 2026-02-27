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

`SurrealHostAdapter::query` accepts the query string and a list of CBOR encoded parameters, then returns one result per statement as `Result<Vec<u8>, String>`.

## Runtime Wiring Example

```rust
use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use surrealdb_host_adapter::SurrealHostAdapter;
use surrealdb_host_adapter::bindings::seamlezz::surrealdb::call;
use wasmtime::component::Linker;

pub async fn add_surreal_bindings(linker: &mut Linker<SurrealHostAdapter>) -> Result<()> {
    let db: Surreal<Any> = Surreal::init();
    db.connect("http://127.0.0.1:8000").await?;
    db.signin(Root { username: "root", password: "root" }).await?;
    db.use_ns("app").use_db("app").await?;

    call::add_to_linker::<_, SurrealHostAdapter>(linker, |state| state)?;
    Ok(())
}
```

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

## Related docs

1. Workspace overview: `README.md`
2. Guest SDK: `crates/surrealdb-component-sdk/README.md`
3. Host integration note: `docs/wasmtime-example.md`
