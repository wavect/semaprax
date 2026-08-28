use std::path::Path;

use semaprax::cleanup::{FieldLivenessShape, BYTES_DROP_LIFECYCLE_ID};
use semaprax::hir::{
    self, DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedMatchMode, ResolvedMatchPattern, ResolvedProgram, ResolvedRecordMatchFieldPattern,
    ResolvedStatement, ResolvedType,
};
use semaprax::{parse, verify};

const BASE_SOURCE: &str = r#"
module test.owned_byte_record_algebra;

@id("owned.outer")
record Outer {
    @id("owned.outer.direct") direct: Bytes,
    @id("owned.outer.marker") marker: i64,
}

@id("owned.discard")
fn discard(value: own Outer) -> i64 { 0 }

@id("app.main")
fn main() -> i64 { 0 }
"#;

const MATCH_SOURCE: &str = r#"
module test.owned_byte_record_matches;

@id("owned.outer")
record Outer {
    @id("owned.outer.direct") direct: Bytes,
    @id("owned.outer.marker") marker: i64,
}

@id("owned.take")
fn take(value: own Outer) -> i64 {
    match own value { Outer { direct, marker: _ } => 0, }
}

@id("owned.inspect")
fn inspect(value: own Outer) -> Outer {
    let measured = match borrow value { Outer { direct, marker: _ } => 0, };
    value
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn resolved(source: &str) -> ResolvedProgram {
    let parsed = parse(source, Path::new("owned-byte-record-algebra-v1.spx")).unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "unexpected verification errors: {diagnostics:?}"
    );
    hir::resolve(&parsed).unwrap()
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap_or_else(|| panic!("missing resolved function {id}"))
}

fn block_tail(expression: &ResolvedExpr) -> &ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &expression.kind else {
        panic!("resolved function body is not a block")
    };
    tail
}

fn binding<'a>(pattern: &'a ResolvedMatchPattern, name: &str) -> &'a hir::ResolvedBinding {
    fn find<'a>(
        fields: &'a [hir::ResolvedRecordMatchPatternField],
        name: &str,
    ) -> Option<&'a hir::ResolvedBinding> {
        fields.iter().find_map(|field| match &field.pattern {
            ResolvedRecordMatchFieldPattern::Binding(binding) if binding.name == name => {
                Some(binding)
            }
            ResolvedRecordMatchFieldPattern::Record { fields, .. } => find(fields, name),
            ResolvedRecordMatchFieldPattern::Binding(_)
            | ResolvedRecordMatchFieldPattern::Wildcard => None,
        })
    }

    let ResolvedMatchPattern::Record { fields, .. } = pattern else {
        panic!("expected a resolved record pattern")
    };
    find(fields, name).unwrap_or_else(|| panic!("missing pattern binding {name}"))
}

fn nominal(id: &str) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments: Vec::new(),
    }
}

fn projected_byte_leaves(
    shape: &FieldLivenessShape,
    path: &mut Vec<String>,
    leaves: &mut Vec<(Vec<String>, String)>,
) {
    match shape {
        FieldLivenessShape::NoDrop => {}
        FieldLivenessShape::Leaf { lifecycle, .. } => {
            leaves.push((path.clone(), lifecycle.as_str().to_owned()));
        }
        FieldLivenessShape::Record { fields, .. } => {
            for field in fields {
                path.push(field.field.as_str().to_owned());
                projected_byte_leaves(&field.shape, path, leaves);
                path.pop();
            }
        }
        _ => panic!("unknown cleanup liveness shape"),
    }
}

fn error_codes(source: &str) -> Vec<&'static str> {
    let parsed = parse(source, Path::new("owned-byte-record-errors.spx")).unwrap();
    verify::verify(&parsed)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn direct_bytes_make_flat_records_non_copy_and_drop_aware() {
    let program = resolved(BASE_SOURCE);
    let id = "owned.outer";
    let facts = program.declarations.type_facts(&nominal(id)).unwrap();
    assert!(!facts.copy, "{id} must not be Copy");
    assert!(!facts.contains_resource, "Bytes is not a user resource");
    assert!(facts.sized, "{id} must remain statically sized");
    assert!(facts.needs_drop, "{id} must carry owned Bytes cleanup");
}

#[test]
fn match_own_consumes_the_owner_and_exposes_owned_bytes_bindings() {
    let program = resolved(MATCH_SOURCE);
    let take = function(&program, "owned.take");
    let tail = block_tail(&take.body);
    let ResolvedExprKind::Match { mode, arms, .. } = &tail.kind else {
        panic!("take tail is not a match")
    };
    assert_eq!(*mode, ResolvedMatchMode::Own);
    let binding = binding(&arms[0].pattern, "direct");
    assert_eq!(binding.ty, ResolvedType::Bytes);
    assert_eq!(binding.ownership, OwnershipMode::Own);
    assert_eq!(arms[0].value.ty, ResolvedType::I64);
}

#[test]
fn match_borrow_exposes_borrowed_bytes_and_leaves_the_owner_usable() {
    let program = resolved(MATCH_SOURCE);
    let inspect = function(&program, "owned.inspect");
    let ResolvedExprKind::Block { statements, tail } = &inspect.body.kind else {
        panic!("inspect body is not a block")
    };
    let ResolvedStatement::Let { value, .. } = &statements[0] else {
        panic!("inspect first statement is not a let")
    };
    let ResolvedExprKind::Match { mode, arms, .. } = &value.kind else {
        panic!("inspect let value is not a match")
    };
    assert_eq!(*mode, ResolvedMatchMode::Borrow);
    let binding = binding(&arms[0].pattern, "direct");
    assert_eq!(binding.ty, ResolvedType::Bytes);
    assert_eq!(binding.ownership, OwnershipMode::Borrow);
    assert_eq!(tail.ty, nominal("owned.outer"));
    assert_eq!(tail.ownership, OwnershipMode::Own);
    assert!(
        matches!(&tail.kind, ResolvedExprKind::Place(place) if place.root == inspect.params[0].id)
    );
}

