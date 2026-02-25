use std::any::type_name;

use anyhow::{Context, Result, anyhow};
use serde::de::DeserializeOwned;

pub trait SingleQueryResultExtractor: Sized {
    fn from_bytes(bytes: &[u8]) -> Result<Self>;
}

fn parse<D: DeserializeOwned>(bytes: &[u8]) -> Result<D> {
    ciborium::from_reader::<D, _>(bytes).with_context(|| {
        format!(
            "failed to parse query result into type {}",
            type_name::<D>()
        )
    })
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
            })
            .collect();

        if errors.is_empty() {
            return None;
        }

        Some(errors.join("; "))
    }
}
