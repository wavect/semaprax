//! Same-receiver transactional state. Semantic copies snapshot into a fresh
//! cell; only the invocation path borrows the receiver's existing cell.
use super::*;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

#[derive(Debug)]
pub(super) struct MutableState {
    value: AtomicI64,
    active: AtomicBool,
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
        }
    }
    pub(super) fn snapshot(&self) -> Result<i64, Flow> {
        if self.active.load(Ordering::Acquire) {
            return Err(Flow::Guard("active mutable receiver cannot be copied"));
        }
        Ok(self.value.load(Ordering::Acquire))
    }
    fn begin(&self) -> Result<MutableCall<'_>, Flow> {
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
    pub(super) fn clone_mutable_closure(
        &mut self,
        closure: &closures::ClosureValue,
    ) -> Result<Value, Flow> {
        let state = closure
            .mutable
            .as_ref()
            .ok_or(Flow::Guard("mutable closure state absent"))?
            .snapshot()?;
        let captures = closure
            .captures
            .iter()
            .map(|(id, value)| Ok((id.clone(), self.clone_value(value)?)))
            .collect::<Result<Vec<_>, Flow>>()?;
        Ok(Value::Closure(Arc::new(closures::ClosureValue {
            target: closure.target.clone(),
            parameters: closure.parameters.clone(),
            captures,
            function: closure.function.clone(),
            result: closure.result.clone(),
            mutable: Some(MutableState::new(state)),
        })))
    }

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
        let argument = self.evaluate(&args[0], environment, depth)?;
        if !matches!(argument, Value::Int(_)) {
            return Err(Flow::Guard("mutable invocation argument"));
        }
        // The operand identifies storage before evaluating its argument. Read
        // its current receiver after that evaluation, as native and Wasm do
        // through their saved address (the argument can replace the binding).
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
        let frame = vec![
            (receiver.captures[0].0.clone(), Value::Int(call.state())),
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
        let copied = MutableState::new(state.snapshot().unwrap());
        {
            let call = state.begin().unwrap();
            call.commit(99);
        }
        assert_eq!(copied.snapshot().unwrap(), 12);
        assert_eq!(state.snapshot().unwrap(), 99);
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