#[test]
fn plain_match_on_an_owned_byte_record_is_spx_o111() {
    let source = r#"
module test.plain_owned_match;
record Outer { direct: Bytes, }
fn invalid(value: own Outer) -> i64 {
    match value { Outer { direct: _ } => 0, }
}
fn main() -> i64 { 0 }
"#;
    let codes = error_codes(source);
    assert_eq!(codes.first(), Some(&"SPX-O111"), "{codes:?}");
}

#[test]
fn explicit_modes_on_copy_records_are_spx_o117() {
    for expression in ["match own value", "match borrow value"] {
        let source = format!(
            r#"
module test.invalid_explicit_match;
record Pair {{ left: i64, right: i64, }}
fn invalid(value: Pair) -> i64 {{
    {expression} {{ Pair {{ left: _, right: _ }} => 0, }}
}}
fn main() -> i64 {{ 0 }}
"#
        );
        assert_eq!(error_codes(&source), ["SPX-O117"], "{expression}");
    }
}

#[test]
fn explicit_owned_record_modes_require_the_exact_record_pattern() {
    for mode in ["own", "borrow"] {
        let source = format!(
            r#"
module test.explicit_wildcard;
record Packet {{ payload: Bytes, marker: i64, }}
fn invalid(packet: own Packet) -> i64 {{
    match {mode} packet {{ _ => 0, }}
}}
fn main() -> i64 {{ 0 }}
"#
        );
        assert!(error_codes(&source).contains(&"SPX-O117"), "{mode}");
    }
}

#[test]
fn borrowed_owned_byte_record_call_requires_an_unprojected_named_place() {
    let source = r#"
module test.borrowed_owned_byte_temporary;
record Packet { payload: Bytes, marker: i64, }
fn inspect(packet: borrow Packet) -> i64 { 0 }
fn invalid(data: borrow Slice<u8>) -> i64 {
    inspect(Packet { payload: bytes_copy(data), marker: 0 })
}
fn main() -> i64 { 0 }
"#;
    assert_eq!(error_codes(source), ["SPX-O118"]);
}

#[test]
fn nested_bytes_are_rejected_outside_the_flat_v1_scope() {
    let source = r#"
module test.nested_owned_bytes;
record Inner { payload: Bytes, }
record Outer { nested: Inner, }
fn invalid(value: own Outer) -> i64 { 0 }
fn main() -> i64 { 0 }
"#;
    assert_eq!(error_codes(source), ["SPX-T268"]);
}

#[test]
fn non_record_and_non_flat_owned_bytes_shapes_are_rejected_at_source_admission() {
    let cases = [
        r#"
module test.class_bytes;
class Packet { payload: Bytes, }
fn main() -> i64 { 0 }
"#,
        r#"
module test.variant_bytes;
record Marker { value: i64, }
variant Packet { Data { payload: Bytes, nested: Marker, }, }
fn main() -> i64 { 0 }
"#,
        r#"
module test.nested_copy_companion;
record Marker { value: i64, }
record Packet { payload: Bytes, marker: Marker, }
fn main() -> i64 { 0 }
"#,
        r#"
module test.array_companion;
record Packet { payload: Bytes, marker: [u8; 1], }
fn main() -> i64 { 0 }
"#,
        r#"
module test.slice_companion;
record Packet { payload: Bytes, marker: Slice<u8>, }
fn main() -> i64 { 0 }
"#,
        r#"
module test.string_companion;
record Packet { payload: Bytes, marker: string, }
fn main() -> i64 { 0 }
"#,
        r#"
module test.generic_owned_bytes;
record Packet<T> { payload: Bytes, marker: T, }
fn main() -> i64 { 0 }
"#,
    ];
    for source in cases {
        let codes = error_codes(source);
        assert!(codes.contains(&"SPX-T268"), "{source}\n{codes:?}");
    }
}

#[test]
fn cleanup_inventory_and_plan_preserve_the_exact_direct_byte_projection() {
    let program = resolved(BASE_SOURCE);
    let discard = function(&program, "owned.discard");
    let outer = nominal("owned.outer");
    let expected = vec![(
        vec!["owned.outer.direct".to_owned()],
        BYTES_DROP_LIFECYCLE_ID.to_owned(),
    )];

    let inventory_slot = discard
        .cleanup
        .slots
        .iter()
        .find(|slot| slot.ty == outer)
        .expect("owned Outer inventory slot");
    let mut inventory_leaves = Vec::new();
    projected_byte_leaves(
        &inventory_slot.shape,
        &mut Vec::new(),
        &mut inventory_leaves,
    );
    assert_eq!(inventory_leaves, expected);
    let inventory_places = discard
        .cleanup
        .flags
        .iter()
        .filter(|flag| flag.place.storage == inventory_slot.id)
        .map(|flag| {
            (
                flag.place
                    .projections
                    .iter()
                    .map(|projection| projection.as_str().to_owned())
                    .collect::<Vec<_>>(),
                flag.lifecycle.as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(inventory_places, expected);

    let plan_slot = discard
        .cleanup_plan
        .slots
        .iter()
        .find(|slot| slot.ty == outer)
        .expect("owned Outer cleanup-plan slot");
    let mut plan_leaves = Vec::new();
    projected_byte_leaves(
        &plan_slot.field_liveness_shape,
        &mut Vec::new(),
        &mut plan_leaves,
    );
    assert_eq!(plan_leaves, expected);
}
