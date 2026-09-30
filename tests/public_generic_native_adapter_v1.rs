//! Harness for the native C11 physical adapter (issue #154). Each subject
//! is a module here, per docs/ARCHITECTURE.md#integration-test-harnesses;
//! future native-adapter fixtures (issues #155-#159, #162) add modules
//! rather than new top-level files.

#[path = "public_generic_native_adapter_v1/authenticated_handoff.rs"]
mod authenticated_handoff;
/// The generated C11 *calling* consumer (issue #158): a real, standalone
/// external C11 program built and linked directly against the same compiled
/// native provider `fixture` exercises, executed end to end.
#[path = "public_generic_native_adapter_v1/c_calling_consumer.rs"]
mod c_calling_consumer;
/// Issue #173: [Hostile Carrier Corpus v1](../docs/PUBLIC-GENERIC-CARRIER-HOSTILE-CORPUS-V1.md)
/// driven against a real `VerifiedPublicGenericDescriptor`, with a
/// five-counter effect ledger proving no hostile ticket reaches allocation,
/// target invocation, ownership commit, a host call, or cleanup.
#[path = "public_generic_native_adapter_v1/carrier_hostile_corpus.rs"]
mod carrier_hostile_corpus;
/// The generated C++17 move-only *calling* consumer (issue #159): a real,
/// standalone external C++17 program that WRAPS the generated C11 calling
/// consumer above, built and linked directly against the same compiled
/// native provider `fixture` exercises, executed end to end.
#[path = "public_generic_native_adapter_v1/cxx_calling_consumer.rs"]
mod cxx_calling_consumer;
#[path = "public_generic_native_adapter_v1/fixture.rs"]
mod fixture;
#[path = "public_generic_native_adapter_v1/malformed_trusted_descriptor.rs"]
mod malformed_trusted_descriptor;
/// Issue #140's "max bounds" callable cell: a payload that exactly
/// saturates the boundary profile (256 owned leaves of 64 KiB each, i.e.
/// exactly `MAX_TOTAL_PAYLOAD_BYTES`) driven through the generated Rust,
/// C11 and C++17 calling consumers against the real compiled provider --
/// the only route in this harness that can reach each consumer's
/// total-payload guard at all.
#[path = "public_generic_native_adapter_v1/max_bounds_saturation.rs"]
mod max_bounds_saturation;
#[path = "public_generic_native_adapter_v1/native_frame_admission.rs"]
mod native_frame_admission;
/// Shared test-support sources, declared exactly once for this binary.
/// Each subject module reaches them with `use crate::...`; declaring them
/// per subject compiled the same file seven times (`clippy::duplicate_mod`).
#[path = "support/public_generic_admitted_subject.rs"]
pub(crate) mod public_generic_admitted_subject;
#[path = "support/public_generic_hostile_corpus.rs"]
pub(crate) mod public_generic_hostile_corpus;
#[path = "public_generic_native_adapter_v1/result_carrier_hostility.rs"]
mod result_carrier_hostility;
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
/// Issue #301: one closed, versioned settlement corpus executed against the
/// interpreter, raw native C11 -O0/-O2 (plus local ASan), the compiled Core
/// Wasm provider and the generated C11/Rust/C++17/TypeScript callers for the
/// same checked `Pair<Bytes>` endpoint, asserted as a complete matrix.
#[cfg(unix)]
#[path = "public_generic_native_adapter_v1/settlement_matrix.rs"]
mod settlement_matrix;
/// Issue #160: the shared malformed-input/wrong-binding corpus, executed
/// identically by the Rust, C11, and C++17 generated calling consumers
/// above and cross-checked against one manifest
/// (`tests/support/public_generic_hostile_corpus.rs`).
#[path = "public_generic_native_adapter_v1/shared_hostile_corpus.rs"]
mod shared_hostile_corpus;

/// Unix-only external caller matrix; no Windows/MSVC coverage is inferred.
#[cfg(unix)]
#[path = "public_generic_native_adapter_v1/consumer_settlement.rs"]
mod consumer_settlement;
