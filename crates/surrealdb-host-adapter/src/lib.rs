mod adapter;
pub mod bindings;
mod config;
mod convert;

pub use adapter::SurrealHostAdapter;
pub use config::{Auth, SurrealHostConfig};

pub struct AdapterState {
    pub adapter: SurrealHostAdapter,
}

impl AdapterState {
    pub fn new(adapter: SurrealHostAdapter) -> Self {
        Self { adapter }
    }
}

impl bindings::seamlezz::surrealdb::call::Host for AdapterState {}
