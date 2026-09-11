//! Harness for the Core Wasm physical adapter (issue #155). Each subject is
//! a module here, per docs/ARCHITECTURE.md#integration-test-harnesses,
//! mirroring `tests/public_generic_native_adapter_v1.rs`'s own convention
//! for its native sibling; future Wasm-adapter fixtures add modules rather
//! than new top-level files.
#[path = "public_generic_wasm_adapter_v1/fixture.rs"]
mod fixture;
#[path = "public_generic_wasm_adapter_v1/reference_wasm_module.rs"]
mod reference_wasm_module;
#[path = "public_generic_wasm_adapter_v1/typescript_calling_consumer.rs"]
mod typescript_calling_consumer;
