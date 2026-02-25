# Wasmtime integration example

```rust
use anyhow::Result;
use surrealdb_host_adapter::{AdapterState, SurrealHostAdapter, SurrealHostConfig};
use surrealdb_host_adapter::bindings::seamlezz::surrealdb::call;
use wasmtime::component::Linker;

async fn add_surreal_bindings(linker: &mut Linker<AdapterState>) -> Result<()> {
    let config = SurrealHostConfig {
        url: "http://127.0.0.1:8000".to_string(),
        namespace: "app".to_string(),
        database: "app".to_string(),
        auth: Default::default(),
    };

    let adapter = SurrealHostAdapter::connect(config).await?;
    let _state = AdapterState::new(adapter);

    call::add_to_linker::<_, AdapterState>(linker, |state| state)?;

    Ok(())
}
```
