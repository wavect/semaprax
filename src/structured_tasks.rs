//! Bounded structured task runtime.
//!
//! Tasks are registered inside a lexical `std::thread::scope`, started in
//! stable-identity order, cooperatively cancelled after the first observed
//! failure, and joined before the scope returns. Borrowed captures are allowed
//! because no worker can outlive the scope.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Deliberately below the proof model's structural bound: one runtime scope
/// must not create an unbounded number of operating-system threads.
pub const MAX_RUNTIME_TASKS: usize = 64;
/// A structured HTTPS task always drains, but its result is discarded after
/// this maximum caller-selected deadline.
pub const MAX_HTTPS_TASK_DEADLINE: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn cancellation_point(&self) -> Result<(), TaskFailure> {
        if self.is_cancelled() {
            Err(TaskFailure::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskFailure {
    Semantic(String),
    Physical(u32),
    Cancelled,
    DeadlineExceeded,
    Panicked,
}

impl TaskFailure {
    pub fn physical(code: u32) -> Result<Self, TaskRuntimeError> {
        if code == 0 {
            Err(TaskRuntimeError::ZeroPhysicalFailure)
        } else {
            Ok(Self::Physical(code))
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskResult {
    pub id: String,
    pub outcome: Result<(), TaskFailure>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskReport {
    pub tasks: Vec<TaskResult>,
    pub first_failure: Option<(String, TaskFailure)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskRuntimeError {
    EmptyIdentity,
    IdentityContainsNul,
    DuplicateIdentity(String),
    CapacityExceeded,
    ZeroPhysicalFailure,
    InvalidDeadline,
    OutputPoisoned,
}

impl fmt::Display for TaskRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentity => f.write_str("task identity must not be empty"),
            Self::IdentityContainsNul => f.write_str("task identity must not contain NUL"),
            Self::DuplicateIdentity(id) => write!(f, "duplicate task identity `{id}`"),
            Self::CapacityExceeded => write!(f, "task scope exceeds {MAX_RUNTIME_TASKS} tasks"),
            Self::ZeroPhysicalFailure => f.write_str("physical task failure code must be nonzero"),
            Self::InvalidDeadline => f.write_str("HTTPS task deadline must be within 1ns..=30s"),
            Self::OutputPoisoned => f.write_str("HTTPS task output slot is poisoned"),
        }
    }
}

impl std::error::Error for TaskRuntimeError {}

type TaskBody<'scope> =
    Box<dyn FnOnce(CancellationToken) -> Result<(), TaskFailure> + Send + 'scope>;

pub struct TaskScope<'scope, 'env> {
    scope: &'scope std::thread::Scope<'scope, 'env>,
    cancellation: CancellationToken,
    identities: BTreeSet<String>,
    pending: Vec<(String, TaskBody<'scope>)>,
}

/// Result slot for one structured HTTPS task. The task publishes only after
/// its provider has settled and before its joined result becomes observable.
#[derive(Debug, Default)]
pub struct HttpsTaskOutput {
    result: Mutex<Option<Result<Vec<u8>, crate::network_provider::HttpFailure>>>,
}

impl HttpsTaskOutput {
    pub fn take(
        &self,
    ) -> Result<Option<Result<Vec<u8>, crate::network_provider::HttpFailure>>, TaskRuntimeError>
    {
        self.result
            .lock()
            .map_err(|_| TaskRuntimeError::OutputPoisoned)
            .map(|mut value| value.take())
    }
}

struct NetworkProviderGuard<P: crate::network_provider::NetworkProvider> {
    provider: Option<P>,
}

impl<P: crate::network_provider::NetworkProvider> NetworkProviderGuard<P> {
    fn provider(&mut self) -> &mut P {
        self.provider.as_mut().expect("provider settles once")
    }

    fn settle(&mut self) {
        if let Some(mut provider) = self.provider.take() {
            provider.settle();
        }
    }
}

impl<P: crate::network_provider::NetworkProvider> Drop for NetworkProviderGuard<P> {
    fn drop(&mut self) {
        self.settle();
    }
}

impl<'scope, 'env> TaskScope<'scope, 'env> {
    pub fn spawn<F>(&mut self, id: impl Into<String>, body: F) -> Result<(), TaskRuntimeError>
    where
        F: FnOnce(CancellationToken) -> Result<(), TaskFailure> + Send + 'scope,
    {
        let id = id.into();
        if id.is_empty() {
            return Err(TaskRuntimeError::EmptyIdentity);
        }
        if id.as_bytes().contains(&0) {
            return Err(TaskRuntimeError::IdentityContainsNul);
        }
        if self.pending.len() == MAX_RUNTIME_TASKS {
            return Err(TaskRuntimeError::CapacityExceeded);
        }
        if !self.identities.insert(id.clone()) {
            return Err(TaskRuntimeError::DuplicateIdentity(id));
        }
        self.pending.push((id, Box::new(body)));
        Ok(())
    }

    pub fn cancel(&self) {
        self.cancellation.0.store(true, Ordering::Release);
    }

    /// Spawn one invocation-owned HTTPS request. The provider is moved into
    /// the task, settled exactly once on success, failure, or panic, and never
    /// outlives the lexical scope. Cancellation is checked before transport;
    /// started blocking I/O drains. A response that completes after `deadline`
    /// is discarded and reported as `DeadlineExceeded`.
    pub fn spawn_https_get<P>(
        &mut self,
        id: impl Into<String>,
        mut provider: P,
        url: impl Into<String>,
        max: usize,
        deadline: Duration,
        output: &'scope HttpsTaskOutput,
    ) -> Result<(), TaskRuntimeError>
    where
        P: crate::network_provider::NetworkProvider + Send + 'scope,
    {
        if deadline.is_zero() || deadline > MAX_HTTPS_TASK_DEADLINE {
            provider.settle();
            return Err(TaskRuntimeError::InvalidDeadline);
        }
        let url = url.into();
        let provider = NetworkProviderGuard {
            provider: Some(provider),
        };
        self.spawn(id, move |token| {
            let mut provider = provider;
            token.cancellation_point()?;
            let started = Instant::now();
            let result = provider.provider().https_get(&url, max);
            provider.settle();
            if started.elapsed() > deadline {
                return Err(TaskFailure::DeadlineExceeded);
            }
            let task_outcome = result
                .as_ref()
                .map(|_| ())
                .map_err(|failure| TaskFailure::Physical(failure.status_code()));
            *output.result.lock().map_err(|_| TaskFailure::Panicked)? = Some(result);
            task_outcome
        })
    }

    fn finish(mut self) -> TaskReport {
        self.pending.sort_by(|left, right| left.0.cmp(&right.0));
        let mut handles = Vec::with_capacity(self.pending.len());
        for (id, body) in self.pending {
            let token = self.cancellation.clone();
            handles.push((
                id,
                self.scope.spawn(move || {
                    let result = body(token.clone());
                    if result.is_err() {
                        token.0.store(true, Ordering::Release);
                    }
                    result
                }),
            ));
        }
        let mut tasks = Vec::with_capacity(handles.len());
        let mut first_failure = None;
        for (id, handle) in handles {
            let outcome = handle.join().unwrap_or(Err(TaskFailure::Panicked));
            if first_failure.is_none() {
                if let Err(failure) = &outcome {
                    first_failure = Some((id.clone(), failure.clone()));
                }
            }
            tasks.push(TaskResult { id, outcome });
        }
        TaskReport {
            tasks,
            first_failure,
        }
    }
}

/// Run one lexical task scope. Every registered task is joined, including
/// after sibling failure; therefore borrowed captures cannot escape.
pub fn task_scope<'env, F, R>(body: F) -> Result<(R, TaskReport), TaskRuntimeError>
where
    F: for<'scope> FnOnce(&mut TaskScope<'scope, 'env>) -> Result<R, TaskRuntimeError>,
{
    std::thread::scope(|scope| {
        let mut tasks = TaskScope {
            scope,
            cancellation: CancellationToken(Arc::new(AtomicBool::new(false))),
            identities: BTreeSet::new(),
            pending: Vec::new(),
        };
        let value = body(&mut tasks)?;
        Ok((value, tasks.finish()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone)]
    struct HttpsProvider {
        result: Result<Vec<u8>, crate::network_provider::HttpFailure>,
        settlements: Arc<AtomicUsize>,
        delay: Duration,
        panic: bool,
    }

    impl crate::network_provider::NetworkProvider for HttpsProvider {
        fn https_get(
            &mut self,
            _url: &str,
            _max: usize,
        ) -> Result<Vec<u8>, crate::network_provider::HttpFailure> {
            if self.panic {
                panic!("provider panic");
            }
            std::thread::sleep(self.delay);
            self.result.clone()
        }

        fn connect(
            &mut self,
            _host: &str,
            _port: u16,
        ) -> Result<
            crate::network_provider::ProviderConnection,
            crate::network_provider::NetworkFailure,
        > {
            Err(crate::network_provider::NetworkFailure::AuthorityDenied)
        }

        fn send(
            &mut self,
            _connection: crate::network_provider::ProviderConnection,
            _bytes: &[u8],
        ) -> Result<usize, crate::network_provider::NetworkFailure> {
            Err(crate::network_provider::NetworkFailure::AuthorityDenied)
        }

        fn recv(
            &mut self,
            _connection: crate::network_provider::ProviderConnection,
            _max: usize,
        ) -> Result<Vec<u8>, crate::network_provider::NetworkFailure> {
            Err(crate::network_provider::NetworkFailure::AuthorityDenied)
        }

        fn wait(
            &mut self,
            _connection: crate::network_provider::ProviderConnection,
            _timeout_ms: u32,
        ) -> Result<crate::network_provider::WaitState, crate::network_provider::NetworkFailure>
        {
            Err(crate::network_provider::NetworkFailure::AuthorityDenied)
        }

        fn close(
            &mut self,
            _connection: crate::network_provider::ProviderConnection,
        ) -> Result<(), crate::network_provider::NetworkFailure> {
            Err(crate::network_provider::NetworkFailure::AuthorityDenied)
        }

        fn settle(&mut self) {
            self.settlements.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn https_provider(
        result: Result<Vec<u8>, crate::network_provider::HttpFailure>,
        settlements: Arc<AtomicUsize>,
    ) -> HttpsProvider {
        HttpsProvider {
            result,
            settlements,
            delay: Duration::ZERO,
            panic: false,
        }
    }

    #[test]
    fn borrowed_work_is_joined_and_reported_in_stable_order() {
        let input = [2usize, 3, 5];
        let total = AtomicUsize::new(0);
        let (_, report) = task_scope(|scope| {
            scope.spawn("z", |_| {
                total.fetch_add(input[2], Ordering::SeqCst);
                Ok(())
            })?;
            scope.spawn("a", |_| {
                total.fetch_add(input[0] + input[1], Ordering::SeqCst);
                Ok(())
            })?;
            Ok(())
        })
        .unwrap();
        assert_eq!(total.load(Ordering::SeqCst), 10);
        assert_eq!(
            report
                .tasks
                .iter()
                .map(|task| task.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "z"]
        );
        assert_eq!(report.first_failure, None);
    }

    #[test]
    fn failure_cancels_siblings_cooperatively_and_is_sticky() {
        let (_, report) = task_scope(|scope| {
            scope.spawn("a", |_| Err(TaskFailure::Semantic("boom".into())))?;
            scope.spawn("b", |token| token.cancellation_point())?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            report.first_failure,
            Some(("a".into(), TaskFailure::Semantic("boom".into())))
        );
        assert_eq!(report.tasks.len(), 2);
    }

    #[test]
    fn duplicate_and_zero_physical_failure_are_rejected() {
        let error = task_scope(|scope| {
            scope.spawn("same", |_| Ok(()))?;
            scope.spawn("same", |_| Ok(()))?;
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error, TaskRuntimeError::DuplicateIdentity("same".into()));
        assert_eq!(
            TaskFailure::physical(0),
            Err(TaskRuntimeError::ZeroPhysicalFailure)
        );
    }

    #[test]
    fn https_task_publishes_only_after_exact_provider_settlement() {
        let settlements = Arc::new(AtomicUsize::new(0));
        let output = HttpsTaskOutput::default();
        let (_, report) = task_scope(|scope| {
            scope.spawn_https_get(
                "fetch",
                https_provider(Ok(b"response".to_vec()), Arc::clone(&settlements)),
                "https://example.test/",
                1024,
                Duration::from_secs(1),
                &output,
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(report.first_failure, None);
        assert_eq!(output.take().unwrap(), Some(Ok(b"response".to_vec())));
        assert_eq!(settlements.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn https_failure_is_sticky_and_deadline_discards_a_drained_response() {
        let failure_settlements = Arc::new(AtomicUsize::new(0));
        let failure_output = HttpsTaskOutput::default();
        let (_, failed) = task_scope(|scope| {
            scope.spawn_https_get(
                "a",
                https_provider(
                    Err(crate::network_provider::HttpFailure::TransportFailed),
                    Arc::clone(&failure_settlements),
                ),
                "https://example.test/",
                1024,
                Duration::from_secs(1),
                &failure_output,
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            failed.first_failure,
            Some(("a".into(), TaskFailure::Physical(3)))
        );
        assert_eq!(
            failure_output.take().unwrap(),
            Some(Err(crate::network_provider::HttpFailure::TransportFailed))
        );
        assert_eq!(failure_settlements.load(Ordering::SeqCst), 1);

        let deadline_settlements = Arc::new(AtomicUsize::new(0));
        let deadline_output = HttpsTaskOutput::default();
        let mut provider = https_provider(Ok(b"late".to_vec()), Arc::clone(&deadline_settlements));
        provider.delay = Duration::from_millis(10);
        let (_, expired) = task_scope(|scope| {
            scope.spawn_https_get(
                "late",
                provider,
                "https://example.test/",
                1024,
                Duration::from_millis(1),
                &deadline_output,
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            expired.first_failure,
            Some(("late".into(), TaskFailure::DeadlineExceeded))
        );
        assert_eq!(deadline_output.take().unwrap(), None);
        assert_eq!(deadline_settlements.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn cancelled_panicked_and_unregistered_https_tasks_still_settle() {
        let cancelled_settlements = Arc::new(AtomicUsize::new(0));
        let cancelled_output = HttpsTaskOutput::default();
        let (_, cancelled) = task_scope(|scope| {
            scope.spawn_https_get(
                "cancelled",
                https_provider(Ok(Vec::new()), Arc::clone(&cancelled_settlements)),
                "https://example.test/",
                1024,
                Duration::from_secs(1),
                &cancelled_output,
            )?;
            scope.cancel();
            Ok(())
        })
        .unwrap();
        assert_eq!(cancelled.tasks[0].outcome, Err(TaskFailure::Cancelled));
        assert_eq!(cancelled_output.take().unwrap(), None);
        assert_eq!(cancelled_settlements.load(Ordering::SeqCst), 1);

        let panic_settlements = Arc::new(AtomicUsize::new(0));
        let panic_output = HttpsTaskOutput::default();
        let mut provider = https_provider(Ok(Vec::new()), Arc::clone(&panic_settlements));
        provider.panic = true;
        let (_, panicked) = task_scope(|scope| {
            scope.spawn_https_get(
                "panic",
                provider,
                "https://example.test/",
                1024,
                Duration::from_secs(1),
                &panic_output,
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(panicked.tasks[0].outcome, Err(TaskFailure::Panicked));
        assert_eq!(panic_output.take().unwrap(), None);
        assert_eq!(panic_settlements.load(Ordering::SeqCst), 1);

        let rejected_settlements = Arc::new(AtomicUsize::new(0));
        let rejected_output = HttpsTaskOutput::default();
        let error = task_scope(|scope| {
            scope.spawn_https_get(
                "",
                https_provider(Ok(Vec::new()), Arc::clone(&rejected_settlements)),
                "https://example.test/",
                1024,
                Duration::from_secs(1),
                &rejected_output,
            )?;
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error, TaskRuntimeError::EmptyIdentity);
        assert_eq!(rejected_settlements.load(Ordering::SeqCst), 1);

        let deadline_settlements = Arc::new(AtomicUsize::new(0));
        let deadline_output = HttpsTaskOutput::default();
        let error = task_scope(|scope| {
            scope.spawn_https_get(
                "deadline",
                https_provider(Ok(Vec::new()), Arc::clone(&deadline_settlements)),
                "https://example.test/",
                1024,
                Duration::ZERO,
                &deadline_output,
            )?;
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error, TaskRuntimeError::InvalidDeadline);
        assert_eq!(deadline_settlements.load(Ordering::SeqCst), 1);
        assert_eq!(deadline_output.take().unwrap(), None);
    }
}
