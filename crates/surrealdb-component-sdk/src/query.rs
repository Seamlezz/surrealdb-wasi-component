use std::any::type_name;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::bindings::seamlezz::surrealdb::call;
use crate::result::QueryResultHolder;

pub struct Query<'a> {
    query_str: &'a str,
    params: Vec<(String, Vec<u8>)>,
    bind_error: Option<anyhow::Error>,
}

impl<'a> Query<'a> {
    pub fn bind<T: Serialize>(mut self, key: impl Into<String>, value: T) -> Self {
        if self.bind_error.is_some() {
            return self;
        }

        let key = key.into();
        let serialized = match serde_cbor::to_vec(&value)
            .with_context(|| format!("failed to bind key {key} with type {}", type_name::<T>()))
        {
            Ok(serialized) => serialized,
            Err(error) => {
                self.bind_error = Some(error);
                return self;
            }
        };

        self.params.push((key, serialized));
        self
    }

    pub async fn execute(self) -> Result<QueryResultHolder> {
        if let Some(error) = self.bind_error {
            return Err(error);
        }

        let results = call::query(self.query_str.to_string(), self.params).await;
        Ok(QueryResultHolder::new(results))
    }
}

pub fn query(query_str: &str) -> Query<'_> {
    Query {
        query_str,
        params: Vec::new(),
        bind_error: None,
    }
}
