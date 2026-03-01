use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;

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
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn register(&self, subscription_id: u64, task: SubscriptionTask) {
        self.tasks.lock().await.insert(subscription_id, task);
    }

    pub async fn complete(&self, subscription_id: u64) {
        self.tasks.lock().await.remove(&subscription_id);
    }

    pub async fn cancel(&self, subscription_id: u64) -> bool {
        let Some(task) = self.tasks.lock().await.remove(&subscription_id) else {
            return false;
        };

        task.stop().await;
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
