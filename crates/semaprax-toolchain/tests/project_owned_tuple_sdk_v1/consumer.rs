#![forbid(unsafe_code)]
#[cfg(feature = "flat")]
use semaprax_generated_native_rust_owned_data_sdk::SpxRecordId7475706c652e5265636f7264;
use semaprax_generated_native_rust_owned_data_sdk::{CallError, NativeRustOwnedDataSdk};

#[cfg(feature = "flat")]
fn record(
    value: SpxRecordId7475706c652e5265636f7264,
    text: &str,
    left: &[u8],
    right: &[u8],
) -> Vec<u8> {
    assert_eq!(value.spx_field_id_74657874, text.len());
    assert_eq!(value.spx_field_id_6c656674, left.len());
    assert_eq!(value.spx_field_id_7269676874, right.len());
    value.spx_field_id_6279746573
}

fn bytes(
    sdk: &mut NativeRustOwnedDataSdk,
    text: &str,
    left: &[u8],
    right: &[u8],
) -> Result<Vec<u8>, CallError> {
    let value = sdk.spx_tuple_dot_bytes(text, left, right);
    #[cfg(feature = "flat")]
    {
        value.map(|value| record(value, text, left, right))
    }
    #[cfg(not(feature = "flat"))]
    {
        value
    }
}

fn text_bytes(
    sdk: &mut NativeRustOwnedDataSdk,
    text: &str,
    left: &[u8],
    right: &[u8],
) -> Result<Vec<u8>, CallError> {
    let value = sdk.spx_tuple_dot_text(text, left, right);
    #[cfg(feature = "flat")]
    {
        value.map(|value| record(value, text, left, right))
    }
    #[cfg(not(feature = "flat"))]
    {
        value
    }
}

fn accepted(sdk: &mut NativeRustOwnedDataSdk, text: &str, left: &[u8], right: &[u8]) {
    assert!(text.len() + left.len() + right.len() <= 65_536);
    assert_eq!(bytes(sdk, text, left, right).unwrap(), left);
    assert_eq!(text_bytes(sdk, text, left, right).unwrap(), text.as_bytes());
    #[cfg(not(feature = "flat"))]
    {
        assert_eq!(
            sdk.spx_tuple_dot_maybe(text, left, right, true).unwrap(),
            Some(left.to_vec())
        );
        assert_eq!(
            sdk.spx_tuple_dot_maybe(text, left, right, false).unwrap(),
            None
        );
        assert_eq!(
            sdk.spx_tuple_dot_result(text, left, right, true).unwrap(),
            Ok(left.to_vec())
        );
        assert_eq!(
            sdk.spx_tuple_dot_result(text, left, right, false).unwrap(),
            Err(-7)
        );
    }
}

fn recovery(sdk: &mut NativeRustOwnedDataSdk) {
    accepted(sdk, "\u{feff}\0€😀", &[0, 255, 195, 40], &[128]);
}

fn rejected(sdk: &mut NativeRustOwnedDataSdk, text: &str, left: &[u8], right: &[u8]) {
    assert!(text.len() + left.len() + right.len() > 65_536);
    assert_eq!(
        bytes(sdk, text, left, right),
        Err(CallError::AdapterRejected)
    );
    recovery(sdk);
    assert_eq!(
        text_bytes(sdk, text, left, right),
        Err(CallError::AdapterRejected)
    );
    recovery(sdk);
    #[cfg(not(feature = "flat"))]
    for active in [false, true] {
        // Unused text/right and inactive None/Err cannot bypass tuple charging.
        assert_eq!(
            sdk.spx_tuple_dot_maybe(text, left, right, active),
            Err(CallError::AdapterRejected)
        );
        recovery(sdk);
        assert_eq!(
            sdk.spx_tuple_dot_result(text, left, right, active),
            Err(CallError::AdapterRejected)
        );
        recovery(sdk);
    }
}

fn exercise(sdk: &mut NativeRustOwnedDataSdk) {
    let raw = (0..65_537)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let unit = "\u{feff}\0€😀";
    assert_eq!(
        unit.as_bytes(),
        &[239, 187, 191, 0, 226, 130, 172, 240, 159, 152, 128]
    );
    assert_eq!(unit.len(), 11);
    let unicode = unit.repeat(2_000);
    assert_eq!(unicode.len(), 22_000);
    let euro_exact = format!("{}a", "€".repeat(21_845));
    let astral_exact = "😀".repeat(16_384);
    assert_eq!(euro_exact.len(), 65_536);
    assert_eq!(astral_exact.len(), 65_536);
    for _ in 0..8 {
        accepted(sdk, "", &[], &[]);
        recovery(sdk);
        for length in [65_535, 65_536] {
            accepted(sdk, "", &raw[..length], &[]);
            accepted(sdk, "", &[], &raw[..length]);
            accepted(sdk, "", &raw[..32_768], &raw[..length - 32_768]);
            accepted(sdk, &unicode, &raw[..20_000], &raw[..length - 42_000]);
        }
        accepted(sdk, &euro_exact, &[], &[]);
        accepted(sdk, &astral_exact, &[], &[]);
        // Isolate each argument's contribution, including unused borrowed data.
        rejected(sdk, "", &raw, &[]);
        rejected(sdk, "", &[], &raw);
        rejected(sdk, "", &raw[..32_768], &raw[..32_769]);
        rejected(sdk, &unicode, &raw[..20_000], &raw[..23_537]);
        rejected(sdk, "a", &raw[..32_768], &raw[..32_768]);
        rejected(sdk, &euro_exact, &[0], &[]);
        rejected(sdk, &astral_exact, &[], &[255]);
        rejected(sdk, &format!("{euro_exact}a"), &[], &[]);
    }
}

fn main() {
    let mut first = NativeRustOwnedDataSdk::new().unwrap();
    let mut second = NativeRustOwnedDataSdk::new().unwrap();
    exercise(&mut first);
    exercise(&mut second);
    let mut input = vec![0, 255, 195, 40, 128];
    let mut kept = bytes(&mut first, "", &input, &[]).unwrap();
    let kept_other = bytes(&mut second, "", &input, &[]).unwrap();
    let kept_text = text_bytes(&mut second, "\u{feff}\0€😀", &[], &[]).unwrap();
    assert_ne!(kept.as_ptr(), kept_other.as_ptr());
    input.fill(7);
    recovery(&mut first);
    recovery(&mut second);
    drop(first);
    drop(second);
    assert_eq!(kept, [0, 255, 195, 40, 128]);
    assert_eq!(kept_other, [0, 255, 195, 40, 128]);
    assert_eq!(
        kept_text,
        [239, 187, 191, 0, 226, 130, 172, 240, 159, 152, 128]
    );
    kept[0] = 9;
    assert_eq!(kept_other, [0, 255, 195, 40, 128]);
    println!(
        "project-owned-tuple-sdk-ok:{}",
        if cfg!(feature = "flat") { "v9" } else { "v8" }
    );
}
