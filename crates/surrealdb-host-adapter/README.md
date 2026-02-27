# surrealdb-host-adapter

Rust host helper for the `seamlezz:surrealdb` WIT contract. This crate exposes query execution and conversion logic. Your host runtime provides state, generated bindings, and linker wiring.

## Core API

```rust
pub async fn query(
    db: &Surreal<Any>,
    query: String,
    params: Vec<(String, Vec<u8>)>,
) -> Result<Vec<Result<Vec<u8>, String>>, QueryError>
```

`query` decodes guest CBOR parameters, executes the SurrealDB statement set, and encodes each statement result back to CBOR.

## QueryError behavior

1. `QueryError::ParamDecode { key, source }` is returned when one bound parameter cannot be decoded from CBOR.
2. `QueryError::QueryExecution(source)` is returned when SurrealDB fails to execute the statement set.
3. Statement extraction and serialization issues remain per statement `Err(String)` entries in the returned vector.

## Wasmtime wiring pattern

The host application owns the adapter type and WIT bindings. The `query` function is called from your generated `call::Host` implementation.

```rust
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tokio::sync::RwLock;

mod bindings {
    wasmtime::component::bindgen!({
        path: "wit",
        world: "adapter",
        imports: { default: async | trappable },
        exports: { default: async },
    });
}

#[derive(Clone)]
pub struct HostState {
    db: Arc<RwLock<Surreal<Any>>>,
}

impl bindings::seamlezz::surrealdb::call::Host for HostState {
    async fn query(
        &mut self,
        query: String,
        params: Vec<(String, Vec<u8>)>,
    ) -> wasmtime::Result<Vec<Result<Vec<u8>, String>>> {
        let db = self.db.read().await;
        surrealdb_host_adapter::query(&db, query, params)
            .await
            .map_err(wasmtime::Error::new)
    }
}
```

## Runtime sequence

1. Create and authenticate a `Surreal<Any>` client.
2. Create host state that stores the client.
3. Generate and register bindings with `bindings::Adapter::add_to_linker`.
4. Create `Store` with host state.
5. Instantiate the component with `bindings::Adapter::instantiate_async`.

## Run the example host

Start SurrealDB:

```bash
docker run --rm -p 8000:8000 surrealdb/surrealdb:latest start --user root --pass root
```

Build a guest component:

```bash
cargo build -p guest-demo --target wasm32-wasip2
wasm-tools component new target/wasm32-wasip2/debug/guest_demo.wasm -o guest-demo.component.wasm
```

Run with defaults:

```bash
cargo run -p host-wasmtime -- guest-demo.component.wasm
```

Run with custom connection settings:

```bash
SURREAL_DB_URL=http://127.0.0.1:8000 \
SURREAL_DB_USER=root \
SURREAL_DB_PASS=root \
SURREAL_DB_NS=app \
SURREAL_DB_NAME=app \
cargo run -p host-wasmtime -- guest-demo.component.wasm
```

## Related docs

1. Workspace overview: `README.md`
2. Guest SDK: `crates/surrealdb-component-sdk/README.md`
3. Full runnable host reference: `examples/host-wasmtime/src/main.rs`
