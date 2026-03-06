use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;

#[cfg(feature = "debug-logs")]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        println!($($arg)*);
    };
}

#[cfg(not(feature = "debug-logs"))]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        {};
    };
}

pub struct SubscriptionTask {
    stop_tx: oneshot::Sender<()>,
    handle: JoinHandle<()>,
}

impl SubscriptionTask {
    pub fn new(stop_tx: oneshot::Sender<()>, handle: JoinHandle<()>) -> Self {
        Self { stop_tx, handle }
    }

    async fn stop(self) {
        let _ = self.stop_tx.send(());
        self.handle.abort();
        let _ = self.handle.await;
    }
}

pub struct SubscriptionManager {
    next_id: AtomicU64,
    tasks: Mutex<HashMap<u64, SubscriptionTask>>,
}

impl SubscriptionManager {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            tasks: Mutex::new(HashMap::new()),
        }
    }

    pub fn allocate_id(&self) -> u64 {
        let subscription_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        debug_log!(
            "subscription manager allocate id. subscription_id={}",
            subscription_id
        );
        subscription_id
    }

    pub async fn register(&self, subscription_id: u64, task: SubscriptionTask) {
        debug_log!(
            "subscription manager register begin. subscription_id={}",
            subscription_id
        );
        self.tasks.lock().await.insert(subscription_id, task);
        debug_log!(
            "subscription manager register complete. subscription_id={}",
            subscription_id
        );
    }

    pub async fn complete(&self, subscription_id: u64) {
        debug_log!(
            "subscription manager complete begin. subscription_id={}",
            subscription_id
        );
        self.tasks.lock().await.remove(&subscription_id);
        debug_log!(
            "subscription manager complete done. subscription_id={}",
            subscription_id
        );
    }

    pub async fn cancel(&self, subscription_id: u64) -> bool {
        debug_log!(
            "subscription manager cancel begin. subscription_id={}",
            subscription_id
        );
        let Some(task) = self.tasks.lock().await.remove(&subscription_id) else {
            debug_log!(
                "subscription manager cancel missing. subscription_id={}",
                subscription_id
            );
            return false;
        };

        debug_log!(
            "subscription manager cancel task found. subscription_id={}",
            subscription_id
        );

        task.stop().await;
        debug_log!(
            "subscription manager cancel complete. subscription_id={}",
            subscription_id
        );
        true
    }

    pub async fn shutdown(&self) {
        let tasks = {
            let mut guard = self.tasks.lock().await;
            guard.drain().map(|(_, task)| task).collect::<Vec<_>>()
        };

        for task in tasks {
            task.stop().await;
        }
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}
