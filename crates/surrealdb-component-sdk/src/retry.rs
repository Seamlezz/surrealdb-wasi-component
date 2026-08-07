use std::{future::Future, num::NonZeroU32, time::Duration};

use crate::QueryError;

/// Bounded retry policy for atomic or idempotent queries that encounter conflicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictRetryPolicy {
    max_attempts: NonZeroU32,
    initial_delay: Duration,
    maximum_delay: Duration,
}

impl ConflictRetryPolicy {
    /// Creates a retry policy with exponential delay capped at `maximum_delay`.
    pub fn new(max_attempts: NonZeroU32, initial_delay: Duration, maximum_delay: Duration) -> Self {
        assert!(
            maximum_delay >= initial_delay,
            "maximum conflict retry delay must not be below the initial delay"
        );
        Self {
            max_attempts,
            initial_delay,
            maximum_delay,
        }
    }

    fn delay_after(&self, attempt: u32) -> Duration {
        let multiplier = 1_u32
            .checked_shl(attempt.saturating_sub(1))
            .unwrap_or(u32::MAX);
        self.initial_delay
            .saturating_mul(multiplier)
            .min(self.maximum_delay)
    }
}

pub(crate) async fn execute_with_conflict_retry<T, Operation, OperationFuture, Delay, DelayFuture>(
    policy: Option<&ConflictRetryPolicy>,
    mut operation: Operation,
    mut delay: Delay,
    conflict_message: impl Fn(&T) -> Option<String>,
) -> Result<(T, NonZeroU32), QueryError>
where
    Operation: FnMut() -> OperationFuture,
    OperationFuture: Future<Output = Result<T, QueryError>>,
    Delay: FnMut(Duration) -> DelayFuture,
    DelayFuture: Future<Output = ()>,
{
    let maximum_attempts = policy.map_or(1, |policy| policy.max_attempts.get());

    for attempt in 1..=maximum_attempts {
        let value = operation().await?;
        let Some(message) = conflict_message(&value) else {
            return Ok((
                value,
                NonZeroU32::new(attempt).expect("attempt starts at one"),
            ));
        };

        if attempt == maximum_attempts {
            return Err(QueryError::ConflictRetryExhausted {
                attempts: NonZeroU32::new(attempt).expect("attempt starts at one"),
                message,
            });
        }

        let policy = policy.expect("multiple attempts require a retry policy");
        delay(policy.delay_after(attempt)).await;
    }

    unreachable!("at least one query attempt is always performed")
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, future::ready, num::NonZeroU32, time::Duration};

    use super::{ConflictRetryPolicy, execute_with_conflict_retry};
    use crate::QueryError;

    fn policy() -> ConflictRetryPolicy {
        ConflictRetryPolicy::new(
            NonZeroU32::new(3).unwrap(),
            Duration::from_millis(10),
            Duration::from_millis(15),
        )
    }

    #[test]
    fn retries_conflicts_and_caps_backoff() {
        let mut responses = VecDeque::from([
            Some("conflict one".to_string()),
            Some("conflict two".to_string()),
            None,
        ]);
        let mut delays = Vec::new();

        let result = futures::executor::block_on(execute_with_conflict_retry(
            Some(&policy()),
            || ready(Ok(responses.pop_front().unwrap())),
            |duration| {
                delays.push(duration);
                ready(())
            },
            Clone::clone,
        ))
        .unwrap();

        assert_eq!(result.1, NonZeroU32::new(3).unwrap());
        assert_eq!(
            delays,
            [Duration::from_millis(10), Duration::from_millis(15)]
        );
    }

    #[test]
    fn reports_exhausted_conflicts() {
        let result = futures::executor::block_on(execute_with_conflict_retry(
            Some(&policy()),
            || ready(Ok::<_, QueryError>(Some("still conflicting".to_string()))),
            |_| ready(()),
            Clone::clone,
        ));

        assert!(matches!(
            result,
            Err(QueryError::ConflictRetryExhausted { attempts, message })
                if attempts == NonZeroU32::new(3).unwrap() && message == "still conflicting"
        ));
    }

    #[test]
    fn does_not_retry_without_an_explicit_policy() {
        let result = futures::executor::block_on(execute_with_conflict_retry(
            None,
            || ready(Ok::<_, QueryError>(Some("conflict".to_string()))),
            |_| ready(()),
            Clone::clone,
        ));

        assert!(matches!(
            result,
            Err(QueryError::ConflictRetryExhausted { attempts, .. })
                if attempts == NonZeroU32::new(1).unwrap()
        ));
    }

    #[test]
    fn propagates_execution_failures() {
        let result =
            futures::executor::block_on(execute_with_conflict_retry::<Option<String>, _, _, _, _>(
                Some(&policy()),
                || {
                    ready(Err(QueryError::Execution {
                        message: "unavailable".into(),
                    }))
                },
                |_| ready(()),
                Clone::clone,
            ));

        assert!(
            matches!(result, Err(QueryError::Execution { message }) if message == "unavailable")
        );
    }
}
