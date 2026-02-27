# surrealdb-wasi-component

Build and run SurrealDB powered WASI components with a shared WIT contract, a guest SDK, and a host adapter.

## Overview

This workspace provides the pieces needed to let a WASI component run SurrealDB queries through a host runtime.

1. `crates/surrealdb-component-sdk` gives guest components a Rust API for query execution.
2. `crates/surrealdb-host-adapter` connects the host runtime to a configured SurrealDB instance.
3. `wit/` defines the `seamlezz:surrealdb@0.1.0` interface contract.
4. `examples/guest-demo` shows the guest side query flow.

## Workspace Layout

1. `crates/surrealdb-component-sdk`: SDK for guest components that target `wasm32-wasip2`.
2. `crates/surrealdb-host-adapter`: host adapter for Wasmtime component linking.
3. `examples/guest-demo`: minimal guest example that calls `query().bind().execute()`.
4. `docs/wasmtime-example.md`: host integration snippet with linker setup.
5. `wit/README.md`: package information for the WIT contract.

## Architecture

1. Guest code builds a query with the SDK and serializes parameters as CBOR.
2. The host adapter receives the call via generated WIT bindings.
3. The adapter converts CBOR parameters into JSON values for SurrealDB.
4. SurrealDB executes the statement set.
5. The adapter serializes each statement result back to CBOR and returns per statement success or error values.

## Quickstart

### Prerequisites

1. Rust toolchain with `wasm32-wasip2` target installed.
2. `task` command available for workflow commands.
3. `wkg` command available for WIT fetch and release packaging.

### Common Commands

```bash
task wit:fetch
task fmt
task lint
task test
task build:host
task build:sdk
task build:demo
task ci
```

## Typical Usage Paths

### Build a guest component

1. Add `surrealdb-component-sdk` to your guest crate.
2. Use `query("...").bind("key", value).execute().await`.
3. Parse results with `parse`, `parse_result`, `take`, or `take_result`.

See `crates/surrealdb-component-sdk/README.md`.

### Wire a host runtime

1. Create and authenticate a `Surreal<Any>` database client.
2. Construct `SurrealHostAdapter::new(db)`.
3. Register bindings with `call::add_to_linker`.

See `crates/surrealdb-host-adapter/README.md` and `docs/wasmtime-example.md`.

## Releases

WIT package release tasks are defined in `Taskfile.yml`.

```bash
task release:dry-run TAG=0.1.0
task release:publish VERSION=0.1.0
```

The release workflow also publishes crate releases and WIT artifacts through repository automation.

## License

Apache 2.0.
