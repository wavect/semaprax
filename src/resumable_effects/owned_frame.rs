//! Checked owned-frame plans and the opt-in registered durable interpreter route.
//! Authenticated inert facts carry no ownership or cleanup authority by themselves.
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
