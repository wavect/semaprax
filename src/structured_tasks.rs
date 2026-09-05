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
use std::sync::Arc;

/// Deliberately below the proof model's structural bound: one runtime scope
/// must not create an unbounded number of operating-system threads.
pub const MAX_RUNTIME_TASKS: usize = 64;

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
}

impl fmt::Display for TaskRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentity => f.write_str("task identity must not be empty"),
            Self::IdentityContainsNul => f.write_str("task identity must not contain NUL"),
            Self::DuplicateIdentity(id) => write!(f, "duplicate task identity `{id}`"),
            Self::CapacityExceeded => write!(f, "task scope exceeds {MAX_RUNTIME_TASKS} tasks"),
            Self::ZeroPhysicalFailure => f.write_str("physical task failure code must be nonzero"),
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
}
