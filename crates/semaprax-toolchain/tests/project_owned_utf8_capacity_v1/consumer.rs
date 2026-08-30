#![forbid(unsafe_code)]

use semaprax_generated_native_rust_owned_data_sdk::NativeRustOwnedDataSdk;

// Independent literal byte oracle, not imported from the source fixture,
// descriptor, provider, or a generated return value.
const UNIT: [u8; 13] = [
    239, 187, 191, 0, 228, 184, 150, 195, 169, 240, 159, 153, 130,
];

fn verify(value: &str, expected: &[u8], byte_len: usize) {
    assert_eq!(value.len(), byte_len);
    assert_eq!(value.as_bytes(), expected);
    assert!(value.starts_with('\u{feff}'));
    assert_eq!(
        value.chars().filter(|character| *character == '\0').count(),
        5_041
    );
    assert_eq!(value.chars().count(), 5_041 * 5 + byte_len - 65_533);
}

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    assert_eq!(arguments.len(), 1);
    let byte_len = match arguments[0].as_str() {
        "65535" => 65_535,
        "65536" => 65_536,
        _ => panic!("expected one admitted capacity"),
    };
    let mut expected = UNIT.repeat(5_041);
    assert_eq!(expected.len(), 65_533);
    expected.resize(byte_len, b'a');
    let mut first = NativeRustOwnedDataSdk::new().unwrap();
    let mut second = NativeRustOwnedDataSdk::new().unwrap();
    let retained = first.spx_utf8_dot_maximum().unwrap();
    let retained_other = second.spx_utf8_dot_maximum().unwrap();
    verify(&retained, &expected, byte_len);
    verify(&retained_other, &expected, byte_len);
    assert_ne!(retained.as_ptr(), retained_other.as_ptr());
    for _ in 0..16 {
        let mut value = first.spx_utf8_dot_maximum().unwrap();
        let other = second.spx_utf8_dot_maximum().unwrap();
        verify(&value, &expected, byte_len);
        verify(&other, &expected, byte_len);
        assert_ne!(value.as_ptr(), retained.as_ptr());
        assert_ne!(value.as_ptr(), other.as_ptr());
        value.clear();
        value.push_str("independently changed");
        verify(&other, &expected, byte_len);
        verify(&retained, &expected, byte_len);
        assert_eq!(value, "independently changed");
    }
    drop(first);
    drop(second);
    verify(&retained, &expected, byte_len);
    verify(&retained_other, &expected, byte_len);
    println!("project-owned-utf8-capacity-ok:{byte_len}");
}
