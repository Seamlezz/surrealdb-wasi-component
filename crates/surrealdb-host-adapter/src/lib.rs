mod convert;
mod manager;

use std::any::Any as Payload;
use std::ops::Deref;

use convert::{cbor_slice_to_surreal, ordered_params, surreal_to_cbor_bytes};
use opentelemetry::context::FutureExt as _;
use surrealdb::{Notification, Surreal, engine::any::Any, method::QueryStream};
use surrealdb_types::{Action, ErrorDetails, QueryError as SurrealQueryError, Value};
use thiserror::Error;
use tokio::task::{JoinError, JoinHandle};
use tracing::Instrument as _;

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
    #[error("database task failed: {0}")]
    TaskFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementError {
    Domain(String),
    RetryableConflict(String),
    NotExecuted(String),
    Failure(String),
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
    #[error("database task failed: {0}")]
    TaskFailed(String),
}

/// Runs database work on a tokio task of its own and waits for it.
///
/// Wasmtime polls a host import on the calling guest's fiber, whose stack is small, and the
/// SurrealDB 4 embedded engine executes a query inside the future that awaits it. A task of its
/// own gives that work a runtime thread's stack. The task carries the caller's tracing span and
/// OpenTelemetry context, so database spans stay children of the guest call; it is aborted when
/// the caller drops the returned future, and a panic in it becomes an error.
async fn run_on_own_task<T: Send + 'static>(
    work: impl Future<Output = T> + Send + 'static,
) -> Result<T, String> {
    let work = work
        .instrument(tracing::Span::current())
        .with_current_context();
    let mut task = AbortOnDrop(tokio::spawn(work));
    (&mut task.0).await.map_err(task_failure)
}

struct AbortOnDrop<T>(JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn task_failure(error: JoinError) -> String {
    match error.try_into_panic() {
        Ok(payload) => panic_message(payload),
        Err(error) => error.to_string(),
    }
}

fn panic_message(payload: Box<dyn Payload + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => payload.downcast_ref::<&str>().map_or_else(
            || "database task panicked".to_string(),
            |message| message.to_string(),
        ),
    }
}

fn decode_params<E>(
    params: Vec<(String, Vec<u8>)>,
    map_error: impl Fn(String, anyhow::Error) -> E,
) -> Result<Vec<(String, Value)>, E> {
    let mut decoded = Vec::with_capacity(params.len());
    for (key, value) in params {
        let decoded_value =
            cbor_slice_to_surreal(&value).map_err(|source| map_error(key.clone(), source))?;
        decoded.push((key, decoded_value));
    }

    Ok(decoded)
}

/// Executes a guest statement set and encodes each statement result as normalized CBOR.
///
/// `db` is an owned handle on the host's client, such as an `Arc<Surreal<Any>>` or a tokio
/// `OwnedRwLockReadGuard<Surreal<Any>>`, and the statements run in that client's session.
/// `Surreal::clone` would instead open a new session for every call.
///
/// The work runs on a tokio task of its own, so a host import polled on a guest's small fiber
/// stack never executes SurrealDB itself. The threads of the tokio runtime need the 10 MiB stack
/// SurrealDB asks an embedding runtime for. Dropping the returned future aborts the work, and a
/// panic in it returns [`QueryError::TaskFailed`].
///
/// # Panics
///
/// Panics when called outside a tokio runtime.
pub async fn query<D>(
    db: D,
    query: String,
    params: Vec<(String, Vec<u8>)>,
) -> Result<Vec<Result<Vec<u8>, StatementError>>, QueryError>
where
    D: Deref<Target = Surreal<Any>> + Send + Sync + 'static,
{
    run_on_own_task(async move { execute_query(&db, query, params).await })
        .await
        .map_err(QueryError::TaskFailed)?
}

async fn execute_query(
    db: &Surreal<Any>,
    query: String,
    params: Vec<(String, Vec<u8>)>,
) -> Result<Vec<Result<Vec<u8>, StatementError>>, QueryError> {
    let mut query_builder = db.query(&query);
    let decoded = decode_params(params, |key, source| QueryError::ParamDecode {
        key,
        source,
    })?;

    let ordered = ordered_params(decoded);
    query_builder = query_builder.bind(ordered);

    let mut response = query_builder.await.map_err(QueryError::QueryExecution)?;
    let mut results = Vec::with_capacity(response.num_statements());

    for index in 0..response.num_statements() {
        match response.take::<Value>(index) {
            Ok(value) => match surreal_to_cbor_bytes(value) {
                Ok(bytes) => results.push(Ok(bytes)),
                Err(error) => results.push(Err(StatementError::Failure(error.to_string()))),
            },
            Err(error) => results.push(Err(statement_error(error))),
        }
    }

    Ok(results)
}

