//! Internal offline root materialization, not a launch or profile admission API.
//!
//! The future caller must supply an already controlled child context. In
//! particular, this module never closes inherited descriptors, changes a live
//! embedding process's root, or launches a tool. General-host bootstrap remains
//! unwired because releasing inherited descriptors can dispatch foreign flushes.

mod plan;
use plan::Plan;

mod linux;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Error {
    Invalid,
    Limit,
    Allocation,
    Io,
}
