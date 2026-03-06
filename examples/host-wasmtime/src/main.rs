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
        println!(
            "wasi host query begin. query={}, params_len={}",
            query,
            params.len()
        );
        let (db, call_stats) = accessor.with(|mut access| {
            let state = access.get();
            (Arc::clone(&state.db), Arc::clone(&state.call_stats))
        });
        println!("wasi host query state captured");
        call_stats.increment_query();
        println!("wasi host query stats incremented");

        let db = db.read().await;
        println!("wasi host query db lock acquired");
        let result = surrealdb_host_adapter::query(&db, query, params)
            .await
            .map_err(wasmtime::Error::new);
        println!("wasi host query adapter returned. ok={}", result.is_ok());
        result
    }

    async fn subscribe<T: Send>(
        accessor: &Accessor<T, Self>,
        query: String,
        params: Vec<(String, Vec<u8>)>,
    ) -> wasmtime::Result<(u64, StreamReader<BindingLiveEvent>)> {
        println!(
            "wasi host subscribe begin. query={}, params_len={}",
            query,
            params.len()
        );
        let (db, subscriptions, call_stats) = accessor.with(|mut access| {
            let state = access.get();
            (
                Arc::clone(&state.db),
                Arc::clone(&state.subscriptions),
                Arc::clone(&state.call_stats),
            )
        });
        println!("wasi host subscribe state captured");
        call_stats.increment_subscribe();
        println!("wasi host subscribe stats incremented");
        let subscription_id = subscriptions.allocate_id();
        println!(
            "wasi host subscribe id allocated. subscription_id={}",
            subscription_id
        );
        let stream = {
            let db = db.read().await;
            println!(
                "wasi host subscribe db lock acquired. subscription_id={}",
                subscription_id
            );
            surrealdb_host_adapter::subscribe(&db, query, params)
                .await
                .map_err(wasmtime::Error::new)?
        };
        println!(
            "wasi host subscribe adapter stream ready. subscription_id={}",
            subscription_id
        );

        let (sender, receiver) = mpsc::unbounded_channel();
        let (stop_tx, mut stop_rx) = oneshot::channel();
        let mut stream = Box::pin(stream);
        let task_subscriptions = Arc::clone(&subscriptions);
        println!(
            "wasi host subscribe task spawn begin. subscription_id={}",
            subscription_id
        );

        let handle = tokio::spawn(async move {
            println!(
                "wasi host subscribe task started. subscription_id={}",
                subscription_id
            );
            loop {
                tokio::select! {
                    _ = &mut stop_rx => {
                        println!(
                            "wasi host subscribe task stop signal. subscription_id={}",
                            subscription_id
                        );
                        break;
                    }
                    notification = stream.next() => {
                        let Some(Ok(notification)) = notification else {
                            println!(
                                "wasi host subscribe task stream ended or errored. subscription_id={}",
                                subscription_id
                            );
                            break;
                        };
                        println!(
                            "wasi host subscribe task notification received. subscription_id={}, action={:?}, query_id={}",
                            subscription_id,
                            notification.action,
                            notification.query_id
                        );

                        let Ok(event) = surrealdb_host_adapter::notification_to_live_event(subscription_id, notification) else {
                            println!(
                                "wasi host subscribe task notification conversion failed. subscription_id={}",
                                subscription_id
                            );
                            break;
                        };
                        println!(
                            "wasi host subscribe task notification converted. subscription_id={}, action={:?}, data_len={}",
                            subscription_id,
                            event.action,
                            event.data.len()
                        );

                        if sender.send(to_binding_live_event(event)).is_err() {
                            println!(
                                "wasi host subscribe task send failed. subscription_id={}",
                                subscription_id
                            );
                            break;
                        }
                        println!(
                            "wasi host subscribe task send complete. subscription_id={}",
                            subscription_id
                        );
                    }
                }
            }

            println!(
                "wasi host subscribe task completing. subscription_id={}",
                subscription_id
            );
            task_subscriptions.complete(subscription_id).await;
            println!(
                "wasi host subscribe task completed. subscription_id={}",
                subscription_id
            );
        });

        println!(
            "wasi host subscribe register begin. subscription_id={}",
            subscription_id
        );
        subscriptions
            .register(subscription_id, SubscriptionTask::new(stop_tx, handle))
            .await;
        println!(
            "wasi host subscribe register complete. subscription_id={}",
            subscription_id
        );

        let reader = accessor.with(|mut access| {
            StreamReader::new(access.as_context_mut(), LiveEventProducer::new(receiver))
        });
        println!(
            "wasi host subscribe reader ready. subscription_id={}",
            subscription_id
        );

        Ok((subscription_id, reader))
    }

    async fn cancel<T: Send>(
        accessor: &Accessor<T, Self>,
        subscription_id: u64,
    ) -> wasmtime::Result<Result<(), String>> {
        println!(
            "wasi host cancel begin. subscription_id={}",
            subscription_id
        );
        let (subscriptions, call_stats) = accessor.with(|mut access| {
            let state = access.get();
            (
                Arc::clone(&state.subscriptions),
                Arc::clone(&state.call_stats),
            )
        });
        println!(
            "wasi host cancel state captured. subscription_id={}",
            subscription_id
        );
        call_stats.increment_cancel();
        println!(
            "wasi host cancel stats incremented. subscription_id={}",
            subscription_id
        );
        if subscriptions.cancel(subscription_id).await {
            println!(
                "wasi host cancel success. subscription_id={}",
                subscription_id
            );
            return Ok(Ok(()));
        }

        println!(
            "wasi host cancel not found. subscription_id={}",
            subscription_id
        );

        Ok(Err(format!("subscription {} not found", subscription_id)))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let component_path = env::args()
        .nth(1)
        .context("Usage: host-wasmtime <path-to-component.wasm>")?;
    println!("host start. component_path={}", component_path);

    let db_url = "memory";
    let db_ns = "example_validation";
    let db_name = "example_validation";
    println!(
        "host config. db_url={}, namespace={}, database={}",
        db_url, db_ns, db_name
    );

    let db: Surreal<Any> = Surreal::init();
    println!("host database connect begin. db_url={}", db_url);
    db.connect(db_url)
        .await
        .context("failed to connect to SurrealDB")?;
    println!("host database connected. db_url={}", db_url);
    println!(
        "host database select begin. namespace={}, database={}",
        db_ns, db_name
    );
    db.use_ns(db_ns)
        .use_db(db_name)
        .await
        .context("failed to select namespace and database")?;
    println!(
        "host database selected. namespace={}, database={}",
        db_ns, db_name
    );

    let adapter = SurrealHostAdapter::new(db);
    println!("host adapter created");

    let mut config = Config::new();
    config.wasm_component_model(true);
    config.wasm_component_model_async(true);
    println!("host wasmtime config ready. component_model=true, async=true");

    let engine = Engine::new(&config)?;
    println!("host engine created");

    let component = Component::from_file(&engine, &component_path)?;
    println!("host component loaded. component_path={}", component_path);

    let mut linker = Linker::new(&engine);
    println!("host linker created");
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
    wasmtime_wasi::p3::add_to_linker(&mut linker)?;
    println!("host wasi interfaces linked");

    bindings::Adapter::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;
    println!("host component bindings linked");

    let mut store = Store::new(&engine, adapter);
    println!("host store created");
    let instance = linker.instantiate_async(&mut store, &component).await?;
    println!("host component instantiated");
    let bindings = bindings::Adapter::new(&mut store, &instance)?;
    println!("host bindings initialized");
    println!("host guest run begin");
    let run_result = store
        .run_concurrent(async |accessor| {
            bindings
                .seamlezz_surrealdb_host_adapter_demo()
                .call_run(accessor)
                .await
        })
        .await??;
    println!("host guest run returned");
    if let Err(message) = run_result {
        bail!("guest demo execution failed: {message}");
    }

    println!("host guest run success");

    println!(
        "Successfully loaded and instantiated component: {}",
        component_path
    );
    println!("Connected to SurrealDB at {}", db_url);
    println!("Using namespace: {}, database: {}", db_ns, db_name);

    store.data().validate_demo_execution()?;
    println!("host runtime validation success");
    println!("Validated guest demo runtime behavior");

    println!("host shutdown begin");
    store.data().shutdown().await;
    println!("host shutdown complete");

    Ok(())
}
