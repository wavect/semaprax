use core::num::NonZeroU32;
use std::io::Write as _;

use semaprax_generated_native_rust_sdk::{
    NativeRustSdk, NativeRustSdkCallError, NativeRustSdkImports, NativeRustSdkStatusClass,
};

const OUTPUT: &[u8] = b"42\n";

struct Host;

impl NativeRustSdkImports for Host {}

fn main() {
    assert!(NativeRustSdk::new(Host, &["unexpected.capability"]).is_err());
    let mut calculator = NativeRustSdk::new(Host, &[]).expect("admit generated calculator SDK");
    assert_eq!(calculator.spx_calculator_dot_add(19, 23), Ok(42));
    assert_eq!(calculator.spx_calculator_dot_subtract(23, 19), Ok(4));
    assert_eq!(calculator.spx_calculator_dot_multiply(6, 7), Ok(42));
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
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(OUTPUT).unwrap();
    stdout.flush().unwrap();
}
