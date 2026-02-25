use std::sync::Arc;

use anyhow::{Context, Result};
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::{Database, Namespace, Root};
use surrealdb::{Surreal, Value};
use tokio::sync::RwLock;

use crate::config::{Auth, SurrealHostConfig};
use crate::convert::{cbor_slice_to_json, ordered_params, surreal_to_cbor_bytes};

#[derive(Clone)]
pub struct SurrealHostAdapter {
    db: Arc<RwLock<Surreal<Any>>>,
}

impl SurrealHostAdapter {
    pub async fn connect(config: SurrealHostConfig) -> Result<Self> {
        config.validate()?;

        let db: Surreal<Any> = Surreal::init();
        db.connect(config.url.clone())
            .await
            .with_context(|| format!("failed to connect to surrealdb at {}", config.url))?;

        match &config.auth {
            Auth::Root { username, password } => {
                db.signin(Root { username, password })
                    .await
                    .context("failed root authentication")?;
            }
            Auth::Namespace { username, password } => {
                db.signin(Namespace {
                    namespace: &config.namespace,
                    username,
                    password,
                })
                .await
                .context("failed namespace authentication")?;
            }
            Auth::Database { username, password } => {
                db.signin(Database {
                    namespace: &config.namespace,
                    database: &config.database,
                    username,
                    password,
                })
                .await
                .context("failed database authentication")?;
            }
        }

        db.use_ns(&config.namespace)
            .use_db(&config.database)
            .await
            .with_context(|| {
                format!(
                    "failed to select namespace {} and database {}",
                    config.namespace, config.database
                )
            })?;

        Ok(Self {
            db: Arc::new(RwLock::new(db)),
        })
    }

    pub async fn query(
        &self,
        query: String,
        params: Vec<(String, Vec<u8>)>,
    ) -> Result<Vec<Result<Vec<u8>, String>>> {
        let db = self.db.read().await;
        let mut query_builder = db.query(&query);

        let mut decoded = Vec::with_capacity(params.len());
        for (key, value) in params {
            let decoded_value = cbor_slice_to_json(&value)
                .with_context(|| format!("failed to decode param {key}"))?;
            decoded.push((key, decoded_value));
        }

        let ordered = ordered_params(decoded);
        query_builder = query_builder.bind(ordered);

        let mut response = query_builder.await.context("surreal query failed")?;
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
}
