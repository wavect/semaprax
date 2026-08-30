#![forbid(unsafe_code)]
use semaprax_generated_native_rust_owned_data_sdk::{CallError, NativeRustOwnedDataSdk};

fn main() {
    let mut first = NativeRustOwnedDataSdk::new().unwrap();
    let mut second = NativeRustOwnedDataSdk::new().unwrap();
    let bytes = [0, 255, 128, 65, 0];
    let mut retained = Vec::new();
    for _ in 0..8 {
        for sdk in [&mut first, &mut second] {
            for length in [0, 1, 2, 3, 4, 5, 2, 1, 0, 3] {
                let mut input = bytes[..length].to_vec();
                let result: Result<Result<Vec<u8>, i64>, CallError> =
                    sdk.spx_result_dot_value(&input);
                match length {
                    0 => assert_eq!(result, Ok(Err(0))),
                    1 => assert_eq!(result, Ok(Err(i64::MIN))),
                    2 => assert_eq!(result, Ok(Err(i64::MAX))),
                    4 => assert_eq!(result, Err(CallError::SemanticFailure)),
                    _ => {
                        let owned = result.unwrap().unwrap();
                        assert_eq!(owned, bytes[..length]);
                        assert_ne!(owned.as_ptr(), input.as_ptr());
                        input.fill(17);
                        assert_eq!(owned, bytes[..length]);
                        retained.push((owned, bytes[..length].to_vec()));
                    }
                }
            }
        }
    }
    drop(first);
    drop(second);
    assert_eq!(retained.len(), 48);
    for (owned, expected) in &retained {
        assert_eq!(owned, expected);
    }
    for index in 0..retained.len() {
        for previous in 0..index {
            assert_ne!(retained[index].0.as_ptr(), retained[previous].0.as_ptr());
        }
    }
    println!("result-extrema-sdk-ok");
}
