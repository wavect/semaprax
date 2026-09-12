# live_invocation/budget.rs

- tests · module · L110-L110 — mod tests;
- BUDGET_EXHAUSTED · constant · L116-L116 — pub const BUDGET_EXHAUSTED: &str = "budget_exhausted";
- DEADLINE_EXCEEDED · constant · L117-L117 — pub const DEADLINE_EXCEEDED: &str = "deadline_exceeded";
- NEGATIVE_REQUEST · constant · L118-L118 — pub const NEGATIVE_REQUEST: &str = "negative_request";
- InvocationClock · interface · L126-L128 — pub trait InvocationClock
- now_millis · function · L127-L127 — fn now_millis(&self) -> i64;
- CumulativeBudgetLedger · struct · L135-L141 — pub struct CumulativeBudgetLedger<'a>
- new · function · L146-L154 — pub fn new(ceiling: i64, clock: &'a mut dyn InvocationClock) -> Self
- with_deadline · function · L159-L171 — pub fn with_deadline(
- resume · function · L181-L203 — pub fn resume(
- committed · function · L209-L211 — pub fn committed(&self) -> i64
- remaining · function · L217-L219 — pub fn remaining(&self) -> i64
- usage · function · L225-L227 — pub fn usage(&self) -> &[InvocationUsage]
- reserve · function · L231-L254 — fn reserve(
- record · function · L256-L260 — fn record(&mut self, usage: &InvocationUsage)
