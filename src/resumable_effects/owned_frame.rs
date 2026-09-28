//! Compiler/evaluator foundation only; no checkpoint or durable ownership authority.
mod facade;
pub(crate) mod plan;
pub use facade::*;

mod checkpoint;
mod codec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedFrameError {
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

mod store;

mod fold;
pub(crate) mod journal;

mod driver;
