//! Focused evidence for Indexed Byte Loop v2.
//!
//! The exact widening is one guard-free `Option<u8>` match over a direct
//! compiler-owned `byte_get` call inside a bounded loop. Everything else
//! remains rejected before backend emission.

use semaprax::hir::{
    self, DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedMatchPattern,
    ResolvedProgram, ResolvedStatement, ResolvedType,
};
use semaprax::{format, graph, parse, verify};
use sha2::{Digest, Sha256};

const VALID: &str = r#"
module test.indexed_byte_loops_v2;

@id("indexed.count-ff")
fn count_ff(bytes: borrow Slice<u8>) -> usize {
    let length = byte_len(bytes);
    let mut index = 0usize;
    let mut total = 0usize;
    while index <= length {
        total = total + match byte_get(bytes, index) {
            Option::Some { value: byte } => if byte == 255u8 { 1usize } else { 0usize },
            Option::None {} => 0usize,
        };
        index = index + 1usize;
        index <= length
    }
    total
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const LEGACY: &str = r#"
module test.mutation_plain;

@id("plain.stable")
fn stable() -> i64 {
    let total = 1;
    let frozen = 3;
    total + frozen
}

@id("main")
fn main() -> i64 { 0 }
"#;

fn error_codes(source: &str) -> (Vec<&'static str>, Vec<&'static str>) {
    let program = parse(source, "indexed-byte-loop-invalid.spx").unwrap();
    let source_codes = verify::verify(&program)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect();
    let resolved_codes = hir::resolve(&program)
        .unwrap_err()
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect();
    (source_codes, resolved_codes)
}

fn assert_rejected(source: &str, expected: &'static str) {
    let (source_codes, resolved_codes) = error_codes(source);
    assert!(
        source_codes.contains(&expected),
        "source verifier did not report {expected}: {source_codes:?}"
    );
    assert!(
        resolved_codes.contains(&expected),
        "resolver did not report {expected}: {resolved_codes:?}"
    );
}

fn indexed_match(program: &mut ResolvedProgram) -> &mut ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "indexed.count-ff")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        unreachable!();
    };
    let ResolvedStatement::While { body, .. } = statements
        .iter_mut()
        .find(|statement| matches!(statement, ResolvedStatement::While { .. }))
        .unwrap()
    else {
        unreachable!();
    };
    let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        unreachable!();
    };
    let ResolvedStatement::Assign { value, .. } = &mut statements[0] else {
        unreachable!();
    };
    let ResolvedExprKind::Binary { right, .. } = &mut value.kind else {
        unreachable!();
    };
    right
}

fn assert_hostile_rejected(baseline: &ResolvedProgram, mutate: impl FnOnce(&mut ResolvedExpr)) {
    let mut hostile = baseline.clone();
    mutate(indexed_match(&mut hostile));
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}

#[test]
fn exact_indexed_byte_loop_is_canonical_graph_v17_and_legacy_bytes_stay_pinned() {
    let program = parse(VALID, "indexed-byte-loop-v2.spx").unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, "indexed-byte-loop-v2-canonical.spx").unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);

    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "indexed.count-ff")
        .unwrap();
    assert!(function.cleanup.slots.is_empty());
    assert!(function.cleanup_plan.slots.is_empty());
    assert!(function
        .cleanup_plan
        .exits
        .iter()
        .all(|exit| exit.finalize_in_order.is_empty()));
    let first = graph::to_json(&program).unwrap();
    assert_eq!(first, graph::to_json(&program).unwrap());
    let wire: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(wire["schema"], "semaprax.graph.v17");
    assert!(first.contains("\"kind\":\"while\""));
    assert!(first.contains("core.bytes.get"));

    let legacy = parse(LEGACY, "indexed-byte-loop-legacy.spx").unwrap();
    let legacy_json = graph::to_json(&legacy).unwrap();
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(legacy_json.as_bytes()))
    );
    assert_eq!(
        digest,
        "sha256:6fe42635e96022507876aabd25acfe06f28521aba50132a5dc16b5070c45cfa7"
    );
}

