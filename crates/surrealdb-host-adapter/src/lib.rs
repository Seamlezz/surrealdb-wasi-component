mod convert;

use convert::{cbor_slice_to_json, ordered_params, surreal_to_cbor_bytes};
use surrealdb::{Surreal, engine::any::Any};
use surrealdb_types::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QueryError {
    #[error("failed to decode param {key}")]
    ParamDecode {
        key: String,
        #[source]
        source: anyhow::Error,
    },
    #[error("surreal query failed")]
    QueryExecution(#[source] surrealdb::Error),
}

pub async fn query(
    db: &Surreal<Any>,
    query: String,
    params: Vec<(String, Vec<u8>)>,
) -> Result<Vec<Result<Vec<u8>, String>>, QueryError> {
    let mut query_builder = db.query(&query);

    let mut decoded = Vec::with_capacity(params.len());
    for (key, value) in params {
        let decoded_value = cbor_slice_to_json(&value).map_err(|source| QueryError::ParamDecode {
            key: key.clone(),
            source,
        })?;
        decoded.push((key, decoded_value));
    }

    let ordered = ordered_params(decoded);
    query_builder = query_builder.bind(ordered);

    let mut response = query_builder
        .await
        .map_err(QueryError::QueryExecution)?;
    let mut results = Vec::with_capacity(response.num_statements());

    for index in 0..response.num_statements() {
        match response.take::<Value>(index) {
            Ok(value) => match surreal_to_cbor_bytes(value) {
                Ok(bytes) => results.push(Ok(bytes)),
                Err(error) => results.push(Err(error.to_string())),
            },
            Err(error) => results.push(Err(error.to_string())),
        }
    }

    Ok(results)
}
