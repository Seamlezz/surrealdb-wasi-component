# surrealdb-wasi-component

Build and run SurrealDB powered WASI components with a shared WIT contract, a guest SDK, and a host adapter.

## Overview

This workspace provides the pieces needed to let a WASI component run SurrealDB queries through a host runtime.

1. `crates/surrealdb-component-sdk` gives guest components a Rust API for query execution.
2. `crates/surrealdb-host-adapter` connects the host runtime to SurrealDB.
3. `wit/` defines the `seamlezz:surrealdb@0.2.0` interface contract.
4. `examples/guest-demo` shows the guest side query and live query flow.

## Workspace Layout

1. `crates/surrealdb-component-sdk`: SDK for guest components that target `wasm32-wasip2`.
2. `crates/surrealdb-host-adapter`: host adapter for Wasmtime component linking.
3. `examples/guest-demo`: guest example that runs query and live query operations.
4. `examples/host-wasmtime`: runnable host example for Wasmtime integration.
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
task test:examples
task build:host
task build:sdk
task build:demo
task ci
```

`task test:examples` runs the guest demo through `host-wasmtime` against an in memory SurrealDB engine and fails if query, subscribe, or cancel calls are not observed.

## Typical Usage Paths

### Build a guest component

1. Add `surrealdb-component-sdk` to your guest crate.
2. Use `query("...").bind("key", value).execute().await`.
3. Parse results with `parse`, `parse_result`, `take`, or `take_result`.

See `crates/surrealdb-component-sdk/README.md`.

### Wire a host runtime

1. Create a `Surreal<Any>` database client and connect it.
2. Define host state and implement your generated `seamlezz::surrealdb::call` host traits.
3. Register WASI and generated bindings with `add_to_linker_async` and `Adapter::add_to_linker`.
4. Instantiate the component with `linker.instantiate_async` and create typed bindings with `Adapter::new`.
5. Call exported guest functions through `store.run_concurrent`.

See `crates/surrealdb-host-adapter/README.md` and `examples/host-wasmtime/src/main.rs`.

## Releases

Components are released independently when their own version changes on `main`.

1. `crates/surrealdb-component-sdk/Cargo.toml` version bump releases the SDK crate.
2. `crates/surrealdb-host-adapter/Cargo.toml` version bump releases the host adapter crate.
3. `wit/world.wit` package version bump releases the WIT OCI artifact.

The automation is defined in `.github/workflows/release-components.yml`.

You can also run that workflow manually and select one component to force a release check without relying on a new push event.

WIT package release tasks in `Taskfile.yml` are still available for manual execution.

```bash
task release:dry-run TAG=0.1.0
task release:publish VERSION=0.1.0
```

Manual fallback workflow is available in `.github/workflows/release.yml` via `workflow_dispatch`.

## License

Apache 2.0.
