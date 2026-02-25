# Wasmtime integration example

```rust
use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use surrealdb_host_adapter::SurrealHostAdapter;
use surrealdb_host_adapter::bindings::seamlezz::surrealdb::call;
use wasmtime::component::Linker;

async fn add_surreal_bindings(linker: &mut Linker<SurrealHostAdapter>) -> Result<()> {
    let db: Surreal<Any> = Surreal::init();
    db.connect("http://127.0.0.1:8000").await?;
    db.signin(Root {
        username: "root",
        password: "root",
    })
    .await?;
    db.use_ns("app").use_db("app").await?;

    let _state = SurrealHostAdapter::new(db);

    call::add_to_linker::<_, SurrealHostAdapter>(linker, |state| state)?;

    Ok(())
}
```
