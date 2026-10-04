//! Unique receiver state. Only invocation borrows the existing cell.
use super::*;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

#[derive(Debug)]
pub(super) struct MutableState {
    value: AtomicI64,
    active: AtomicBool,
    thread: std::thread::ThreadId,
}
impl PartialEq for MutableState {
    fn eq(&self, other: &Self) -> bool {
        self.value.load(Ordering::Acquire) == other.value.load(Ordering::Acquire)
            && self.active.load(Ordering::Acquire) == other.active.load(Ordering::Acquire)
    }
}
impl MutableState {
    pub(super) fn new(value: i64) -> Self {
        Self {
            value: AtomicI64::new(value),
            active: AtomicBool::new(false),
            thread: std::thread::current().id(),
        }
    }
    pub(super) fn snapshot(&self) -> Result<i64, Flow> {
        if self.active.load(Ordering::Acquire) {
            return Err(Flow::Guard("active mutable receiver cannot be inspected"));
        }
        Ok(self.value.load(Ordering::Acquire))
    }
    fn begin(&self) -> Result<MutableCall<'_>, Flow> {
        if self.thread != std::thread::current().id() {
            return Err(Flow::Guard("mutable receiver belongs to another thread"));
        }
        if self
            .active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(Flow::Guard("mutable receiver re-entry"));
        }
        Ok(MutableCall(self))
    }
}
struct MutableCall<'a>(&'a MutableState);
impl MutableCall<'_> {
    fn state(&self) -> i64 {
        self.0.value.load(Ordering::Acquire)
    }
    fn commit(&self, value: i64) {
        self.0.value.store(value, Ordering::Release);
    }
}
impl Drop for MutableCall<'_> {
    fn drop(&mut self) {
        self.0.active.store(false, Ordering::Release);
    }
}

impl Evaluator<'_> {
    pub(super) fn evaluate_mutable_invocation(
        &mut self,
        expression: &ResolvedExpr,
        callable: &ResolvedExpr,
        args: &[ResolvedExpr],
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        let ResolvedExprKind::Place(place) = &callable.kind else {
            return Err(Flow::Guard("mutable receiver must be a direct binding"));
        };
        if !place.projections.is_empty() || args.len() != 1 || expression.ty != ResolvedType::I64 {
            return Err(Flow::Guard("mutable invocation shape"));
        }
        let slot = *environment
            .slots
            .get(&place.root)
            .ok_or(Flow::Guard("mutable receiver absent"))?;
        let Value::Closure(receiver) = &environment.bindings[slot].1 else {
            return Err(Flow::Guard("mutable receiver is not a closure"));
        };
        let receiver = Arc::clone(receiver);
        if receiver.captures.len() != 1 || receiver.parameters.len() != 1 {
            return Err(Flow::Guard("mutable receiver schema"));
        }
        let state = receiver
            .mutable
            .as_ref()
            .ok_or(Flow::Guard("mutable receiver state absent"))?;
        let call = state.begin()?;
        let initial = call.state();
        let argument = self.evaluate(&args[0], environment, depth)?;
        if !matches!(argument, Value::Int(_)) {
            return Err(Flow::Guard("mutable invocation argument"));
        }
        let frame = vec![
            (receiver.captures[0].0.clone(), Value::Int(initial)),
            (receiver.parameters[0].id.clone(), argument),
        ];
        let candidate = self.call_frame(&receiver.function, frame, depth + 1)?;
        let Value::Int(candidate) = candidate else {
            return Err(Flow::Guard("mutable candidate state is not i64"));
        };
        call.commit(candidate);
        Ok(Value::Int(candidate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mutable_state_commits_only_explicit_success_and_releases_failure_guard() {
        let state = MutableState::new(7);
        {
            let failed_call = state.begin().unwrap();
            assert_eq!(failed_call.state(), 7);
            assert!(matches!(
                state.begin(),
                Err(Flow::Guard("mutable receiver re-entry"))
            ));
            assert!(state.snapshot().is_err());
        }
        assert_eq!(state.snapshot().unwrap(), 7);
        {
            let successful_call = state.begin().unwrap();
            successful_call.commit(12);
        }
        assert_eq!(state.snapshot().unwrap(), 12);
    }

    #[test]
    fn mutable_state_rejects_foreign_thread_before_reading_or_committing() {
        let state = Arc::new(MutableState::new(12));
        let foreign = Arc::clone(&state);
        assert!(std::thread::spawn(move || matches!(
            foreign.begin(),
            Err(Flow::Guard("mutable receiver belongs to another thread"))
        ))
        .join()
        .unwrap());
        assert_eq!(state.snapshot().unwrap(), 12);
        assert!(state.begin().is_ok());
    }

    #[test]
    fn mutable_state_guard_releases_during_unwind_without_publishing() {
        let state = MutableState::new(i64::MIN);
        let failure = std::panic::catch_unwind(|| {
            let _call = state.begin().unwrap();
            panic!("private update panic");
        });
        assert!(failure.is_err());
        assert_eq!(state.snapshot().unwrap(), i64::MIN);
        assert!(state.begin().is_ok());
    }
}
