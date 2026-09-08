/// Canonical `f64` literal text: shortest round-trip decimal that always
/// re-parses as a floating-point literal (it keeps a fraction or exponent).
pub(crate) fn canonical_f64_bits(bits: u64) -> String {
    let text = format!("{}", f64::from_bits(bits));
    if text.contains('.')
        || text.contains('e')
        || !text.chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        text
    } else {
        format!("{text}.0")
    }
}

/// Canonical `f32` literal text in the same style, without the suffix.
pub(crate) fn canonical_f32_bits(bits: u32) -> String {
    let text = format!("{}", f32::from_bits(bits));
    if text.contains('.')
        || text.contains('e')
        || !text.chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        text
    } else {
        format!("{text}.0")
    }
}

pub(crate) fn write_escaped(output: &mut impl std::fmt::Write, value: &str) {
    for value in value.chars() {
        match value {
            '\\' => output.write_str("\\\\").unwrap(),
            '"' => output.write_str("\\\"").unwrap(),
            value => output.write_char(value).unwrap(),
        }
    }
}

pub(crate) fn write_joined(output: &mut impl std::fmt::Write, values: &[String], separator: &str) {
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            output.write_str(separator).unwrap();
        }
        output.write_str(value).unwrap();
    }
}
