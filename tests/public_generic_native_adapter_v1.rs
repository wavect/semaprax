//! Harness for the native C11 physical adapter (issue #154). Each subject
//! is a module here, per docs/ARCHITECTURE.md#integration-test-harnesses;
//! future native-adapter fixtures (issues #155-#159, #162) add modules
//! rather than new top-level files.
#[path = "public_generic_native_adapter_v1/fixture.rs"]
mod fixture;
/// The generated Rust *calling* consumer (issue #156): a real, standalone
/// external crate built against the same compiled native provider `fixture`
/// exercises from C, executed end to end.
#[path = "public_generic_native_adapter_v1/rust_calling_consumer.rs"]
mod rust_calling_consumer;
