//! Harness for the native C11 physical adapter (issue #154). Each subject
//! is a module here, per docs/ARCHITECTURE.md#integration-test-harnesses;
//! future native-adapter fixtures (issues #155-#159, #162) add modules
//! rather than new top-level files.
/// The generated C11 *calling* consumer (issue #158): a real, standalone
/// external C11 program built and linked directly against the same compiled
/// native provider `fixture` exercises, executed end to end.
#[path = "public_generic_native_adapter_v1/c_calling_consumer.rs"]
mod c_calling_consumer;
/// The generated C++17 move-only *calling* consumer (issue #159): a real,
/// standalone external C++17 program that WRAPS the generated C11 calling
/// consumer above, built and linked directly against the same compiled
/// native provider `fixture` exercises, executed end to end.
#[path = "public_generic_native_adapter_v1/cxx_calling_consumer.rs"]
mod cxx_calling_consumer;
#[path = "public_generic_native_adapter_v1/fixture.rs"]
mod fixture;
/// The generated Rust *calling* consumer (issue #156): a real, standalone
/// external crate built against the same compiled native provider `fixture`
/// exercises from C, executed end to end.
#[path = "public_generic_native_adapter_v1/rust_calling_consumer.rs"]
mod rust_calling_consumer;
/// Issue #162: closes `carrier::settlement_corpus`'s own documented native
/// exclusion by running the identical base-shape and failure-injection
/// corpus against `InterpreterProvider`/`WasmProvider` in-process and
/// native C11 compiled-and-executed at `-O0`/`-O2`, comparing all four
/// against one independently pinned expectation and against each other.
#[path = "public_generic_native_adapter_v1/settlement_corpus.rs"]
mod settlement_corpus;
/// Issue #160: the shared malformed-input/wrong-binding corpus, executed
/// identically by the Rust, C11, and C++17 generated calling consumers
/// above and cross-checked against one manifest
/// (`tests/support/public_generic_hostile_corpus.rs`).
#[path = "public_generic_native_adapter_v1/shared_hostile_corpus.rs"]
mod shared_hostile_corpus;
