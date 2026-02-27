# surrealdb-component-sdk

Rust SDK for guest WASI components that need to execute SurrealDB queries through the `seamlezz:surrealdb` WIT interface.

## What this crate provides

1. Query builder entry point with `query("...")`.
2. Typed parameter binding with CBOR serialization.
3. Result extraction helpers for statement based SurrealDB responses.
4. Utility types for SurrealDB style values, including `Datetime` and `RecordId`.

## Installation

Add this crate to your guest component crate.

```toml
[dependencies]
surrealdb-component-sdk = { path = "../../crates/surrealdb-component-sdk" }
serde_json = "1"
anyhow = "1"
```

This crate is configured as both `cdylib` and `rlib`, so it supports component builds and local Rust testing flows.

## Query Flow

```rust
use anyhow::Result;
use surrealdb_component_sdk::query;

async fn run() -> Result<()> {
    let result = query("SELECT * FROM person WHERE id = $id")
        .bind("id", "person:demo")
        .execute()
        .await?;

    let rows: Vec<serde_json::Value> = result.parse(0)?;
    let _ = rows;

    Ok(())
}
```

## Result Handling

`QueryResultHolder` stores one entry per statement.

1. `parse::<T>(index)`: parse successful statement result into `T`.
2. `parse_result::<T>(index)`: preserve statement error as `Result<T, String>`.
3. `take::<T>(index)`: parse via `SingleQueryResultExtractor`.
4. `take_result::<T>(index)`: same as `take`, while preserving statement error.
5. `find_user_error()`: combines user facing errors and ignores transaction cascade noise.

## Binding Behavior

`bind(key, value)` serializes each value to CBOR.

1. Serialization failure is captured once and returned by `execute()`.
2. After a bind failure, additional binds are ignored to preserve the first meaningful error.

## Built in value helpers

1. `Datetime`: wrapper around `chrono::DateTime<Utc>` with flexible deserialization from RFC3339 strings and unix timestamp values.
2. `RecordId`: table plus key model for SurrealDB records.
3. `RecordIdKey`: key variants for numeric, string, uuid like, array, and object forms.

## Contract expectations

The crate calls the async WIT function:

`call.query(query: string, params: list<tuple<string, list<u8>>>) -> list<result<list<u8>, string>>`

Parameter and result payloads are CBOR encoded.

## Related docs

1. Workspace overview: `README.md`
2. Host side adapter: `crates/surrealdb-host-adapter/README.md`
3. Guest example: `examples/guest-demo/src/main.rs`
