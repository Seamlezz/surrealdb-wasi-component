use anyhow::{Result, anyhow};
use serde::de::DeserializeOwned;

use crate::decoder;

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

#[derive(Debug, Clone)]
pub struct QueryResultHolder {
    results: Vec<Result<Vec<u8>, String>>,
}

impl QueryResultHolder {
    pub fn new(results: Vec<Result<Vec<u8>, String>>) -> Self {
        Self { results }
    }

    pub fn len(&self) -> usize {
        self.results.len()
    }

    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    pub fn take<T: SingleQueryResultExtractor>(&self, index: usize) -> Result<T> {
        let bytes = self
            .results
            .get(index)
            .ok_or_else(|| anyhow!("result index {index} out of bounds"))?
            .as_ref()
            .map_err(|error| anyhow!(error.clone()))?;

        T::from_bytes(bytes)
    }

    pub fn take_result<T: SingleQueryResultExtractor>(
        &self,
        index: usize,
    ) -> Result<Result<T, String>> {
        let Some(result) = self.results.get(index) else {
            return Err(anyhow!("result index {index} out of bounds"));
        };

        if let Some(error) = self.find_user_error() {
            return Ok(Err(error));
        }

        match result {
            Ok(bytes) => Ok(Ok(T::from_bytes(bytes)?)),
            Err(error) => Ok(Err(error.clone())),
        }
    }

    pub fn parse<D: DeserializeOwned>(&self, index: usize) -> Result<D> {
        let bytes = self
            .results
            .get(index)
            .ok_or_else(|| anyhow!("result index {index} out of bounds"))?
            .as_ref()
            .map_err(|error| anyhow!(error.clone()))?;

        parse(bytes)
    }

    pub fn parse_result<D: DeserializeOwned>(&self, index: usize) -> Result<Result<D, String>> {
        let Some(result) = self.results.get(index) else {
            return Err(anyhow!("result index {index} out of bounds"));
        };

        if let Some(error) = self.find_user_error() {
            return Ok(Err(error));
        }

        match result {
            Ok(bytes) => Ok(Ok(parse(bytes)?)),
            Err(error) => Ok(Err(error.clone())),
        }
    }

    pub fn find_user_error(&self) -> Option<String> {
        let errors: Vec<String> = self
            .results
            .iter()
            .filter_map(|result| result.as_ref().err().cloned())
            .filter(|error| {
                !error.contains("The query was not executed due to a failed transaction")
                    && !error.contains("The query was not executed due to a cancelled transaction")
                    && !error.contains("the transaction was aborted due to a prior error")
            })
            .collect();

        if errors.is_empty() {
            return None;
        }

        Some(errors.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::QueryResultHolder;

    const FAILED_TRANSACTION: &str = "The query was not executed due to a failed transaction";
    const CANCELLED_TRANSACTION: &str = "The query was not executed due to a cancelled transaction";

    #[test]
    fn parse_result_returns_meaningful_transaction_errors() {
        let results = QueryResultHolder::new(vec![
            Ok(serde_cbor::to_vec(&serde_cbor::Value::Null).unwrap()),
            Err(FAILED_TRANSACTION.into()),
            Err("Specify a database to use".into()),
            Err(CANCELLED_TRANSACTION.into()),
            Err("Another statement error".into()),
        ]);

        let error = results
            .parse_result::<serde_cbor::Value>(0)
            .unwrap()
            .unwrap_err();

        assert_eq!(error, "Specify a database to use; Another statement error");
    }

    #[test]
    fn take_result_returns_meaningful_transaction_errors() {
        let results = QueryResultHolder::new(vec![
            Ok(serde_cbor::to_vec(&Vec::<i32>::new()).unwrap()),
            Err(CANCELLED_TRANSACTION.into()),
            Err("Specify a database to use".into()),
        ]);

        let error = results.take_result::<Option<i32>>(0).unwrap().unwrap_err();

        assert_eq!(error, "Specify a database to use");
    }

    #[test]
    fn result_methods_preserve_transaction_error_without_meaningful_error() {
        let results = QueryResultHolder::new(vec![
            Err(FAILED_TRANSACTION.into()),
            Err(CANCELLED_TRANSACTION.into()),
        ]);

        let parse_error = results
            .parse_result::<serde_cbor::Value>(0)
            .unwrap()
            .unwrap_err();
        let take_error = results.take_result::<Option<i32>>(1).unwrap().unwrap_err();

        assert_eq!(parse_error, FAILED_TRANSACTION);
        assert_eq!(take_error, CANCELLED_TRANSACTION);
    }
}
