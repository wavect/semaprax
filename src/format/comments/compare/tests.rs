use super::*;
use std::fmt::Write as _;
use std::path::Path;

const SOURCE: &str = "// héader 🌍\r\nmodule compare.comments;\r\n\r\n// declaration\r\n@id(\"compare.main\") fn main()->i64 {\r\n let x=1; // trailing λ\r\n while x<0 { // nested\r\n  x<0 // tail\r\n }\r\n x\r\n // closing\r\n}\r\n// final Ω  \r\n";

#[test]
fn full_commented_comparison_keeps_old_unicode_crlf_trailing_nested_and_noncanonical_bytes() {
    for source in [
        SOURCE,
        "module compare.empty;\n",
        "module compare.single; // final\n",
    ] {
        let (program, comments) =
            crate::parse_with_comments(source, Path::new("compare.spx")).unwrap();
        let original = super::super::canonical_with_comments(&program, &comments);
        let cases = [
            original.clone(),
            source.to_owned(),
            format!("!{original}"),
            original.trim_end().to_owned(),
            format!("{original}\n"),
        ];
        for expected in cases {
            let comparison = canonical_comparison(&program, &comments, &expected);
            assert_eq!(comparison.bytes, Some(original.len()));
            assert_eq!(comparison.matches, original == expected);
        }
        let (program, comments) =
            crate::parse_with_comments(&original, Path::new("compare.spx")).unwrap();
        assert!(canonical_comparison(&program, &comments, &original).matches);
        assert_eq!(
            super::super::canonical_with_comments(&program, &comments),
            original
        );
    }
}

#[test]
fn comparison_accepts_every_later_write_after_mismatch_and_checked_length_overflow() {
    let mut output = Compare::new("é");
    output.write_str("!").unwrap();
    assert!(output.mismatch);
    output.write_char('🌍').unwrap();
    output.write_str("later").unwrap();
    assert_eq!(output.offset, 10);
    assert_eq!(output.total, Some(10));
    assert!(!output.finish().matches);
    let mut overflow = Compare {
        expected: b"",
        offset: usize::MAX - 1,
        total: Some(usize::MAX - 1),
        mismatch: false,
    };
    overflow.write_str("abc").unwrap();
    overflow.write_str("later").unwrap();
    overflow.write_char('é').unwrap();
    assert!(overflow.total.is_none());
    assert!(!overflow.finish().matches);
    let mut short = Compare::new("prefix suffix");
    short.write_str("prefix").unwrap();
    assert!(!short.finish().matches);
}
