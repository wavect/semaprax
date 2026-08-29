#![forbid(unsafe_code)]

use semaprax_generated_native_rust_owned_data_sdk::{CallError, NativeRustOwnedDataSdk};

fn main() {
    let mut sdk = NativeRustOwnedDataSdk::new().unwrap();
    assert_eq!(sdk.spx_frame_dot_payload(b"42\0\xff"), Ok(b"42\0\xff".to_vec()));
    assert_eq!(sdk.spx_frame_dot_payload(b""), Ok(Vec::new()));
    let maximum = vec![42u8; 65_536];
    assert_eq!(sdk.spx_frame_dot_payload(&maximum), Ok(maximum.clone()));
    assert_eq!(sdk.spx_frame_dot_payload_hyphen_maybe(b""), Ok(None));
    assert_eq!(
        sdk.spx_frame_dot_payload_hyphen_maybe(b"42"),
        Ok(Some(b"42".to_vec()))
    );
    assert_eq!(
        sdk.spx_frame_dot_payload_hyphen_result(b"x"),
        Ok(Err(-7))
    );
    assert_eq!(
        sdk.spx_frame_dot_payload_hyphen_result(b"42"),
        Ok(Ok(b"42".to_vec()))
    );
    for token in 0..32u8 {
        assert_eq!(sdk.spx_frame_dot_payload(&[token]), Ok(vec![token]));
    }
    let _: Option<CallError> = None;
    println!("42");
}
