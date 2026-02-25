mod adapter;
pub mod bindings;
mod convert;

pub use adapter::SurrealHostAdapter;

impl bindings::seamlezz::surrealdb::call::Host for SurrealHostAdapter {}
