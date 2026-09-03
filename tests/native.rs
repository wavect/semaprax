//! Native backend products: the status ABI, callable bundles, byte-call
//! staging, and owned/ordinary string and tuple settlement.

#[path = "native/bytes_call_staging.rs"]
mod bytes_call_staging;
#[path = "native/callable_bundle.rs"]
mod callable_bundle;
#[path = "native/owned_data_string_settlement.rs"]
mod owned_data_string_settlement;
#[path = "native/owned_tuple_admission.rs"]
mod owned_tuple_admission;
#[path = "native/owned_utf8_settlement.rs"]
mod owned_utf8_settlement;
#[path = "native/status_abi.rs"]
mod status_abi;
#[path = "native/string_settlement.rs"]
mod string_settlement;
