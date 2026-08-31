#![forbid(unsafe_code)]
use semaprax_generated_native_rust_owned_data_sdk::{CallError, NativeRustOwnedDataSdk};

fn successes(sdk: &mut NativeRustOwnedDataSdk) -> Vec<Vec<u8>> {
    let mut text = String::from("é\0A");
    let mut bytes = vec![0, 255, 128];
    let mut other_text = String::from("Z\0λ!");
    let mut other_bytes = vec![65, 0, 255, 127, 128, 42];
    assert_eq!(text.as_bytes(), [195, 169, 0, 65]);
    assert_eq!(other_text.as_bytes(), [90, 0, 206, 187, 33]);
    // Literal calls independently check the generated Rust signature of every
    // admitted arity. No descriptor-derived dispatch supplies expected values.
    let results: [Result<Vec<u8>, CallError>; 9] = [
        sdk.spx_mixed_dot_arity0(),
        sdk.spx_mixed_dot_arity1(-13),
        sdk.spx_mixed_dot_arity2(-13, true),
        sdk.spx_mixed_dot_arity3(-13, true, &text),
        sdk.spx_mixed_dot_arity4(-13, true, &text, &bytes),
        sdk.spx_mixed_dot_arity5(-13, true, &text, &bytes, 29),
        sdk.spx_mixed_dot_arity6(-13, true, &text, &bytes, 29, false),
        sdk.spx_mixed_dot_arity7(-13, true, &text, &bytes, 29, false, &other_text),
        sdk.spx_mixed_dot_arity8(
            -13,
            true,
            &text,
            &bytes,
            29,
            false,
            &other_text,
            &other_bytes,
        ),
    ];
    let owned = results
        .into_iter()
        .map(|result| {
            let value = result.unwrap();
            assert_eq!(value, b"ok");
            for input in [
                text.as_bytes(),
                bytes.as_slice(),
                other_text.as_bytes(),
                other_bytes.as_slice(),
            ] {
                assert_ne!(value.as_ptr(), input.as_ptr());
            }
            value
        })
        .collect::<Vec<_>>();
    assert_eq!(text, "é\0A");
    assert_eq!(bytes, [0, 255, 128]);
    assert_eq!(other_text, "Z\0λ!");
    assert_eq!(other_bytes, [65, 0, 255, 127, 128, 42]);
    text.replace_range(.., "xxxx");
    bytes.fill(17);
    other_text.replace_range(.., "yyyyy");
    other_bytes.fill(19);
    for value in &owned {
        assert_eq!(value, b"ok");
    }
    assert_eq!(text, "xxxx");
    assert_eq!(bytes, [17; 3]);
    assert_eq!(other_text, "yyyyy");
    assert_eq!(other_bytes, [19; 6]);
    owned
}

fn mutations(sdk: &mut NativeRustOwnedDataSdk, retained: &mut Vec<Vec<u8>>) {
    for changed in 0..8 {
        // Exactly one parameter differs; byte/string parameters intentionally
        // change byte length, because that is the subject's declared predicate.
        let value = sdk
            .spx_mixed_dot_arity8(
                if changed == 0 { -12 } else { -13 },
                changed != 1,
                if changed == 2 { "é\0" } else { "é\0A" },
                if changed == 3 {
                    &[0, 255]
                } else {
                    &[0, 255, 128]
                },
                if changed == 4 { 30 } else { 29 },
                changed == 5,
                if changed == 6 { "Z\0λ" } else { "Z\0λ!" },
                if changed == 7 {
                    &[65, 0, 255, 127, 128]
                } else {
                    &[65, 0, 255, 127, 128, 42]
                },
            )
            .unwrap();
        assert_eq!(value, b"bad", "parameter {changed}");
        // A language-level rejection is ordinary Bytes, not an adapter error.
        // Immediately repeat the healthy full tuple before the next mutation.
        let healthy = sdk
            .spx_mixed_dot_arity8(
                -13,
                true,
                "é\0A",
                &[0, 255, 128],
                29,
                false,
                "Z\0λ!",
                &[65, 0, 255, 127, 128, 42],
            )
            .unwrap();
        assert_eq!(healthy, b"ok");
        retained.push(healthy);
    }
}

fn main() {
    let mut first = NativeRustOwnedDataSdk::new().unwrap();
    let mut second = NativeRustOwnedDataSdk::new().unwrap();
    let mut retained = Vec::new();
    for _ in 0..8 {
        for sdk in [&mut first, &mut second] {
            retained.extend(successes(sdk));
            mutations(sdk, &mut retained);
        }
    }
    drop(first);
    drop(second);
    assert_eq!(retained.len(), 272);
    for (index, value) in retained.iter().enumerate() {
        assert_eq!(value, b"ok");
        for previous in &retained[..index] {
            assert_ne!(value.as_ptr(), previous.as_ptr());
        }
    }
    retained[0][0] = b'X';
    for other in &retained[1..] {
        assert_eq!(other, b"ok");
    }
    println!("mixed-arity-sdk-ok");
}
