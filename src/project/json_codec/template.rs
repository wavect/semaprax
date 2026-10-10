//! Expand compiler-owned markers once; authored replacement bytes stay opaque.

pub(super) fn expand(source: &str, replacements: &[(&str, &str)]) -> String {
    let mut output = String::with_capacity(source.len());
    let mut remaining = source;
    while let Some((offset, marker, value)) = replacements
        .iter()
        .filter_map(|(marker, value)| {
            assert!(!marker.is_empty(), "codec template marker must be nonempty");
            remaining
                .find(*marker)
                .map(|offset| (offset, *marker, *value))
        })
        .min_by_key(|(offset, _, _)| *offset)
    {
        output.push_str(&remaining[..offset]);
        output.push_str(value);
        remaining = &remaining[offset + marker.len()..];
    }
    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::expand;

    #[test]
    fn replacements_are_opaque_and_unknown_template_text_is_preserved() {
        assert_eq!(
            expand(
                "__NAME__ / __ID__ / __BOUND__ / __UNKNOWN__ / é",
                &[
                    ("__NAME__", "__ID__"),
                    ("__ID__", "a.__BOUND__"),
                    ("__BOUND__", "16")
                ]
            ),
            "__ID__ / a.__BOUND__ / 16 / __UNKNOWN__ / é"
        );
    }

    #[test]
    fn ordinary_marker_expansion_preserves_existing_template_bytes() {
        let source = "__ID__::__NAME__ = __BOUND__; __NAME__";
        let replacements = [
            ("__NAME__", "Record"),
            ("__ID__", "app.record"),
            ("__BOUND__", "64"),
        ];
        let previous = replacements
            .iter()
            .fold(source.to_owned(), |text, (marker, value)| {
                text.replace(*marker, value)
            });
        assert_eq!(expand(source, &replacements), previous);
        assert_eq!(expand(source, &[]), source);
    }
}
