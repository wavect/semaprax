use core::num::NonZeroU32;
use semaprax_generated_native_rust_sdk::{
    NativeRustSdk, NativeRustSdkCallError, NativeRustSdkImports, NativeRustSdkStatusClass,
};

struct Host;

impl NativeRustSdkImports for Host {}

fn main() {
    let mut calculator = NativeRustSdk::new(Host, &[]).expect("admit generated Project SDK");
    assert_eq!(calculator.spx_calculator_dot_add(19, 23), Ok(42));
    assert_eq!(calculator.spx_calculator_dot_divide(84, 2), Ok(42));
    assert_eq!(
        calculator.spx_calculator_dot_divide(1, 0),
        Err(NativeRustSdkCallError::Semantic {
            domain_id: "semaprax.native-rust-semantics.v1",
            code: NonZeroU32::new(1).unwrap(),
            class: NativeRustSdkStatusClass::Contract,
            retryable: false,
        })
    );
    assert_eq!(
        calculator.spx_calculator_dot_is_hyphen_negative(-1),
        Ok(true)
    );
    assert_eq!(calculator.spx_calculator_dot_not(true), Ok(false));
    println!("42");
}
