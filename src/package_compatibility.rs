//! Offline stable-ID semantic package compatibility evidence v1.

mod auth;
mod compare;
mod model;
mod wire;

pub use compare::{
    generate, verify, CompatibilityInput, CompatibilityOptions, VerifiedEvidence, MAX_FINDINGS,
    MAX_INPUT_BYTES, MAX_JSON_DEPTH, MAX_OUTPUT_BYTES, MAX_WORK_UNITS, SCHEMA,
};
use compare::{DIGEST_DOMAIN, INPUT_DOMAIN};
