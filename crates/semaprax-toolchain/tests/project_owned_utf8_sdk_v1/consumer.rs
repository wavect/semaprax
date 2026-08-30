#![forbid(unsafe_code)]

use semaprax_generated_native_rust_owned_data_sdk::{CallError, NativeRustOwnedDataSdk};

const LEFT: &str = "\u{feff}hello\0世界é";

fn exercise(sdk: &mut NativeRustOwnedDataSdk) {
    for divisor in [-1, 1, 2] {
        assert_eq!(sdk.spx_utf8_dot_left(divisor).unwrap(), LEFT);
        assert_eq!(sdk.spx_utf8_dot_right(divisor).unwrap(), "");
    }
    for _ in 0..20 {
        // The source stages an owned argument before division fails. These
        // observations prove safe-API recovery, not a physical allocation trace
        // or reuse of one initialized native context across invocations.
        assert_eq!(sdk.spx_utf8_dot_left(0), Err(CallError::SemanticFailure));
        assert_eq!(sdk.spx_utf8_dot_left(2).unwrap(), LEFT);
        assert_eq!(sdk.spx_utf8_dot_right(0), Err(CallError::SemanticFailure));
        assert_eq!(sdk.spx_utf8_dot_right(2).unwrap(), "");
    }

    // This is the literal Node corpus, including malformed UTF-8 as raw Bytes.
    let inputs = [
        Vec::new(),
        vec![0, 255, 195, 40],
        (0..65_536).map(|index| (index % 251) as u8).collect(),
    ];
    for input in &inputs {
        let mut output = sdk.spx_bytes_dot_raw(input).unwrap();
        assert_eq!(&output, input);
        let retained = sdk.spx_bytes_dot_raw(input).unwrap();
        if !output.is_empty() {
            assert_ne!(output.as_ptr(), input.as_ptr());
            assert_ne!(output.as_ptr(), retained.as_ptr());
            output[0] ^= 255;
            assert_eq!(&retained, input);
        }
    }
    assert_eq!(
        sdk.spx_bytes_dot_raw(&vec![0; 65_537]),
        Err(CallError::AdapterRejected)
    );
    assert_eq!(sdk.spx_utf8_dot_left(1).unwrap(), LEFT);
    assert_eq!(
        sdk.spx_bytes_dot_raw(&[0, 255, 195, 40]).unwrap(),
        [0, 255, 195, 40]
    );
}

fn main() {
    let mut first = NativeRustOwnedDataSdk::new().unwrap();
    let mut second = NativeRustOwnedDataSdk::new().unwrap();
    let retained_text = first.spx_utf8_dot_left(1).unwrap();
    let retained_empty = first.spx_utf8_dot_right(1).unwrap();
    let retained_bytes = first.spx_bytes_dot_raw(&[0, 255, 195, 40]).unwrap();
    exercise(&mut first);
    exercise(&mut second);

    let mut changed_text = second.spx_utf8_dot_left(1).unwrap();
    assert_ne!(changed_text.as_ptr(), retained_text.as_ptr());
    changed_text.clear();
    changed_text.push_str("changed independently");
    let mut changed_bytes = second.spx_bytes_dot_raw(&[0, 255, 195, 40]).unwrap();
    changed_bytes.fill(7);
    drop(first);
    drop(second);

    assert_eq!(retained_text, LEFT);
    assert_eq!(retained_text.len(), 17);
    assert_eq!(
        retained_text.as_bytes(),
        &[239, 187, 191, 104, 101, 108, 108, 111, 0, 228, 184, 150, 231, 149, 140, 195, 169]
    );
    assert!(retained_empty.is_empty());
    assert_eq!(retained_bytes, [0, 255, 195, 40]);
    assert_eq!(changed_text, "changed independently");
    assert_eq!(changed_bytes, [7; 4]);
    println!("project-owned-utf8-sdk-ok");
}
