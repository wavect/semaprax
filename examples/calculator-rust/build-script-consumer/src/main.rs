mod generated {
    include!(concat!(env!("OUT_DIR"), "/semaprax_sdk/lib.rs"));
}

use generated::{NativeRustSdk, NativeRustSdkImports};

struct Host;
impl NativeRustSdkImports for Host {}

fn main() {
    let mut calculator = NativeRustSdk::new(Host, &[]).expect("admit prepared calculator SDK");
    assert_eq!(calculator.spx_calculator_dot_add(19, 23), Ok(42));
    println!("42");
}
