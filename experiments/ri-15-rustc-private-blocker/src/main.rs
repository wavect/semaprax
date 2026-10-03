#![feature(rustc_private)]

// This compile-time dependency is intentional: the experiment cannot begin
// until a maintained toolchain supplies both private compiler crates.
extern crate rustc_driver;
extern crate rustc_interface;

/// The requested Semaprax-owned value is deliberately non-zero-sized.
/// No representation or ownership equivalence with Rust is asserted here.
struct SemapraxState {
    generation: u64,
    cleanup_token: [u8; 32],
}

/// The capability RI-15 would need to make safe: a Semaprax callback supplied
/// to generic Rust code while retaining Semaprax ownership and cleanup rules.
trait SemapraxCallback<T> {
    fn call(&mut self, state: &mut SemapraxState, input: T) -> T;
}

struct RustConsumer<C> {
    callback: C,
}

impl<C: SemapraxCallback<u64>> RustConsumer<C> {
    fn round_trip(&mut self, state: &mut SemapraxState, input: u64) -> u64 {
        self.callback.call(state, input)
    }
}

fn main() {
    // The imported crates are intentionally unused. Their resolution is the
    // reproducible blocker before any compiler-plugin integration is written.
    let _ = core::mem::size_of::<SemapraxState>();
}