fn statement_error(error: surrealdb::Error) -> StatementError {
    let message = error.message().to_string();
    match error.details() {
        ErrorDetails::Thrown => StatementError::Domain(
            message
                .strip_prefix("An error occurred: ")
                .unwrap_or(&message)
                .to_string(),
        ),
        ErrorDetails::Query(Some(SurrealQueryError::TransactionConflict)) => {
            StatementError::RetryableConflict(message)
        }
        ErrorDetails::Query(Some(
            SurrealQueryError::NotExecuted | SurrealQueryError::Cancelled,
        )) => StatementError::NotExecuted(message),
        _ => StatementError::Failure(message),
    }
}

/// Opens a live query and returns its notification stream.
///
/// Takes `db` and runs on a task of its own exactly as [`query`] does; a panic in the work returns
/// [`SubscribeError::TaskFailed`].
///
/// # Panics
///
/// Panics when called outside a tokio runtime.
pub async fn subscribe<D>(
    db: D,
    query: String,
    params: Vec<(String, Vec<u8>)>,
) -> Result<QueryStream<Notification<Value>>, SubscribeError>
where
    D: Deref<Target = Surreal<Any>> + Send + Sync + 'static,
{
    run_on_own_task(async move { open_live_query(&db, query, params).await })
        .await
        .map_err(SubscribeError::TaskFailed)?
}

async fn open_live_query(
    db: &Surreal<Any>,
    query: String,
    params: Vec<(String, Vec<u8>)>,
) -> Result<QueryStream<Notification<Value>>, SubscribeError> {
    let mut query_builder = db.query(&query);
    let decoded = decode_params(params, |key, source| SubscribeError::ParamDecode {
        key,
        source,
    })?;

    let ordered = ordered_params(decoded);
    query_builder = query_builder.bind(ordered);

    let mut response = query_builder
        .await
        .map_err(SubscribeError::QueryExecution)?;

    let stream = response
        .stream::<Notification<Value>>(())
        .map_err(SubscribeError::StreamOpen)?;
    Ok(stream)
}

