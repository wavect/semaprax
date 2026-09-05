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
