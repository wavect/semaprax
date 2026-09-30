// Encoded byte bounds and immutable legacy wire checks.
use super::*;

#[test]
fn bounded_encoding_keeps_legacy_wire_and_exact_utf8_boundary() {
    let value = String::from("é");
    let expected = vec![2, 0, 0, 0, 0xc3, 0xa9];
    assert_eq!(encode(&value).unwrap(), expected);
    assert_eq!(encode_bounded(&value, expected.len()).unwrap(), expected);
    assert_eq!(encode_bounded(&value, MAX_BYTES).unwrap(), expected);
    assert_eq!(decode::<String>(&expected).unwrap(), value);
    let errors = encode_bounded(&value, expected.len() - 1).unwrap_err();
    assert_eq!(errors[0].code, "SPX-G305");
}

#[test]
fn bounded_encoding_handles_zero_small_and_excessive_limits() {
    assert_eq!(encode_bounded(&7u8, 1).unwrap(), vec![7]);
    assert_eq!(encode_bounded(&7u8, 0).unwrap_err()[0].code, "SPX-G305");
    assert_eq!(
        encode_bounded(&7u8, MAX_BYTES + 1).unwrap_err()[0].code,
        "SPX-G305"
    );
    let mut empty = Encoder {
        bytes: Vec::new(),
        byte_limit: 0,
        nodes: 0,
        depth: 0,
    };
    empty.raw(&[]).unwrap();
    assert!(empty.bytes.is_empty());
    assert_eq!(empty.raw(&[1]).unwrap_err()[0].code, "SPX-G305");
    assert!(empty.bytes.is_empty());
}

#[test]
fn bounded_raw_refusal_preserves_prior_bytes_at_growth_boundary() {
    let mut out = Encoder {
        bytes: Vec::new(),
        byte_limit: 257,
        nodes: 0,
        depth: 0,
    };
    out.raw(&[17; 256]).unwrap();
    assert_eq!(out.raw(&[18, 19]).unwrap_err()[0].code, "SPX-G305");
    assert_eq!(out.bytes, vec![17; 256]);
    out.raw(&[20]).unwrap();
    assert_eq!(out.bytes.len(), 257);
    assert_eq!(out.bytes[256], 20);
    assert_eq!(out.raw(&[21]).unwrap_err()[0].code, "SPX-G305");
    assert_eq!(out.bytes[256], 20);
}
