//! Deterministic C literal escaping shared by native emitters.
use std::fmt::Write as _;

pub(crate) fn c_string(value: &str) -> String {
    let mut escaped = crate::bounded_output::CappedString::new();
    for byte in value.as_bytes() {
        match *byte {
            b'\\' => escaped.push_str("\\\\"),
            b'"' => escaped.push_str("\\\""),
            b'\n' => escaped.push_str("\\n"),
            b'\r' => escaped.push_str("\\r"),
            b'\t' => escaped.push_str("\\t"),
            b'?' | 0x00..=0x1f | 0x7f..=0xff => {
                write!(escaped, "\\{byte:03o}").expect("writing to a string cannot fail");
            }
            value => escaped.push(char::from(value)),
        }
    }
    escaped.into_string()
}
