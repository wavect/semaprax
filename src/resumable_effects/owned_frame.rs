//! Compiler/evaluator foundation only; no checkpoint or durable ownership authority.
pub(crate) mod plan;
pub(crate) use plan::{compile_owned_frame_plan, CheckedOwnedFramePlan};

mod checkpoint;
mod codec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OwnedFrameError {
    Malformed,
    Authentication,
    Binding,
    Capacity,
    Fuel,
    Storage,
    Busy,
    InDoubt,
    Policy,
    UnsupportedStore,
}
