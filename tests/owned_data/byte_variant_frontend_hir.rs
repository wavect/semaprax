use std::path::Path;

use semaprax::hir::{
    self, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedMatchMode, ResolvedMatchPattern,
    ResolvedProgram, ResolvedType,
};
use semaprax::{parse, verify};

const SOURCE: &str = include_str!("../owned_byte_variant_v1_fixture.spx");

fn resolved(source: &str) -> ResolvedProgram {
    let parsed = parse(source, Path::new("owned-byte-variant-frontend-hir-v1.spx")).unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics.iter().all(|item| !item.severity.is_error()),
        "unexpected diagnostics: {diagnostics:?}"
    );
    hir::resolve(&parsed).unwrap()
}

fn tail(expression: &ResolvedExpr) -> &ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &expression.kind else {
        panic!("expected block")
    };
    tail
}

fn diagnostic_codes(source: &str) -> Vec<&'static str> {
    let parsed = parse(source, Path::new("owned-byte-variant-negative.spx")).unwrap();
    verify::verify(&parsed)
        .into_iter()
        .filter(|item| item.severity.is_error())
        .map(|item| item.code)
        .collect()
}

#[test]
fn authored_and_exact_prelude_owned_byte_variants_resolve_with_exact_binding_modes() {
    let program = resolved(SOURCE);
    for (function_id, expected_mode, expected_payload_ownership) in [
        (
            "sum.inspect",
            ResolvedMatchMode::Borrow,
            OwnershipMode::Borrow,
        ),
        ("sum.consume", ResolvedMatchMode::Own, OwnershipMode::Own),
        (
            "sum.consume-option",
            ResolvedMatchMode::Own,
            OwnershipMode::Own,
        ),
        (
            "sum.consume-result",
            ResolvedMatchMode::Own,
            OwnershipMode::Own,
        ),
    ] {
        let function = program
            .functions
            .iter()
            .find(|item| item.id.as_str() == function_id)
            .unwrap();
        let ResolvedExprKind::Match { mode, arms, .. } = &tail(&function.body).kind else {
            panic!("{function_id} does not resolve to a match")
        };
        assert_eq!(*mode, expected_mode);
        let payload = arms.iter().find_map(|arm| {
            let ResolvedMatchPattern::Variant { fields, .. } = &arm.pattern else {
                return None;
            };
            fields
                .iter()
                .find(|field| field.binding.ty == ResolvedType::Bytes)
                .map(|field| &field.binding)
        });
        let payload = payload.unwrap_or_else(|| panic!("missing Bytes payload in {function_id}"));
        assert_eq!(payload.ownership, expected_payload_ownership);
    }
    hir::validate(&program).unwrap();
}

#[test]
fn nested_generic_and_two_owned_result_profiles_remain_closed() {
    let nested = r#"
module invalid.nested;
record Boxed { bytes: Bytes, }
variant Bad { Value { payload: Boxed, }, }
fn main() -> i64 { 0 }
"#;
    assert!(diagnostic_codes(nested).contains(&"SPX-T268"));

    let generic = r#"
module invalid.generic;
variant Bad<T> { Value { payload: Bytes, marker: T, }, }
fn main() -> i64 { 0 }
"#;
    assert!(diagnostic_codes(generic).contains(&"SPX-T268"));

    let two_owned = r#"
module invalid.result;
fn bad(value: own Result<Bytes, Bytes>) -> i64 { 0 }
fn main() -> i64 { 0 }
"#;
    assert!(diagnostic_codes(two_owned).contains(&"SPX-T268"));
}

#[test]
fn exact_reverse_result_and_boolean_profiles_are_admitted() {
    let source = r#"
module admitted.prelude;
fn consume_left(input: own Result<bool, Bytes>) -> i64 {
  match own input {
    Result::Ok { value } => if value { 1 } else { 0 },
    Result::Err { error } => 0,
  }
}

fn consume_right(input: own Result<Bytes, bool>) -> i64 {
  match own input {
    Result::Ok { value } => 0,
    Result::Err { error } => if error { 1 } else { 0 },
  }
}
fn main() -> i64 { 0 }
"#;
    let program = resolved(source);
    hir::validate(&program).unwrap();
}

