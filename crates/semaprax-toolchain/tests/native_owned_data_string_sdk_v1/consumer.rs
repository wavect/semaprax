#![forbid(unsafe_code)]
use semaprax_generated_native_rust_owned_data_sdk::{CallError, NativeRustOwnedDataSdk};

fn main() {
    let input = [0, 1, 127, 128, 255];
    let mut sdk = NativeRustOwnedDataSdk::new().unwrap();
    for _ in 0..16 {
        // Each failed invocation is followed by a successful call through the
        // same safe SDK object, exercising its close/reinitialization boundary.
        assert_eq!(
            sdk.spx_s_dot_local(&input, 0),
            Err(CallError::SemanticFailure)
        );
        assert_eq!(sdk.spx_s_dot_local(&input, 1).unwrap(), input);
        assert_eq!(
            sdk.spx_s_dot_late(&input, 0),
            Err(CallError::SemanticFailure)
        );
        assert_eq!(sdk.spx_s_dot_late(&input, 1).unwrap(), input);
        assert_eq!(
            sdk.spx_s_dot_callee(&input, 0),
            Err(CallError::SemanticFailure)
        );
        assert_eq!(sdk.spx_s_dot_callee(&input, 1).unwrap(), input);
        assert_eq!(
            sdk.spx_s_dot_mixed(&input, 0),
            Err(CallError::SemanticFailure)
        );
        assert_eq!(sdk.spx_s_dot_mixed(&input, 1).unwrap(), input);
        assert_eq!(
            sdk.spx_s_dot_mixed_hyphen_late(&input, 0),
            Err(CallError::SemanticFailure)
        );
        assert_eq!(sdk.spx_s_dot_mixed_hyphen_late(&input, 1).unwrap(), input);
        assert_eq!(
            sdk.spx_s_dot_mixed_hyphen_reverse(&input, 0),
            Err(CallError::SemanticFailure)
        );
        assert_eq!(
            sdk.spx_s_dot_mixed_hyphen_reverse(&input, 1).unwrap(),
            input
        );
        assert_eq!(
            sdk.spx_s_dot_loop(&input, 2),
            Err(CallError::SemanticFailure)
        );
        assert_eq!(sdk.spx_s_dot_loop(&input, 10).unwrap(), input);
        assert_eq!(sdk.spx_s_dot_clone(&input).unwrap(), input);
        assert_eq!(sdk.spx_s_dot_concat(&input).unwrap(), input);
        assert_eq!(sdk.spx_s_dot_nul(&input).unwrap(), input);
        assert!(sdk.spx_s_dot_empty(&[]).unwrap().is_empty());
        assert_eq!(sdk.spx_s_dot_empty(&input).unwrap(), input);
    }
    drop(sdk);
    println!("standalone-owned-data-string-sdk-ok");
}
