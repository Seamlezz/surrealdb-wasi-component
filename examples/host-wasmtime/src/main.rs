use anyhow::{Context, Result, bail, ensure};
use futures_util::StreamExt;
use std::env;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context as TaskContext, Poll};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb_host_adapter::{SubscriptionManager, SubscriptionTask};
use tokio::sync::{RwLock, mpsc, oneshot};
use wasmtime::component::{
    Accessor, Component, Destination, HasSelf, Linker, ResourceTable, StreamProducer, StreamReader,
    StreamResult,
};
use wasmtime::{AsContextMut, Config, Engine, Store, StoreContextMut};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

mod bindings {
    wasmtime::component::bindgen!({
        path: "wit",
        world: "adapter",
        imports: { default: async | store | trappable },
        exports: {
            default: async,
        },
    });
}

pub struct SurrealHostAdapter {
    db: Arc<RwLock<Surreal<Any>>>,
    subscriptions: Arc<SubscriptionManager>,
    call_stats: Arc<CallStats>,
    table: ResourceTable,
    wasi: WasiCtx,
}

pub struct CallStats {
    query_calls: AtomicU64,
    subscribe_calls: AtomicU64,
    cancel_calls: AtomicU64,
}

impl CallStats {
    fn new() -> Self {
        Self {
            query_calls: AtomicU64::new(0),
            subscribe_calls: AtomicU64::new(0),
            cancel_calls: AtomicU64::new(0),
        }
    }

