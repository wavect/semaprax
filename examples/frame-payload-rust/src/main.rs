#![forbid(unsafe_code)]

use semaprax_generated_native_rust_owned_data_sdk::NativeRustOwnedDataSdk;
use serde::Deserialize;

#[derive(Deserialize)]
struct Corpus {
    schema: String,
    maximum_frame_bytes: usize,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    kind: String,
    frame_hex: Option<String>,
    payload_hex: Option<String>,
    payload_length: Option<usize>,
    valid: bool,
    error: Option<i64>,
}

fn from_hex(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

fn materialize(case: &Case) -> (Vec<u8>, Vec<u8>) {
    if case.kind == "hex" {
        return (
            from_hex(case.frame_hex.as_deref().unwrap()),
            case.payload_hex
                .as_deref()
                .map(from_hex)
                .unwrap_or_default(),
        );
    }
    assert_eq!(case.kind, "generated-index-mod-256");
    let length = case.payload_length.unwrap();
    let payload = (0..length).map(|index| index as u8).collect::<Vec<_>>();
    let mut frame = Vec::with_capacity(8 + length);
    frame.extend_from_slice(b"SPX1");
    frame.extend_from_slice(&(length as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    (frame, payload)
}

fn main() {
    let corpus: Corpus = serde_json::from_str(include_str!("../corpus.json")).unwrap();
    assert_eq!(corpus.schema, "semaprax.frame-payload-corpus.v1");
    assert_eq!(corpus.maximum_frame_bytes, 65_536);
    let mut sdk = NativeRustOwnedDataSdk::new().unwrap();
    let mut direct_calls = 0usize;
    for case in &corpus.cases {
        let (frame, expected) = materialize(case);
        let optional = sdk
            .spx_frame_dot_payload_hyphen_maybe(&frame)
            .unwrap();
        let result = sdk
            .spx_frame_dot_payload_hyphen_result(&frame)
            .unwrap();
        if case.valid {
            assert_eq!(optional.as_deref(), Some(expected.as_slice()), "{}", case.name);
            assert_eq!(result.as_ref().map(Vec::as_slice), Ok(expected.as_slice()), "{}", case.name);
            direct_calls += 1;
            assert_eq!(sdk.spx_frame_dot_payload(&frame).unwrap(), expected, "{}", case.name);
        } else {
            assert_eq!(optional, None, "{}", case.name);
            assert_eq!(result, Err(case.error.unwrap()), "{}", case.name);
        }
    }
    assert_eq!(
        direct_calls,
        corpus.cases.iter().filter(|case| case.valid).count()
    );
    println!("frame-payload-rust-v1-ok");
}
