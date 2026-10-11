use std::num::NonZeroU32;

use thiserror::Error;

/// Failure returned by an individual SurrealDB statement.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StatementError {
    #[error("domain query rejection: {0}")]
    Domain(String),
    #[error("retryable transaction conflict: {0}")]
    RetryableConflict(String),
    #[error("query statement was not executed: {0}")]
    NotExecuted(String),
    #[error("query statement failed: {0}")]
    Failure(String),
}

/// Failure that prevents a query from producing a usable response.
#[derive(Debug, Error)]
pub enum QueryError {
    #[error("failed to bind key {key} with type {value_type}")]
    Bind {
        key: String,
        value_type: &'static str,
        #[source]
        source: serde_cbor::Error,
    },
    #[error("surrealdb query execution failed: {message}")]
    Execution { message: String },
    #[error("surrealdb transaction conflict remained after {attempts} attempts: {message}")]
    ConflictRetryExhausted {
        attempts: NonZeroU32,
        message: String,
    },
}

/// Failure while selecting or decoding a statement result.
#[derive(Debug, Error)]
pub enum QueryResultError {
    #[error("query result index {index} is out of bounds for {result_count} statements")]
    IndexOutOfBounds { index: usize, result_count: usize },
    #[error("query statement {index} failed: {source}")]
    Statement {
        index: usize,
        #[source]
        source: StatementError,
    },
    #[error("query transaction returned more than one domain rejection")]
    MultipleDomainRejections,
    #[error("query result {index} could not be decoded: {source:#}")]
    Decode {
        index: usize,
        #[source]
        source: anyhow::Error,
    },
}

/// Domain rejection raised by `THROW` inside a SurrealQL transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainError {
    message: String,
}

impl DomainError {
    pub(crate) fn new(message: String) -> Self {
        Self { message }
    }

    /// Returns the unwrapped message supplied to `THROW`.
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[cfg(test)]
mod tests {
    use super::QueryResultError;

    #[test]
    fn reports_nested_decode_causes_through_telemetry() {
        let source =
            anyhow::anyhow!("expected a record identifier").context("failed to parse query result");
        let error = QueryResultError::Decode { index: 1, source };
        let reported = otel_wasi::Error::new("database-result-decode-failed", error);

        assert_eq!(
            otel_wasi::WasiError::message(&reported),
            "query result 1 could not be decoded: failed to parse query result: expected a record identifier"
        );
    }
}