#[test]
fn malformed_general_or_effectful_loop_matches_are_exact_t252() {
    assert_rejected(
        &VALID.replace(
            "Option::Some { value: byte } =>",
            "Option::Some { value: byte } if byte == 255u8 =>",
        ),
        "SPX-T252",
    );
    assert_rejected(
        &VALID.replace("match byte_get(bytes, index)", "match Option<u8>::None {}"),
        "SPX-T252",
    );
    assert_rejected(
        &VALID.replace(
            "byte_get(bytes, index)",
            "byte_get(array_as_slice([255u8]), index)",
        ),
        "SPX-T252",
    );
    let allocation = VALID.replace(
        "Option::Some { value: byte } => if byte == 255u8 { 1usize } else { 0usize }",
        "Option::Some { value: byte } => { let copied = bytes_copy(bytes); let view = bytes_as_slice(copied); if byte == 255u8 { byte_len(view) } else { 0usize } }",
    );
    assert_rejected(&allocation, "SPX-T252");
    assert_rejected(&allocation, "SPX-T267");
}

#[test]
fn hostile_hir_cannot_forge_indexed_match_identity_inventory_or_types() {
    let program = parse(VALID, "indexed-byte-loop-hostile.spx").unwrap();
    let baseline = hir::resolve(&program).unwrap();

    assert_hostile_rejected(&baseline, |expression| {
        let ResolvedExprKind::Match { arms, .. } = &mut expression.kind else {
            unreachable!();
        };
        arms[0].guard = Some(Box::new(arms[0].value.clone()));
    });
    assert_hostile_rejected(&baseline, |expression| {
        let ResolvedExprKind::Match { scrutinee, .. } = &mut expression.kind else {
            unreachable!();
        };
        let ResolvedExprKind::Call { callee, .. } = &mut scrutinee.kind else {
            unreachable!();
        };
        *callee = DeclarationId::new("forged.byte.get");
    });
    assert_hostile_rejected(&baseline, |expression| {
        let ResolvedExprKind::Match { arms, .. } = &mut expression.kind else {
            unreachable!();
        };
        arms.pop();
    });
    assert_hostile_rejected(&baseline, |expression| {
        let ResolvedExprKind::Match { arms, .. } = &mut expression.kind else {
            unreachable!();
        };
        let ResolvedMatchPattern::Variant { variant, .. } = &mut arms[0].pattern else {
            unreachable!();
        };
        *variant = DeclarationId::new("forged.option");
    });
    assert_hostile_rejected(&baseline, |expression| {
        let ResolvedExprKind::Match { arms, .. } = &mut expression.kind else {
            unreachable!();
        };
        let ResolvedMatchPattern::Variant { case, .. } = &mut arms[0].pattern else {
            unreachable!();
        };
        *case = DeclarationId::new("forged.option.case");
    });
    assert_hostile_rejected(&baseline, |expression| {
        let ResolvedExprKind::Match { arms, .. } = &mut expression.kind else {
            unreachable!();
        };
        let ResolvedMatchPattern::Variant { fields, .. } = &mut arms[0].pattern else {
            unreachable!();
        };
        fields[0].field = DeclarationId::new("forged.option.field");
    });
    assert_hostile_rejected(&baseline, |expression| {
        let ResolvedExprKind::Match { arms, .. } = &mut expression.kind else {
            unreachable!();
        };
        let ResolvedMatchPattern::Variant { fields, .. } = &mut arms[0].pattern else {
            unreachable!();
        };
        fields[0].binding.ty = ResolvedType::Bool;
        fields[0].binding.ownership = OwnershipMode::Borrow;
    });
    assert_hostile_rejected(&baseline, |expression| {
        expression.ownership = OwnershipMode::Borrow;
    });
}
