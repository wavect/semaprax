use super::*;
use std::fmt::Write as _;
use std::path::Path;

const SOURCE: &str = "// héader 🌍\r\nmodule compare.comments;\r\n\r\n// declaration\r\n@id(\"compare.main\") fn main()->i64 {\r\n let x=1; // trailing λ\r\n while x<0 { // nested\r\n  x<0 // tail\r\n }\r\n x\r\n // closing\r\n}\r\n// final Ω  \r\n";

fn comparison_fixture(source: &str, module_only: bool) -> (crate::ast::Program, Comments) {
    let path = Path::new("compare.spx");
    if !module_only {
        return crate::parse_with_comments(source, path).unwrap();
    }
    // Empty declaration sequences are formatter edge cases, not admitted
    // source modules. Preserve their exact corpus bytes and parser refusal.
    let refusal = crate::parse_with_comments(source, path).unwrap_err();
    assert_eq!(refusal.code, "SPX-P101");
    assert_eq!(refusal.message, "a module must declare at least one function");
    let anchored = format!("{source}\n@id(\"compare.fixture.anchor\") fn main()->i64 {{ 0 }}\n");
    let (mut program, comments) = crate::parse_with_comments(&anchored, path).unwrap();
    assert_eq!(program.functions.len(), 1);
    program.functions.clear();
    let (_, original_comments) = crate::lexer::lex_with_comments(source, "compare.spx").unwrap();
    assert_eq!(comments, original_comments);
    (program, original_comments)
}

#[test]
fn full_commented_comparison_keeps_old_unicode_crlf_trailing_nested_and_noncanonical_bytes() {
    for (source, module_only) in [
        (SOURCE, false),
        ("module compare.empty;\n", true),
        ("module compare.single; // final\n", true),
    ] {
        let (program, comments) = comparison_fixture(source, module_only);
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
        let (program, comments) = comparison_fixture(&original, module_only);
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
