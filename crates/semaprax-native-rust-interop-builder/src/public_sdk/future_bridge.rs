//! Pure RI-09 Future adapter source for an explicitly owned same-thread runtime.
//!
//! This prepares Rust code only. It does not authorize an executor, network
//! effect, Semaprax async export, publication, or durable checkpoint route.

pub const LOCAL_FUTURE_BRIDGE_SCHEMA: &str = "semaprax.native-rust-local-future-bridge.v1";

/// Exact standalone source to stage inside a caller-authorized Rust package.
/// The caller supplies a compatible executor and retains its Cargo authority.
pub fn render_local_future_bridge() -> &'static str {
    include_str!("future_runtime.template")
}
