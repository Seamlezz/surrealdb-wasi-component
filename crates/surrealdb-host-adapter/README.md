# surrealdb-host-adapter

Rust host helper for the `seamlezz:surrealdb` WIT contract. This crate exposes query and live query execution plus conversion logic. Your host runtime provides state, generated bindings, and linker wiring.

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

The host application owns the adapter type and WIT bindings. The adapter `query` and live query helpers are called from generated host trait implementations.

See `examples/host-wasmtime/src/main.rs` for the complete wiring, including:

1. `bindgen!` with `imports: { default: async | store | trappable }`.
2. Linker setup with `wasmtime_wasi::p2::add_to_linker_async` and `bindings::Adapter::add_to_linker`.
3. Raw instance creation with `linker.instantiate_async`.
4. Typed export calls through `store.run_concurrent`.

## Runtime sequence

1. Create and connect a `Surreal<Any>` client.
2. Create host state that stores the client.
3. Generate and register bindings with `bindings::Adapter::add_to_linker`.
4. Create `Store` with host state.
5. Instantiate with `linker.instantiate_async` and construct typed bindings with `bindings::Adapter::new`.
6. Call guest exports through `store.run_concurrent`.

## Run the example host

Build a guest component:

```bash
cargo build -p guest-demo --target wasm32-wasip2
```

Run the example host with memory backed SurrealDB:

```bash
cargo run -p host-wasmtime -- target/wasm32-wasip2/debug/guest_demo.wasm
```

The example host always validates that guest `query`, `subscribe`, and `cancel` calls executed.

## Related docs

1. Workspace overview: `README.md`
2. Guest SDK: `crates/surrealdb-component-sdk/README.md`
3. Full runnable host reference: `examples/host-wasmtime/src/main.rs`
