#![forbid(unsafe_code)]
use semaprax_generated_native_rust_owned_data_sdk::NativeRustOwnedDataSdk;

fn input(length: usize) -> Vec<u8> {
    match length {
        0 => Vec::new(),
        3 => vec![255, 0, 128],
        5 => vec![0, 255, 195, 40, 128],
        65_535 | 65_536 => (0..length).map(|index| (index % 251) as u8).collect(),
        _ => panic!("unexpected fixture length"),
    }
}

fn round(sdk: &mut NativeRustOwnedDataSdk, length: usize) -> Vec<(usize, Vec<u8>)> {
    let mut bytes = input(length);
    let expected = input(length);
    let mut recovery = input(3);
    let some = sdk.spx_inactive_dot_maybe(&bytes, true).unwrap().unwrap();
    assert_eq!(sdk.spx_inactive_dot_maybe(&bytes, false).unwrap(), None);
    let recovered_some = sdk
        .spx_inactive_dot_maybe(&recovery, true)
        .unwrap()
        .unwrap();
    let ok = sdk.spx_inactive_dot_result(&bytes, true).unwrap().unwrap();
    assert_eq!(sdk.spx_inactive_dot_result(&bytes, false).unwrap(), Err(-7));
    let recovered_ok = sdk
        .spx_inactive_dot_result(&recovery, true)
        .unwrap()
        .unwrap();
    assert_eq!(bytes, expected);
    assert_eq!(recovery, input(3));
    let outputs = vec![
        (length, some),
        (3, recovered_some),
        (length, ok),
        (3, recovered_ok),
    ];
    for (index, (output_length, output)) in outputs.iter().enumerate() {
        assert_eq!(output, &input(*output_length));
        if !output.is_empty() {
            assert_ne!(output.as_ptr(), bytes.as_ptr());
            assert_ne!(output.as_ptr(), recovery.as_ptr());
            for (_, previous) in &outputs[..index] {
                assert_ne!(output.as_ptr(), previous.as_ptr());
            }
        }
    }
    bytes.fill(0x31);
    recovery.fill(0x32);
    for (output_length, output) in &outputs {
        assert_eq!(output, &input(*output_length));
    }
    outputs
}

fn main() {
    let mut first = NativeRustOwnedDataSdk::new().unwrap();
    let mut second = NativeRustOwnedDataSdk::new().unwrap();
    let mut retained = Vec::new();
    let mut calls = 0;
    for _ in 0..4 {
        for sdk in [&mut first, &mut second] {
            for length in [0, 5, 65_535, 65_536] {
                retained.extend(round(sdk, length));
                calls += 6;
            }
        }
    }
    drop(first);
    drop(second);
    assert_eq!(calls, 192);
    assert_eq!(retained.len(), 128);
    for (index, (length, output)) in retained.iter().enumerate() {
        assert_eq!(output, &input(*length));
        if !output.is_empty() {
            for (_, previous) in &retained[..index] {
                if !previous.is_empty() {
                    assert_ne!(output.as_ptr(), previous.as_ptr());
                }
            }
        }
    }
    // Mutate one retained result after both SDKs are gone. Every other result
    // must keep its independent literal corpus bytes, including sibling calls.
    let changed = retained
        .iter()
        .position(|(_, bytes)| !bytes.is_empty())
        .unwrap();
    retained[changed].1[0] = 0x73;
    for (index, (length, output)) in retained.iter().enumerate() {
        if index != changed {
            assert_eq!(output, &input(*length));
        }
    }
    println!("inactive-cleanup-sdk-ok:{calls}");
}
