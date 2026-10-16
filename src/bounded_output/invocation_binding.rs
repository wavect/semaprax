//! Exact active ledger identity for one invocation's retained physical backing.
use super::{active, Budget, Rc};
pub(crate) struct InvocationBinding(Option<Rc<Budget>>);
impl InvocationBinding {
    // Caller prepays this header before capture; cloning Rc allocates nothing.
    pub(crate) fn capture() -> Self {
        Self(active())
    }
    pub(crate) fn current(&self) -> bool {
        match (&self.0, active()) {
            (None, None) => true,
            (Some(held), Some(current)) => Rc::ptr_eq(held, &current) && !held.overflowed.get(),
            _ => false,
        }
    }
}
