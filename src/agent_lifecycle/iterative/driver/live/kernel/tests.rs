//! Control/carrier gates only; actual retained lifecycle behavior is exercised
//! by the existing driver harness. These tokens grant no source/ACK authority.
use super::*;
use std::cell::RefCell;
use std::rc::{Rc, Weak};
struct Token {
    _root: Rc<()>,
    turn: usize,
}
struct Output {
    token: Token,
    status: &'static str,
}
struct Failure {
    _token: Token,
}
struct Trace {
    initial: Option<Token>,
    events: Rc<RefCell<Vec<String>>>,
    reject_all: bool,
    fail_effect: bool,
}
impl Trace {
    fn event(&self, value: impl Into<String>) {
        self.events.borrow_mut().push(value.into());
    }
}
impl Route for Trace {
    type State = Token;
    type Observed = Token;
    type Proposed = Token;
    type Granted = Token;
    type Executed = Token;
    type Step = Token;
    type Output = Output;
    type Failure = Failure;
    fn initialize(&mut self) -> Result<Flow<Token, Output>, Failure> {
        self.event("initialize");
        Ok(Flow::Advance(self.initial.take().unwrap()))
    }
    fn begin_turn(&mut self, token: Token) -> Result<Flow<Token, Output>, Failure> {
        self.event(format!("turn:{}", token.turn));
        Ok(Flow::Advance(token))
    }
    fn observe(&mut self, token: Token) -> Result<Flow<Token, Output>, Failure> {
        self.event(format!("observe:{}", token.turn));
        Ok(Flow::Advance(token))
    }
    fn propose(
        &mut self,
        token: Token,
        attempt: usize,
        rejection: Option<&str>,
    ) -> Result<Proposal<Token, Token, Output>, Failure> {
        self.event(format!(
            "propose:{}:{attempt}:{}",
            token.turn,
            rejection.unwrap_or("none")
        ));
        if self.reject_all || (token.turn == 0 && attempt == 0) {
            Ok(Proposal::Rejected(token))
        } else {
            Ok(Proposal::Accepted(token))
        }
    }
    fn attempt_limit(&mut self, token: Token) -> Result<Output, Failure> {
        self.event("attempt-limit");
        Ok(Output {
            token,
            status: "model-failed",
        })
    }
    fn authorize(&mut self, token: Token, attempt: usize) -> Result<Flow<Token, Output>, Failure> {
        self.event(format!("authorize:{}:{attempt}", token.turn));
        Ok(Flow::Advance(token))
    }
    fn effect(&mut self, token: Token, attempt: usize) -> Result<Flow<Token, Output>, Failure> {
        self.event(format!("effect:{}:{attempt}", token.turn));
        if self.fail_effect {
            Err(Failure { _token: token })
        } else {
            Ok(Flow::Advance(token))
        }
    }
    fn reduce(&mut self, token: Token, attempt: usize) -> Result<Flow<Token, Output>, Failure> {
        self.event(format!("reduce:{}:{attempt}", token.turn));
        Ok(Flow::Advance(token))
    }
    fn transition(
        &mut self,
        mut token: Token,
        attempt: usize,
    ) -> Result<Transition<Token, Output>, Failure> {
        self.event(format!("transition:{}:{attempt}", token.turn));
        if token.turn == 0 {
            token.turn += 1;
            Ok(Transition::Continue(token))
        } else {
            Ok(Transition::Stopped(Output {
                token,
                status: "complete",
            }))
        }
    }
}
fn trace() -> (Trace, Weak<()>, Rc<RefCell<Vec<String>>>) {
    let root = Rc::new(());
    let weak = Rc::downgrade(&root);
    let events = Rc::new(RefCell::new(Vec::new()));
    (
        Trace {
            initial: Some(Token {
                _root: root,
                turn: 0,
            }),
            events: events.clone(),
            reject_all: false,
            fail_effect: false,
        },
        weak,
        events,
    )
}
#[test]
fn live_kernel_consumes_nonclone_phase_through_retry_and_continue_once() {
    let (route, weak, events) = trace();
    let output = run(route).unwrap_or_else(|_| panic!("control success"));
    assert_eq!(output.status, "complete");
    assert_eq!(output.token.turn, 1);
    assert_eq!(weak.strong_count(), 1);
    assert_eq!(
        *events.borrow(),
        vec![
            "initialize",
            "turn:0",
            "observe:0",
            "propose:0:0:none",
            "propose:0:1:proposal.decode.attempt.1",
            "authorize:0:1",
            "effect:0:1",
            "reduce:0:1",
            "transition:0:1",
            "turn:1",
            "observe:1",
            "propose:1:0:none",
            "authorize:1:0",
            "effect:1:0",
            "reduce:1:0",
            "transition:1:0"
        ]
    );
    drop(output);
    assert!(weak.upgrade().is_none());
}
#[test]
fn live_kernel_failed_phase_retains_token_and_does_not_enter_later_stage() {
    let (mut route, weak, events) = trace();
    route.fail_effect = true;
    let failure = run(route).err().expect("typed failure");
    assert_eq!(weak.strong_count(), 1);
    assert_eq!(events.borrow().last().unwrap(), "effect:0:1");
    assert!(!events
        .borrow()
        .iter()
        .any(|event| event.starts_with("reduce:")));
    drop(failure);
    assert!(weak.upgrade().is_none());
}
#[test]
fn live_kernel_attempt_limit_keeps_observation_and_has_zero_later_stage() {
    let (mut route, weak, events) = trace();
    route.reject_all = true;
    let output = run(route).unwrap_or_else(|_| panic!("bounded refusal"));
    assert_eq!(output.status, "model-failed");
    assert_eq!(weak.strong_count(), 1);
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| event.starts_with("propose:"))
            .count(),
        MAX_PROPOSAL_ATTEMPTS
    );
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| event.starts_with("observe:"))
            .count(),
        1
    );
    assert!(!events
        .borrow()
        .iter()
        .any(|event| event.starts_with("authorize:")));
    drop(output);
    assert!(weak.upgrade().is_none());
}
