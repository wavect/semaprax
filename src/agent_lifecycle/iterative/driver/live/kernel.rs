//! One feedback control loop over closed, consuming stage carriers. This module
//! owns no evaluator, reservation ledger, phase constructor or publication API.
//! Route errors must retain their actual phase owner; returning an unrelated
//! generic error after acquiring an owned phase is not an admitted route.
use super::super::MAX_PROPOSAL_ATTEMPTS;

pub(super) enum Flow<T, O> {
    Advance(T),
    Stopped(O),
}
pub(super) enum Proposal<O, P, R> {
    Accepted(P),
    Rejected(O),
    Stopped(R),
}
pub(super) enum Transition<S, O> {
    Continue(S),
    Stopped(O),
}
/// Private to the live implementation. Each operation consumes the preceding
/// carrier, including limit/failure exits; no Clone bound or owner extraction.
pub(super) trait Route {
    type State;
    type Observed;
    type Proposed;
    type Granted;
    type Executed;
    type Step;
    type Output;
    type Failure;
    fn initialize(&mut self) -> Result<Flow<Self::State, Self::Output>, Self::Failure>;
    fn begin_turn(
        &mut self,
        state: Self::State,
    ) -> Result<Flow<Self::State, Self::Output>, Self::Failure>;
    fn observe(
        &mut self,
        state: Self::State,
    ) -> Result<Flow<Self::Observed, Self::Output>, Self::Failure>;
    fn propose(
        &mut self,
        observed: Self::Observed,
        attempt: usize,
        rejection: Option<&str>,
    ) -> Result<Proposal<Self::Observed, Self::Proposed, Self::Output>, Self::Failure>;
    fn attempt_limit(&mut self, observed: Self::Observed) -> Result<Self::Output, Self::Failure>;
    fn authorize(
        &mut self,
        proposed: Self::Proposed,
        attempt: usize,
    ) -> Result<Flow<Self::Granted, Self::Output>, Self::Failure>;
    fn effect(
        &mut self,
        granted: Self::Granted,
        attempt: usize,
    ) -> Result<Flow<Self::Executed, Self::Output>, Self::Failure>;
    fn reduce(
        &mut self,
        executed: Self::Executed,
        attempt: usize,
    ) -> Result<Flow<Self::Step, Self::Output>, Self::Failure>;
    fn transition(
        &mut self,
        step: Self::Step,
        attempt: usize,
    ) -> Result<Transition<Self::State, Self::Output>, Self::Failure>;
}
pub(super) fn run<R: Route>(mut route: R) -> Result<R::Output, R::Failure> {
    macro_rules! advance {
        ($phase:expr) => {
            match $phase? {
                Flow::Advance(value) => value,
                Flow::Stopped(output) => return Ok(output),
            }
        };
    }
    let mut state = advance!(route.initialize());
    loop {
        state = advance!(route.begin_turn(state));
        let mut observed = advance!(route.observe(state));
        let mut attempt = 0usize;
        let mut rejection: Option<String> = None;
        let proposed = loop {
            match route.propose(observed, attempt, rejection.as_deref())? {
                Proposal::Accepted(proposed) => break proposed,
                Proposal::Stopped(output) => return Ok(output),
                Proposal::Rejected(retained) => {
                    observed = retained;
                    attempt += 1;
                    if attempt >= MAX_PROPOSAL_ATTEMPTS {
                        return route.attempt_limit(observed);
                    }
                    rejection = Some(format!("proposal.decode.attempt.{attempt}"));
                }
            }
        };
        let granted = advance!(route.authorize(proposed, attempt));
        let executed = advance!(route.effect(granted, attempt));
        let step = advance!(route.reduce(executed, attempt));
        match route.transition(step, attempt)? {
            Transition::Continue(next) => state = next,
            Transition::Stopped(output) => return Ok(output),
        }
    }
}

#[cfg(test)]
mod tests;