    fn increment_query(&self) {
        self.query_calls.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_subscribe(&self) {
        self.subscribe_calls.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_cancel(&self) {
        self.cancel_calls.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> (u64, u64, u64) {
        (
            self.query_calls.load(Ordering::Relaxed),
            self.subscribe_calls.load(Ordering::Relaxed),
            self.cancel_calls.load(Ordering::Relaxed),
        )
    }
}

impl SurrealHostAdapter {
    pub fn new(db: Surreal<Any>) -> Self {
        let wasi = WasiCtxBuilder::new().inherit_stdio().build();

        Self {
            db: Arc::new(RwLock::new(db)),
            subscriptions: Arc::new(SubscriptionManager::new()),
            call_stats: Arc::new(CallStats::new()),
            table: ResourceTable::new(),
            wasi,
        }
    }

    async fn shutdown(&self) {
        self.subscriptions.shutdown().await;
    }

    fn validate_demo_execution(&self) -> Result<()> {
        let (query_calls, subscribe_calls, cancel_calls) = self.call_stats.snapshot();

        ensure!(
            query_calls > 0,
            "guest component did not execute any query calls"
        );
        ensure!(
            subscribe_calls > 0,
            "guest component did not execute any subscribe calls"
        );
        ensure!(
            cancel_calls > 0,
            "guest component did not execute any cancel calls"
        );

        Ok(())
    }
}

impl bindings::seamlezz::surrealdb::call::Host for SurrealHostAdapter {}

impl WasiView for SurrealHostAdapter {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

type BindingLiveAction = bindings::seamlezz::surrealdb::call::LiveAction;
type BindingLiveEvent = bindings::seamlezz::surrealdb::call::LiveEvent;

fn map_live_action(action: surrealdb_host_adapter::LiveAction) -> BindingLiveAction {
    match action {
        surrealdb_host_adapter::LiveAction::Create => BindingLiveAction::Create,
        surrealdb_host_adapter::LiveAction::Update => BindingLiveAction::Update,
        surrealdb_host_adapter::LiveAction::Delete => BindingLiveAction::Delete,
        surrealdb_host_adapter::LiveAction::Killed => BindingLiveAction::Killed,
    }
}

fn to_binding_live_event(event: surrealdb_host_adapter::LiveEvent) -> BindingLiveEvent {
    BindingLiveEvent {
        subscription_id: event.subscription_id,
        query_id: event.query_id,
        action: map_live_action(event.action),
        data: event.data,
    }
}

struct LiveEventProducer {
    receiver: mpsc::UnboundedReceiver<BindingLiveEvent>,
}

impl LiveEventProducer {
    fn new(receiver: mpsc::UnboundedReceiver<BindingLiveEvent>) -> Self {
        Self { receiver }
    }
}

impl<T> StreamProducer<T> for LiveEventProducer {
    type Item = BindingLiveEvent;
    type Buffer = Option<Self::Item>;

    fn poll_produce<'a>(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        _store: StoreContextMut<'a, T>,
        mut destination: Destination<'a, Self::Item, Self::Buffer>,
        finish: bool,
    ) -> Poll<wasmtime::Result<StreamResult>> {
        if finish {
            return Poll::Ready(Ok(StreamResult::Cancelled));
        }

        let this = self.get_mut();
        match this.receiver.poll_recv(cx) {
            Poll::Ready(Some(event)) => {
                destination.set_buffer(Some(event));
                Poll::Ready(Ok(StreamResult::Completed))
            }
            Poll::Ready(None) => Poll::Ready(Ok(StreamResult::Dropped)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl bindings::seamlezz::surrealdb::call::HostWithStore for HasSelf<SurrealHostAdapter> {
    async fn query<T: Send>(
        accessor: &Accessor<T, Self>,
        query: String,
        params: Vec<(String, Vec<u8>)>,
    ) -> wasmtime::Result<Vec<Result<Vec<u8>, String>>> {
        let (db, call_stats) = accessor.with(|mut access| {
            let state = access.get();
            (Arc::clone(&state.db), Arc::clone(&state.call_stats))
        });
        call_stats.increment_query();

        let db = db.read().await;
        surrealdb_host_adapter::query(&db, query, params)
            .await
            .map_err(wasmtime::Error::new)
    }

    async fn subscribe<T: Send>(
        accessor: &Accessor<T, Self>,
        query: String,
        params: Vec<(String, Vec<u8>)>,
    ) -> wasmtime::Result<(u64, StreamReader<BindingLiveEvent>)> {
        let (db, subscriptions, call_stats) = accessor.with(|mut access| {
            let state = access.get();
            (
                Arc::clone(&state.db),
                Arc::clone(&state.subscriptions),
                Arc::clone(&state.call_stats),
            )
        });
        call_stats.increment_subscribe();
        let subscription_id = subscriptions.allocate_id();
        let stream = {
            let db = db.read().await;
            surrealdb_host_adapter::subscribe(&db, query, params)
                .await
                .map_err(wasmtime::Error::new)?
        };

        let (sender, receiver) = mpsc::unbounded_channel();
        let (stop_tx, mut stop_rx) = oneshot::channel();
        let mut stream = Box::pin(stream);
        let task_subscriptions = Arc::clone(&subscriptions);

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => {
                        break;
                    }
                    notification = stream.next() => {
                        let Some(Ok(notification)) = notification else {
                            break;
                        };

                        let Ok(event) = surrealdb_host_adapter::notification_to_live_event(subscription_id, notification) else {
                            break;
                        };

                        if sender.send(to_binding_live_event(event)).is_err() {
                            break;
                        }
                    }
                }
            }

            task_subscriptions.complete(subscription_id).await;
        });

        subscriptions
            .register(subscription_id, SubscriptionTask::new(stop_tx, handle))
            .await;

        let reader = accessor.with(|mut access| {
            StreamReader::new(access.as_context_mut(), LiveEventProducer::new(receiver))
        });

        Ok((subscription_id, reader))
    }

    async fn cancel<T: Send>(
        accessor: &Accessor<T, Self>,
        subscription_id: u64,
    ) -> wasmtime::Result<Result<(), String>> {
        let (subscriptions, call_stats) = accessor.with(|mut access| {
            let state = access.get();
            (
                Arc::clone(&state.subscriptions),
                Arc::clone(&state.call_stats),
            )
        });
        call_stats.increment_cancel();
        if subscriptions.cancel(subscription_id).await {
            return Ok(Ok(()));
        }

        Ok(Err(format!("subscription {} not found", subscription_id)))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let component_path = env::args()
        .nth(1)
        .context("Usage: host-wasmtime <path-to-component.wasm>")?;

    let db_url = "memory";
    let db_ns = "example_validation";
    let db_name = "example_validation";

    let db: Surreal<Any> = Surreal::init();
    db.connect(db_url)
        .await
        .context("failed to connect to SurrealDB")?;
    db.use_ns(db_ns)
        .use_db(db_name)
        .await
        .context("failed to select namespace and database")?;

    let adapter = SurrealHostAdapter::new(db);

    let mut config = Config::new();
    config.wasm_component_model(true);
    config.wasm_component_model_async(true);

    let engine = Engine::new(&config)?;

    let component = Component::from_file(&engine, &component_path)?;

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;

    bindings::Adapter::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;

    let mut store = Store::new(&engine, adapter);
    let instance = linker.instantiate_async(&mut store, &component).await?;
    let bindings = bindings::Adapter::new(&mut store, &instance)?;
    let run_result = store
        .run_concurrent(async |accessor| {
            bindings
                .seamlezz_surrealdb_host_adapter_demo()
                .call_run(accessor)
                .await
        })
        .await??;
    if let Err(message) = run_result {
        bail!("guest demo execution failed: {message}");
    }

    println!(
        "Successfully loaded and instantiated component: {}",
        component_path
    );
    println!("Connected to SurrealDB at {}", db_url);
    println!("Using namespace: {}, database: {}", db_ns, db_name);

    store.data().validate_demo_execution()?;
    println!("Validated guest demo runtime behavior");

    store.data().shutdown().await;

    Ok(())
}
