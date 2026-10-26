use std::{num::NonZeroU32, time::Duration};

use anyhow::{Context, Result, ensure};
use surrealdb_component_sdk::{ConflictRetryPolicy, query};

/// Writes the increment, then holds the transaction open so the other one overlaps it.
const INCREMENT: &str = "
    BEGIN TRANSACTION;
    UPDATE counter_retry:shared SET count += 1;
    SLEEP 50ms;
    COMMIT TRANSACTION;
";

fn policy() -> ConflictRetryPolicy {
    ConflictRetryPolicy::new(
        NonZeroU32::new(5).expect("attempt count is nonzero"),
        Duration::from_millis(10),
        Duration::from_millis(40),
    )
}

/// Two increments race inside this async export. The loser of the conflict waits out its backoff
/// on the guest clock and retries, so both complete and neither increment is lost.
pub async fn run() -> Result<()> {
    query("DEFINE TABLE counter_retry SCHEMALESS")
        .execute()
        .await?;
    query("CREATE counter_retry:shared SET count = 0")
        .execute()
        .await?;

    let increment = || query(INCREMENT).retry_conflicts(policy()).execute();
    let (first, second) = futures::join!(increment(), increment());
    let (first, second) = (
        first.context("first increment")?,
        second.context("second increment")?,
    );

    ensure!(
        first.attempts().get() + second.attempts().get() > 2,
        "the increments never conflicted, so no retry ran: attempts {} and {}",
        first.attempts(),
        second.attempts()
    );
    let count: i64 = query("SELECT VALUE count FROM ONLY counter_retry:shared")
        .execute()
        .await?
        .parse(0)?;
    ensure!(count == 2, "expected count 2, found {count}");
    Ok(())
}