pub fn notification_to_live_event(
    subscription_id: u64,
    notification: Notification<Value>,
) -> Result<LiveEvent, SubscribeError> {
    let action = match notification.action {
        Action::Create => LiveAction::Create,
        Action::Update => LiveAction::Update,
        Action::Delete => LiveAction::Delete,
        Action::Killed | Action::Error => LiveAction::Killed,
    };

    let data = surreal_to_cbor_bytes(notification.data).map_err(SubscribeError::Serialize)?;

    Ok(LiveEvent {
        subscription_id,
        query_id: notification.query_id.to_string(),
        action,
        data,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use surrealdb::{Surreal, engine::any::Any};
    use surrealdb_types::{Error, QueryError as SurrealQueryError};

    use std::sync::Arc;

    use super::{StatementError, query, run_on_own_task, statement_error, subscribe};

    /// Wasmtime polls a host import on the calling guest's fiber, whose stack defaults to 2 MiB.
    /// A quarter of that leaves no room for SurrealDB to execute on the caller's stack.
    const GUEST_FIBER_STACK: usize = 512 * 1024;

    /// The worker stack SurrealDB asks an embedding runtime for.
    const SURREALDB_THREAD_STACK: usize = 10 * 1024 * 1024;

    /// Polls `call` on a thread with a guest fiber sized stack while `runtime` keeps its own
    /// worker threads, the way a host import is polled inside a running host.
    fn poll_on_guest_stack<T: Send + 'static>(
        runtime: &tokio::runtime::Runtime,
        call: impl Future<Output = T> + Send + 'static,
    ) -> T {
        let handle = runtime.handle().clone();
        std::thread::Builder::new()
            .stack_size(GUEST_FIBER_STACK)
            .spawn(move || handle.block_on(call))
            .unwrap()
            .join()
            .unwrap()
    }

    fn host_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_stack_size(SURREALDB_THREAD_STACK)
            .build()
            .unwrap()
    }

    #[test]
    fn a_panic_in_database_work_becomes_an_error() {
        let runtime = host_runtime();

        let outcome = runtime.block_on(run_on_own_task(async { panic!("engine exploded") }));

        assert_eq!(outcome, Err::<(), _>("engine exploded".to_string()));
    }

    #[test]
    fn dropping_the_call_aborts_its_database_work() {
        let runtime = host_runtime();
        let (work_dropped_tx, work_dropped_rx) = tokio::sync::oneshot::channel::<()>();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();

        runtime.block_on(async move {
            let call = run_on_own_task(async move {
                let _drop_signal = work_dropped_tx;
                started_tx.send(()).unwrap();
                std::future::pending::<()>().await;
            });
            tokio::select! {
                _ = call => unreachable!("pending work never completes"),
                _ = started_rx => {}
            }
        });

        let aborted = runtime.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(5), work_dropped_rx).await
        });
        assert!(
            matches!(aborted, Ok(Err(_))),
            "the abandoned work kept running"
        );
    }

    #[test]
    fn database_work_runs_in_the_callers_span_and_trace_context() {
        use opentelemetry::{Context, context::FutureExt as _};
        use tracing::Instrument as _;

        #[derive(Debug, PartialEq)]
        struct GuestCall(u32);

        let subscriber = tracing_subscriber::registry();
        tracing::subscriber::with_default(subscriber, || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            let guest_span = tracing::info_span!("guest call");
            let guest_context = Context::new().with_value(GuestCall(7));

            let (span, call) = runtime
                .block_on(
                    run_on_own_task(async {
                        (
                            tracing::Span::current().id(),
                            Context::current().get::<GuestCall>().map(|call| call.0),
                        )
                    })
                    .instrument(guest_span.clone())
                    .with_context(guest_context),
                )
                .unwrap();

            assert_eq!(span, guest_span.id());
            assert_eq!(call, Some(7));
        });
    }

    #[test]
    fn query_runs_when_polled_on_a_guest_fiber_stack() {
        let runtime = host_runtime();
        let db = runtime.block_on(test_db());

        let results = poll_on_guest_stack(&runtime, async move {
            query(
                Arc::clone(&db),
                "DEFINE TABLE person SCHEMALESS; CREATE person:one SET name = 'one'; SELECT VALUE name FROM person;"
                    .to_string(),
                Vec::new(),
            )
            .await
            .unwrap()
        });

        let names =
            serde_cbor::from_slice::<serde_json::Value>(results[2].as_ref().unwrap()).unwrap();
        assert_eq!(names, json!(["one"]));
    }

    #[test]
    fn subscribe_runs_when_polled_on_a_guest_fiber_stack() {
        let runtime = host_runtime();
        let db = runtime.block_on(async {
            let db = test_db().await;
            query(
                Arc::clone(&db),
                "DEFINE TABLE person SCHEMALESS".into(),
                Vec::new(),
            )
            .await
            .unwrap();
            db
        });

        let opened = poll_on_guest_stack(&runtime, async move {
            subscribe(
                Arc::clone(&db),
                "LIVE SELECT * FROM person".to_string(),
                Vec::new(),
            )
            .await
            .is_ok()
        });

        assert!(opened);
    }

    async fn test_db() -> Arc<Surreal<Any>> {
        let db: Surreal<Any> = Surreal::init();
        db.connect("memory").await.unwrap();
        db.use_ns("host_adapter_tests")
            .use_db("typed_transport")
            .await
            .unwrap();
        Arc::new(db)
    }

    #[tokio::test]
    async fn binds_uuid_params_as_native_surreal_values() {
        let db = test_db().await;
        let param = serde_cbor::to_vec(&json!({
            "$surrealdb::uuid": "018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d"
        }))
        .unwrap();

        let results = query(
            Arc::clone(&db),
            "RETURN type::is_uuid($id); RETURN $id;".to_string(),
            vec![("id".to_string(), param)],
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 2);

        let is_uuid =
            serde_cbor::from_slice::<serde_json::Value>(results[0].as_ref().unwrap()).unwrap();
        assert_eq!(is_uuid, json!(true));

        let value =
            serde_cbor::from_slice::<serde_json::Value>(results[1].as_ref().unwrap()).unwrap();
        assert_eq!(
            value,
            json!({"$surrealdb::uuid": "018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d"})
        );
    }

    #[tokio::test]
    async fn binds_record_id_with_string_key_as_native_record() {
        let db = test_db().await;
        let param = serde_cbor::to_vec(&json!({"table": "person", "key": "demo"})).unwrap();

        let results = query(
            Arc::clone(&db),
            "RETURN $id = person:demo; RETURN type::is_record($id);".to_string(),
            vec![("id".to_string(), param)],
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 2);

        let is_equal =
            serde_cbor::from_slice::<serde_json::Value>(results[0].as_ref().unwrap()).unwrap();
        assert_eq!(is_equal, json!(true));

        let is_record =
            serde_cbor::from_slice::<serde_json::Value>(results[1].as_ref().unwrap()).unwrap();
        assert_eq!(is_record, json!(true));
    }

    #[tokio::test]
    async fn binds_record_id_with_number_key_as_native_record() {
        let db = test_db().await;
        let param = serde_cbor::to_vec(&json!({"table": "person", "key": 42})).unwrap();

        let results = query(
            Arc::clone(&db),
            "RETURN $id = person:42; RETURN type::is_record($id);".to_string(),
            vec![("id".to_string(), param)],
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 2);

        let is_equal =
            serde_cbor::from_slice::<serde_json::Value>(results[0].as_ref().unwrap()).unwrap();
        assert_eq!(is_equal, json!(true));

        let is_record =
            serde_cbor::from_slice::<serde_json::Value>(results[1].as_ref().unwrap()).unwrap();
        assert_eq!(is_record, json!(true));
    }

    #[tokio::test]
    async fn binds_record_id_with_uuid_key_as_native_record() {
        let db = test_db().await;
        let param = serde_cbor::to_vec(&json!({
            "table": "person",
            "key": {"$surrealdb::uuid": "018f6b5b-f4b4-7f28-8b34-9b46ef4f2f4d"}
        }))
        .unwrap();

        let results = query(
            Arc::clone(&db),
            "RETURN type::is_record($id);".to_string(),
            vec![("id".to_string(), param)],
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 1);

        let is_record =
            serde_cbor::from_slice::<serde_json::Value>(results[0].as_ref().unwrap()).unwrap();
        assert_eq!(is_record, json!(true));
    }

    #[tokio::test]
    async fn binds_record_id_with_object_key_as_native_record() {
        let db = test_db().await;
        let param = serde_cbor::to_vec(&json!({
            "table": "person",
            "key": {"tenant": "demo", "shard": 1}
        }))
        .unwrap();

        let results = query(
            Arc::clone(&db),
            "RETURN type::is_record($id);".to_string(),
            vec![("id".to_string(), param)],
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 1);

        let is_record =
            serde_cbor::from_slice::<serde_json::Value>(results[0].as_ref().unwrap()).unwrap();
        assert_eq!(is_record, json!(true));
    }

    async fn single_results(db: &Arc<Surreal<Any>>, query_text: &str) -> Vec<serde_json::Value> {
        query(Arc::clone(db), query_text.to_string(), Vec::new())
            .await
            .unwrap()
            .into_iter()
            .map(|result| serde_cbor::from_slice(&result.unwrap()).unwrap())
            .collect()
    }

    #[tokio::test]
    async fn returns_sized_numbers_as_plain_numbers() {
        let db = test_db().await;

        let results = single_results(&db, "RETURN 255u8; RETURN -3i16; RETURN 1.5f32;").await;

        assert_eq!(results, vec![json!(255), json!(-3), json!(1.5)]);
    }

    #[tokio::test]
    async fn returns_calendar_values_as_text() {
        let db = test_db().await;

        let results = single_results(
            &db,
            "RETURN pd'2026-11-19'; RETURN pt'13:45:00'; RETURN ts'2026-11-19T13:45:00'; RETURN ivl'P1M2D';",
        )
        .await;

        assert_eq!(
            results,
            vec![
                json!("2026-11-19"),
                json!("13:45:00"),
                json!("2026-11-19T13:45:00"),
                json!("P1M2D"),
            ]
        );
    }

    #[tokio::test]
    async fn guest_queries_run_in_the_hosts_session() {
        let db = test_db().await;
        let host_session: Option<String> = db
            .query("RETURN <string> session::id()")
            .await
            .unwrap()
            .take(0)
            .unwrap();

        let guest_session = single_results(&db, "RETURN <string> session::id();").await;

        assert_eq!(guest_session, vec![json!(host_session.unwrap())]);
    }

    #[tokio::test]
    async fn round_trips_unsigned_integers_beyond_i64() {
        let db = test_db().await;
        let large = u64::MAX - 5;
        let param = serde_cbor::to_vec(&large).unwrap();

        let results = query(
            Arc::clone(&db),
            "RETURN $count;".to_string(),
            vec![("count".to_string(), param)],
        )
        .await
        .unwrap();

        let value = serde_cbor::from_slice::<u64>(results[0].as_ref().unwrap()).unwrap();
        assert_eq!(value, large);
    }

    #[test]
    fn preserves_structured_transaction_conflicts() {
        let error = Error::query(
            "Transaction conflict".into(),
            SurrealQueryError::TransactionConflict,
        );

        assert_eq!(
            statement_error(error),
            StatementError::RetryableConflict("Transaction conflict".into())
        );
    }

    #[tokio::test]
    async fn preserves_thrown_domain_errors() {
        let db = test_db().await;
        let results = query(Arc::clone(&db), "THROW 'invalid-token'".into(), Vec::new())
            .await
            .unwrap();

        assert_eq!(
            results,
            vec![Err(StatementError::Domain("invalid-token".into()))]
        );
    }
}
