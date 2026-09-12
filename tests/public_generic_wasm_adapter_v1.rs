//! Harness for the Core Wasm physical adapter (issue #155). Each subject is
//! a module here, per docs/ARCHITECTURE.md#integration-test-harnesses,
//! mirroring `tests/public_generic_native_adapter_v1.rs`'s own convention
//! for its native sibling; future Wasm-adapter fixtures add modules rather
//! than new top-level files.
/// Issue #229: real, Node-executed evidence that this compiler's own Wasm
/// backend can genuinely compile (not hand-assemble) a `.wasm` artifact
/// whose own bytecode performs the fixture endpoint's byte-reversal
/// computation, plus a precise accounting of what still blocks compiling
/// the full open/input_prepare/call/result_export/release provider ABI.
#[path = "public_generic_wasm_adapter_v1/compiled_reference_endpoint.rs"]
mod compiled_reference_endpoint;
#[path = "public_generic_wasm_adapter_v1/fixture.rs"]
mod fixture;
#[path = "public_generic_wasm_adapter_v1/reference_wasm_module.rs"]
mod reference_wasm_module;
/// Issue #160: the shared malformed-input/wrong-binding corpus, executed by
/// the TypeScript/Wasm calling consumer above and cross-checked against the
/// same manifest (`tests/support/public_generic_hostile_corpus.rs`) the
/// native harness's `shared_hostile_corpus` module uses.
#[path = "public_generic_wasm_adapter_v1/shared_hostile_corpus.rs"]
mod shared_hostile_corpus;
#[path = "public_generic_wasm_adapter_v1/typescript_calling_consumer.rs"]
mod typescript_calling_consumer;
