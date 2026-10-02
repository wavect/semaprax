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
#[cfg(test)]
pub(crate) use store::source_v8::TestRetainedSourceOwnedWaitRegistrationV8;
pub(crate) use store::source_v8::{
    fresh_source_owned_wait_v8, prepare_fresh_source_owned_wait_v8, recover_source_owned_wait_v8,
    ExplicitStoreRegistrationGrant, FreshSourceOwnedWaitFactsV8, SourceOwnedWaitLeaseV8,
    SourceOwnedWaitLimitsV8, SourceOwnedWaitStoreRegistrationV8,
};

mod fold;
pub(crate) mod journal;

mod driver;

pub(crate) mod v2;
