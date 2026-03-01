# SurrealDB WIT Package

This directory defines the `seamlezz:surrealdb@0.2.0` package.

The package exposes one async interface, `call`, with three functions:

1. `query` for statement execution.
2. `subscribe` for live query streams.
3. `cancel` to stop an active subscription.

Parameters and live event payloads are CBOR encoded.

Publish with Taskfile targets from repository root.
