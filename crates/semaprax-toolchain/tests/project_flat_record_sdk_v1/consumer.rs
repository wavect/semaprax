#![forbid(unsafe_code)]

use semaprax_generated_native_rust_owned_data_sdk::{
    CallError, NativeRustOwnedDataSdk, SpxRecordId6c6566742e5061796c6f6164080c7fc285,
    SpxRecordId72696768742e5061796c6f6164,
};

fn left(
    value: &SpxRecordId6c6566742e5061796c6f6164080c7fc285,
    input: &[u8],
    count: i64,
    valid: bool,
) {
    // The empty persistent field identity maps to this exact prefix-only name.
    assert_eq!(value.spx_field_id_, input);
    assert_eq!(value.spx_field_id_6c6566742e636f756e74, count);
    assert_eq!(value.spx_field_id_6c6566742e76616c6964, valid);
    assert_eq!(value.spx_field_id_6c6566742e73697a65, input.len());
}

fn right(value: &SpxRecordId72696768742e5061796c6f6164, input: &[u8], count: i64, valid: bool) {
    assert_eq!(value.spx_field_id_72696768742e6279746573, input);
    assert_eq!(value.spx_field_id_72696768742e636f756e74, count);
    assert_eq!(value.spx_field_id_72696768742e76616c6964, valid);
    assert_eq!(value.spx_field_id_72696768742e73697a65, input.len());
}

fn main() {
    let mut first = NativeRustOwnedDataSdk::new().unwrap();
    let mut second = NativeRustOwnedDataSdk::new().unwrap();
    let cases = [
        Vec::new(),
        vec![0, 255, 128, 65, 0],
        (0..65_536).map(|index| (index % 251) as u8).collect(),
    ];
    for _ in 0..16 {
        for input in &cases {
            for divisor in [-1, 1, 2, 0] {
                for valid in [false, true] {
                    for sdk in [&mut first, &mut second] {
                        let l = sdk.spx_left_dot_payload(input, divisor, valid);
                        let r = sdk.spx_right_dot_payload(input, divisor, valid);
                        if divisor == 0 {
                            assert_eq!(l, Err(CallError::SemanticFailure));
                            assert_eq!(r, Err(CallError::SemanticFailure));
                            // Real arithmetic failures both after and before
                            // Bytes creation must settle before object reuse.
                            left(
                                &sdk.spx_left_dot_payload(input, 2, valid).unwrap(),
                                input,
                                42,
                                valid,
                            );
                            right(
                                &sdk.spx_right_dot_payload(input, 2, valid).unwrap(),
                                input,
                                42,
                                valid,
                            );
                        } else {
                            left(&l.unwrap(), input, 84 / divisor, valid);
                            right(&r.unwrap(), input, 84 / divisor, valid);
                        }
                    }
                }
            }
        }
    }
    let over = vec![0u8; 65_537];
    assert_eq!(
        first.spx_left_dot_payload(&over, 1, true),
        Err(CallError::AdapterRejected)
    );
    assert_eq!(
        first.spx_right_dot_payload(&over, 1, true),
        Err(CallError::AdapterRejected)
    );
    left(
        &first.spx_left_dot_payload(&[], 1, false).unwrap(),
        &[],
        84,
        false,
    );
    right(
        &first.spx_right_dot_payload(&[], 1, false).unwrap(),
        &[],
        84,
        false,
    );

    let mut input = vec![0, 255, 128, 65, 0];
    let mut kept_left = first.spx_left_dot_payload(&input, -1, true).unwrap();
    let kept_right = second.spx_right_dot_payload(&input, -1, true).unwrap();
    input.fill(9);
    first.spx_right_dot_payload(&input, 2, false).unwrap();
    second.spx_left_dot_payload(&input, 2, false).unwrap();
    drop(first);
    drop(second);
    left(&kept_left, &[0, 255, 128, 65, 0], -84, true);
    right(&kept_right, &[0, 255, 128, 65, 0], -84, true);
    kept_left.spx_field_id_[0] = 7;
    right(&kept_right, &[0, 255, 128, 65, 0], -84, true);
    // Reused SDK objects close/reinitialize their contexts on every call;
    // this is not a claim about persistent-context invocation counters.
    println!("project-flat-record-sdk-ok");
}
