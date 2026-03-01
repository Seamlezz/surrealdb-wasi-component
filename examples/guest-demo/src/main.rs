use anyhow::{Context, Result, anyhow};
use std::future::Future;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll, Wake, Waker};
use surrealdb_component_sdk::{LiveAction, query, subscribe};

fn main() {
    if let Err(error) = block_on(run_demo()) {
        panic!("{error:#}");
    }
}

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    let waker = Waker::from(Arc::new(NoopWaker));
    let mut context = TaskContext::from_waker(&waker);
    let mut future = std::pin::pin!(future);

    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::hint::spin_loop(),
        }
    }
}

struct NoopWaker;

impl Wake for NoopWaker {
    fn wake(self: Arc<Self>) {}
}

pub async fn run_demo() -> Result<()> {
    let mut subscription = subscribe("LIVE SELECT * FROM person WHERE id = $id")
        .bind("id", "person:demo")
        .execute()
        .await?;

    query("CREATE person:demo CONTENT { id: person:demo, name: 'demo', age: 42 }")
        .execute()
        .await?;

    let create_event = subscription
        .next_event()
        .await?
        .context("live stream ended before create event")?;
    let create_payload: serde_json::Value = create_event.parse()?;
    match create_event.action {
        LiveAction::Create => {
            let _ = create_payload;
        }
        action => return Err(anyhow!("unexpected live action for create: {action:?}")),
    }

    query("UPDATE person:demo SET age = 43")
        .execute()
        .await?;

    let update_event = subscription
        .next_event()
        .await?
        .context("live stream ended before update event")?;
    let update_payload: serde_json::Value = update_event.parse()?;
    match update_event.action {
        LiveAction::Update => {
            let _ = update_payload;
        }
        action => return Err(anyhow!("unexpected live action for update: {action:?}")),
    }

    query("DELETE person:demo")
        .execute()
        .await?;

    let delete_event = subscription
        .next_event()
        .await?
        .context("live stream ended before delete event")?;
    let delete_payload: serde_json::Value = delete_event.parse()?;
    match delete_event.action {
        LiveAction::Delete => {
            let _ = delete_payload;
        }
        action => return Err(anyhow!("unexpected live action for delete: {action:?}")),
    }

    subscription.cancel().await?;

    let result = query("SELECT * FROM person WHERE id = $id")
        .bind("id", "person:demo")
        .execute()
        .await?;

    let _rows: Vec<serde_json::Value> = result.parse(0)?;
    Ok(())
}