#[test]
fn multiple_owned_cases_and_multiple_fields_keep_case_qualified_ownership() {
    let source = r#"
module admitted.multi_owned;
variant Packet {
  Empty,
  Pair { left: Bytes, right: Bytes, marker: i64, },
  Alternate { payload: Bytes, },
}
fn consume(input: own Packet) -> i64 {
  match own input {
    Packet::Empty {} => 0,
    Packet::Pair { left, right, marker } =>
      if byte_len(bytes_as_slice(left)) == 1usize &&
         byte_len(bytes_as_slice(right)) == 1usize { marker } else { 0 },
    Packet::Alternate { payload } =>
      if byte_len(bytes_as_slice(payload)) == 1usize { 1 } else { 0 },
  }
}
fn main() -> i64 { consume(Packet::Empty {}) }
"#;
    let program = resolved(source);
    let function = program
        .functions
        .iter()
        .find(|item| item.name == "consume")
        .unwrap();
    let ResolvedExprKind::Match { arms, .. } = &tail(&function.body).kind else {
        panic!("consume does not resolve to a match")
    };
    let owned_bytes = arms
        .iter()
        .flat_map(|arm| match &arm.pattern {
            ResolvedMatchPattern::Variant { fields, .. } => fields
                .iter()
                .filter(|field| field.binding.ty == ResolvedType::Bytes)
                .map(|field| field.binding.ownership)
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    assert_eq!(owned_bytes, vec![OwnershipMode::Own; 3]);
    hir::validate(&program).unwrap();
}

#[test]
fn owned_variant_modes_require_exact_patterns_and_unprojected_borrow_places() {
    let wildcard = r#"
module invalid.wildcard;
variant Packet { Data { payload: Bytes, }, }
fn bad(value: own Packet) -> i64 { match own value { _ => 0, } }
fn main() -> i64 { 0 }
"#;
    assert_eq!(diagnostic_codes(wildcard).first(), Some(&"SPX-O117"));

    let borrowed_temporary = r#"
module invalid.borrow;
variant Packet { Data { payload: Bytes, }, }
fn make(value: own Bytes) -> Packet { Packet::Data { payload: value } }
fn bad(value: own Bytes) -> i64 {
  match borrow make(value) { Packet::Data { payload } => 0, }
}
fn main() -> i64 { 0 }
"#;
    assert!(diagnostic_codes(borrowed_temporary).contains(&"SPX-O117"));
}

#[test]
fn postfix_try_does_not_consume_owned_variant_payloads() {
    let source = r#"
module invalid.owned_try;
fn bad(value: own Result<Bytes, i64>) -> Result<Bytes, i64> {
  let bytes = value?;
  Result<Bytes, i64>::Ok { value: bytes }
}
fn main() -> i64 { 0 }
"#;
    assert!(diagnostic_codes(source).contains(&"SPX-T218"));
}

#[test]
fn hostile_hir_cannot_relabel_an_owned_payload_binding_as_copy() {
    let mut program = resolved(SOURCE);
    let function = program
        .functions
        .iter_mut()
        .find(|item| item.id.as_str() == "sum.consume")
        .unwrap();
    let ResolvedExprKind::Match { arms, .. } = &mut tail_mut(&mut function.body).kind else {
        panic!("expected match")
    };
    let binding = arms
        .iter_mut()
        .find_map(|arm| match &mut arm.pattern {
            ResolvedMatchPattern::Variant { fields, .. } => fields
                .iter_mut()
                .find(|field| field.binding.ty == ResolvedType::Bytes)
                .map(|field| &mut field.binding),
            _ => None,
        })
        .unwrap();
    binding.ownership = OwnershipMode::Value;
    assert!(hir::validate(&program).is_err());
}

#[test]
fn hostile_hir_cannot_replace_an_owned_case_with_a_discarding_wildcard() {
    let mut program = resolved(SOURCE);
    let function = program
        .functions
        .iter_mut()
        .find(|item| item.id.as_str() == "sum.consume")
        .unwrap();
    let ResolvedExprKind::Match { arms, .. } = &mut tail_mut(&mut function.body).kind else {
        panic!("expected match")
    };
    arms[0].pattern = ResolvedMatchPattern::Wildcard;
    assert!(hir::validate(&program).is_err());
}

fn tail_mut(expression: &mut ResolvedExpr) -> &mut ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &mut expression.kind else {
        panic!("expected block")
    };
    tail
}
