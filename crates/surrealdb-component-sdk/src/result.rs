use std::num::NonZeroU32;

use anyhow::Result;
use serde::de::DeserializeOwned;

use crate::{
    DomainError, QueryResultError, StatementError, bindings::seamlezz::surrealdb::call, decoder,
};

/// Extracts one statement result using SurrealDB collection semantics.
pub trait SingleQueryResultExtractor: Sized {
    fn from_bytes(bytes: &[u8]) -> Result<Self>;
}

fn parse<D: DeserializeOwned>(bytes: &[u8]) -> Result<D> {
    decoder::decode(bytes, "failed to parse query result")
}

impl<D: DeserializeOwned> SingleQueryResultExtractor for Vec<D> {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        parse(bytes)
    }
}

impl<D: DeserializeOwned> SingleQueryResultExtractor for Option<D> {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let values: Vec<D> = parse(bytes)?;
        Ok(values.into_iter().next())
    }
}

/// Typed result of an explicit SurrealQL transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionOutcome<T> {
    Committed(T),
    Rejected(DomainError),
}

/// Results returned by one query execution, including its attempt count.
#[derive(Debug, Clone)]
pub struct QueryResponse {
    results: Vec<Result<Vec<u8>, StatementError>>,
    attempts: NonZeroU32,
}

impl QueryResponse {
    pub(crate) fn new(results: Vec<Result<Vec<u8>, StatementError>>) -> Self {
        Self {
            results,
            attempts: NonZeroU32::new(1).expect("one is nonzero"),
        }
    }

    pub(crate) fn set_attempts(&mut self, attempts: NonZeroU32) {
        self.attempts = attempts;
    }

    /// Returns how many complete query executions were attempted.
    pub fn attempts(&self) -> NonZeroU32 {
        self.attempts
    }

    /// Returns the number of statement results.
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Returns whether the query produced no statement results.
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Extracts a collection shaped statement result.
    pub fn take<T: SingleQueryResultExtractor>(&self, index: usize) -> Result<T, QueryResultError> {
        let bytes = self.statement_bytes(index)?;
        T::from_bytes(bytes).map_err(|source| QueryResultError::Decode { index, source })
    }

    /// Decodes a statement result directly into `D`.
    pub fn parse<D: DeserializeOwned>(&self, index: usize) -> Result<D, QueryResultError> {
        let bytes = self.statement_bytes(index)?;
        parse(bytes).map_err(|source| QueryResultError::Decode { index, source })
    }

    /// Decodes a transaction result while preserving a domain rejection as data.
    pub fn transaction<D: DeserializeOwned>(
        &self,
        index: usize,
    ) -> Result<TransactionOutcome<D>, QueryResultError> {
        let mut domain_errors = self.results.iter().filter_map(|result| match result {
            Err(StatementError::Domain(message)) => Some(message),
            _ => None,
        });
        if let Some(message) = domain_errors.next() {
            if domain_errors.next().is_some() {
                return Err(QueryResultError::MultipleDomainRejections);
            }
            return Ok(TransactionOutcome::Rejected(DomainError::new(
                message.clone(),
            )));
        }

        if let Some((failure_index, source)) =
            self.results
                .iter()
                .enumerate()
                .find_map(|(failure_index, result)| match result {
                    Err(source @ StatementError::RetryableConflict(_))
                    | Err(source @ StatementError::Failure(_)) => {
                        Some((failure_index, source.clone()))
                    }
                    _ => None,
                })
        {
            return Err(QueryResultError::Statement {
                index: failure_index,
                source,
            });
        }

        let bytes = self.statement_bytes(index)?;
        let value = parse(bytes).map_err(|source| QueryResultError::Decode { index, source })?;
        Ok(TransactionOutcome::Committed(value))
    }

    pub(crate) fn retryable_conflict(&self) -> Option<String> {
        self.results.iter().find_map(|result| match result {
            Err(StatementError::RetryableConflict(message)) => Some(message.clone()),
            _ => None,
        })
    }

    fn statement_bytes(&self, index: usize) -> Result<&[u8], QueryResultError> {
        let result = self
            .results
            .get(index)
            .ok_or(QueryResultError::IndexOutOfBounds {
                index,
                result_count: self.results.len(),
            })?;
        result
            .as_deref()
            .map_err(|source| QueryResultError::Statement {
                index,
                source: source.clone(),
            })
    }
}

impl From<call::StatementError> for StatementError {
    fn from(value: call::StatementError) -> Self {
        match value {
            call::StatementError::Domain(message) => Self::Domain(message),
            call::StatementError::RetryableConflict(message) => Self::RetryableConflict(message),
            call::StatementError::NotExecuted(message) => Self::NotExecuted(message),
            call::StatementError::Failure(message) => Self::Failure(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::{QueryResponse, TransactionOutcome};
    use crate::{QueryResultError, StatementError};

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    struct Value {
        value: i32,
    }

    fn encoded_value() -> Vec<u8> {
        serde_cbor::to_vec(&Value { value: 42 }).unwrap()
    }

    #[test]
    fn returns_committed_transaction_value() {
        let response = QueryResponse::new(vec![Ok(encoded_value())]);

        assert_eq!(
            response.transaction::<Value>(0).unwrap(),
            TransactionOutcome::Committed(Value { value: 42 })
        );
    }

    #[test]
    fn returns_typed_domain_rejection_from_any_statement() {
        let response = QueryResponse::new(vec![
            Err(StatementError::Domain("invalid-token".into())),
            Err(StatementError::NotExecuted("transaction aborted".into())),
        ]);

        let TransactionOutcome::Rejected(error) = response.transaction::<Value>(1).unwrap() else {
            panic!("expected a domain rejection");
        };
        assert_eq!(error.message(), "invalid-token");
    }

    #[test]
    fn rejects_ambiguous_domain_failures() {
        let response = QueryResponse::new(vec![
            Err(StatementError::Domain("first".into())),
            Err(StatementError::Domain("second".into())),
        ]);

        assert!(matches!(
            response.transaction::<Value>(0),
            Err(QueryResultError::MultipleDomainRejections)
        ));
    }

    #[test]
    fn preserves_nonretryable_statement_failures() {
        let response = QueryResponse::new(vec![
            Err(StatementError::Failure("database unavailable".into())),
            Err(StatementError::NotExecuted("transaction aborted".into())),
        ]);

        assert!(matches!(
            response.transaction::<Value>(1),
            Err(QueryResultError::Statement {
                index: 0,
                source: StatementError::Failure(message),
            }) if message == "database unavailable"
        ));
    }
}
