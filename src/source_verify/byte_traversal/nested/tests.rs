//! Source only: complete v1 refusal oracle stays compiled independently.
use super::{before_selected, with_before};
const BASE: &str = r#"module nested.bytes;
@id("nested.sum") fn sum(left: borrow Slice<u8>, right: borrow Slice<u8>)->i64 {
 let mut total=0;
 for outer in left {
  for inner in right { total=total+i64_from_u8(outer)+i64_from_u8(inner); 0 }
  0
 }
 total
}
@id("nested.main") fn main()->i64 {0}
"#;
fn current(source: &str) -> Vec<crate::diagnostic::Diagnostic> {
    crate::check(source, "nested-bytes.spx")
        .err()
        .unwrap_or_default()
}
fn before(source: &str) -> Vec<crate::diagnostic::Diagnostic> {
    with_before(|| current(source))
}
#[test]
fn nested_byte_sources_available_at_outer_entry_are_admitted_and_v1_refusal_is_retained() {
    for source in [BASE.to_owned(),
        BASE.replace("for inner in right", "for inner in left"),
        BASE.replace("for inner in right {", "while false { for inner in right {")
            .replace("i64_from_u8(inner); 0 }", "i64_from_u8(inner); 0 } 0 }"),
        BASE.replace("total=total+i64_from_u8(outer)+i64_from_u8(inner); 0",
            "for third in left { total=total+i64_from_u8(outer)+i64_from_u8(inner)+i64_from_u8(third); 0 } 0")
    ] {
        assert!(current(&source).is_empty(), "{:?}", current(&source));
        let old = before(&source);
        assert!(old.iter().any(|diagnostic| diagnostic.code=="SPX-T284"));
        let ast = crate::check(&source, "nested-bytes.spx").unwrap();
        let canonical = crate::format::canonical(&ast);
        assert_eq!(crate::format::canonical(&crate::check(&canonical, "nested-bytes.spx").unwrap()),canonical);
    }
}
#[test]
fn nested_byte_closed_routes_keep_complete_old_diagnostics_and_order() {
    for source in [
        BASE.replace(
            "for inner in right",
            "for inner in byte_range(right,0usize,byte_len(right))",
        ),
        BASE.replace(
            "for inner in right",
            "let local=byte_range(right,0usize,byte_len(right)); for inner in local",
        ),
        BASE.replace("for inner in right", "for inner in missing"),
        BASE.replace("for inner in right", "for inner in total"),
        BASE.replace("for inner in right", "for own inner in right"),
        BASE.replace("for inner in right", "for borrow inner in right"),
        BASE.replace("for outer in left", "for outer in total"),
        BASE.replace(
            "let mut total=0;",
            "let mut total=0; let mut mutable=right;",
        )
        .replace("for inner in right", "for inner in mutable"),
    ] {
        let expected = before(&source);
        assert!(!expected.is_empty());
        assert_eq!(
            current(&source),
            expected,
            "full diagnostics/order: {source}"
        );
    }
}
#[test]
fn nonnested_byte_source_profile_keeps_full_old_diagnostics_and_before_scope_unwinds() {
    let scalar = BASE.replace(
        "for inner in right { total=total+i64_from_u8(outer)+i64_from_u8(inner); 0 }",
        "total=total+i64_from_u8(outer);",
    );
    for source in [
        scalar.clone(),
        scalar.replace("for outer in left", "for outer in missing"),
        scalar.replace(
            "for outer in left",
            "for outer in byte_range(left,0usize,byte_len(left))",
        ),
    ] {
        assert_eq!(current(&source), before(&source));
    }
    let panic = std::panic::catch_unwind(|| with_before(|| panic!("v1 scope unwind")));
    assert!(panic.is_err());
    assert!(!before_selected());
}
