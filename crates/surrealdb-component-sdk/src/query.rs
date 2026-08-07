use std::any::type_name;

use serde::Serialize;

use crate::{
    ConflictRetryPolicy, QueryError, QueryResponse, StatementError,
    bindings::current_parent_context, bindings::seamlezz::surrealdb::call,
    retry::execute_with_conflict_retry,
};

/// SurrealDB query under construction.
pub struct Query<'a> {
    query_str: &'a str,
    params: Vec<(String, Vec<u8>)>,
    bind_error: Option<QueryError>,
    conflict_retry_policy: Option<ConflictRetryPolicy>,
}

impl<'a> Query<'a> {
    /// Binds one typed parameter using CBOR transport encoding.
    pub fn bind<T: Serialize>(mut self, key: impl Into<String>, value: T) -> Self {
        if self.bind_error.is_some() {
            return self;
        }

        let key = key.into();
        match serde_cbor::to_vec(&value) {
            Ok(serialized) => self.params.push((key, serialized)),
            Err(source) => {
                self.bind_error = Some(QueryError::Bind {
                    key,
                    value_type: type_name::<T>(),
                    source,
                });
            }
        }

        self
    }

    /// Enables conflict retries for an atomic or idempotent query.
    pub fn retry_conflicts(mut self, policy: ConflictRetryPolicy) -> Self {
        self.conflict_retry_policy = Some(policy);
        self
    }

    /// Executes the query and returns all typed statement results.
    pub async fn execute(self) -> Result<QueryResponse, QueryError> {
        if let Some(error) = self.bind_error {
            return Err(error);
        }

        let query = self.query_str.to_string();
        let params = self.params;
        let (mut response, attempts) = execute_with_conflict_retry(
            self.conflict_retry_policy.as_ref(),
            || {
                let query = query.clone();
                let params = params.clone();
                async move {
                    let results = call::query(current_parent_context(), query, params)
                        .await
                        .map_err(|failure| QueryError::Execution {
                            message: failure.message,
                        })?
                        .into_iter()
                        .map(|result| result.map_err(StatementError::from))
                        .collect();
                    Ok(QueryResponse::new(results))
                }
            },
            |duration| async move {
                wstd::time::Timer::after(duration.into()).wait().await;
            },
            QueryResponse::retryable_conflict,
        )
        .await?;
        response.set_attempts(attempts);
        Ok(response)
    }
}

/// Starts constructing a SurrealDB query.
pub fn query(query_str: &str) -> Query<'_> {
    Query {
        query_str,
        params: Vec::new(),
        bind_error: None,
        conflict_retry_policy: None,
    }
}
