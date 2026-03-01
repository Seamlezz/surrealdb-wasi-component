mod convert;
mod manager;

use convert::{cbor_slice_to_json, ordered_params, surreal_to_cbor_bytes};
use surrealdb::{Notification, Surreal, engine::any::Any, method::QueryStream};
use surrealdb_types::{Action, Value};
use thiserror::Error;

pub use manager::{SubscriptionManager, SubscriptionTask};

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

#[derive(Debug, Clone, Copy)]
pub enum LiveAction {
    Create,
    Update,
    Delete,
    Killed,
}

#[derive(Debug, Clone)]
pub struct LiveEvent {
    pub subscription_id: u64,
    pub query_id: String,
    pub action: LiveAction,
    pub data: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum SubscribeError {
    #[error("failed to decode param {key}")]
    ParamDecode {
        key: String,
        #[source]
        source: anyhow::Error,
    },
    #[error("surreal query failed")]
    QueryExecution(#[source] surrealdb::Error),
    #[error("failed to open live stream")]
    StreamOpen(#[source] surrealdb::Error),
    #[error("failed to serialize live notification")]
    Serialize(#[source] anyhow::Error),
}

fn decode_params<E>(
    params: Vec<(String, Vec<u8>)>,
    map_error: impl Fn(String, anyhow::Error) -> E,
) -> Result<Vec<(String, serde_json::Value)>, E> {
    let mut decoded = Vec::with_capacity(params.len());
    for (key, value) in params {
        let decoded_value = cbor_slice_to_json(&value).map_err(|source| map_error(key.clone(), source))?;
        decoded.push((key, decoded_value));
    }

    Ok(decoded)
}

pub async fn query(
    db: &Surreal<Any>,
    query: String,
    params: Vec<(String, Vec<u8>)>,
) -> Result<Vec<Result<Vec<u8>, String>>, QueryError> {
    let mut query_builder = db.query(&query);
    let decoded = decode_params(params, |key, source| QueryError::ParamDecode { key, source })?;

    let ordered = ordered_params(decoded);
    query_builder = query_builder.bind(ordered);

    let mut response = query_builder.await.map_err(QueryError::QueryExecution)?;
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

pub async fn subscribe(
    db: &Surreal<Any>,
    query: String,
    params: Vec<(String, Vec<u8>)>,
) -> Result<QueryStream<Notification<Value>>, SubscribeError> {
    let mut query_builder = db.query(&query);
    let decoded = decode_params(params, |key, source| SubscribeError::ParamDecode { key, source })?;

    let ordered = ordered_params(decoded);
    query_builder = query_builder.bind(ordered);

    let mut response = query_builder
        .await
        .map_err(SubscribeError::QueryExecution)?;

    response
        .stream::<Notification<Value>>(())
        .map_err(SubscribeError::StreamOpen)
}

pub fn notification_to_live_event(
    subscription_id: u64,
    notification: Notification<Value>,
) -> Result<LiveEvent, SubscribeError> {
    let action = match notification.action {
        Action::Create => LiveAction::Create,
        Action::Update => LiveAction::Update,
        Action::Delete => LiveAction::Delete,
        Action::Killed => LiveAction::Killed,
    };

    let data = surreal_to_cbor_bytes(notification.data).map_err(SubscribeError::Serialize)?;

    let event = LiveEvent {
        subscription_id,
        query_id: notification.query_id.to_string(),
        action,
        data,
    };
    Ok(event)
}
