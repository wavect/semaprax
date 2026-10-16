use std::path::Path;

use semaprax::cleanup::{FieldLivenessShape, BYTES_DROP_LIFECYCLE_ID};
use semaprax::hir::{
    self, DeclarationId, OwnershipMode, PlaceProjection, ResolvedExpr, ResolvedExprKind,
    ResolvedMatchMode, ResolvedMatchPattern, ResolvedProgram, ResolvedRecordMatchFieldPattern,
};
use semaprax::loan_plan::LoanCause;
use semaprax::{parse, verify};

const NESTED_SOURCE: &str = r#"
module test.nested_owned_record_frontend_hir;

@id("nested.leaf")
record Leaf {
    @id("nested.leaf.payload") payload: Bytes,
    @id("nested.leaf.marker") marker: i64,
}

@id("nested.branch")
record Branch {
    @id("nested.branch.leaf") leaf: Leaf,
    @id("nested.branch.enabled") enabled: bool,
}

@id("nested.envelope")
record Envelope {
    @id("nested.envelope.left") left: Branch,
    @id("nested.envelope.right") right: Branch,
    @id("nested.envelope.sequence") sequence: usize,
}

@id("nested.inspect")
fn inspect(packet: own Envelope) -> usize {
    let left = bytes_as_slice(packet.left.leaf.payload);
    let right = bytes_as_slice(packet.right.leaf.payload);
    byte_len(left) + byte_len(right)
}

@id("nested.identity")
fn identity(packet: own Envelope) -> Envelope { packet }

@id("app.main")
fn main() -> i64 { 0 }
"#;

const NESTED_MATCH_SOURCE: &str = r#"
module test.nested_owned_record_exact_destructuring;

@id("nested.match.leaf")
record Leaf {
    @id("nested.match.leaf.payload") payload: Bytes,
    @id("nested.match.leaf.marker") marker: i64,
}

@id("nested.match.branch")
record Branch {
    @id("nested.match.branch.leaf") leaf: Leaf,
    @id("nested.match.branch.enabled") enabled: bool,
}

@id("nested.match.envelope")
record Envelope {
    @id("nested.match.envelope.left") left: Branch,
    @id("nested.match.envelope.right") right: Branch,
    @id("nested.match.envelope.sequence") sequence: i64,
}

@id("nested.match.consume")
fn consume(packet: own Envelope) -> i64 {
    match own packet {
        Envelope {
            left: Branch { leaf: Leaf { payload: left_payload, marker: _ }, enabled: _ },
            right: Branch { leaf: Leaf { payload: right_payload, marker: _ }, enabled: _ },
            sequence,
        } => sequence,
    }
}

@id("nested.match.inspect")
fn inspect(packet: own Envelope) -> i64 {
    let measured = match borrow packet {
        Envelope {
            left: Branch { leaf: Leaf { payload: left_payload, marker: _ }, enabled: _ },
            right: Branch { leaf: Leaf { payload: right_payload, marker: _ }, enabled: _ },
            sequence: _,
        } => {
            let left_view = bytes_as_slice(left_payload);
            let right_view = bytes_as_slice(right_payload);
            if byte_len(left_view) == byte_len(right_view) { 1 } else { 0 }
        },
    };
    let after = bytes_as_slice(packet.right.leaf.payload);
    if byte_len(after) == 0usize { measured } else { measured }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let parsed = parse(source, Path::new("nested-owned-records-v1.spx")).expect("source parses");
    verify::verify(&parsed)
}

fn resolved() -> ResolvedProgram {
    let parsed = parse(NESTED_SOURCE, Path::new("nested-owned-records-v1.spx"))
        .expect("nested fixture parses");
    let report = verify::verify(&parsed);
    assert!(
        report
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "nested fixture source verification failed: {report:?}"
    );
    hir::resolve(&parsed).expect("nested fixture resolves and validates")
}

fn projection_ids(projections: &[PlaceProjection]) -> Vec<&str> {
    projections
        .iter()
        .map(|projection| match projection {
            PlaceProjection::Field(field) => field.as_str(),
            PlaceProjection::VariantField { .. } => panic!("record path contains variant step"),
        })
        .collect()
}

fn block_tail(expression: &ResolvedExpr) -> &ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &expression.kind else {
        panic!("resolved function body is not a block")
    };
    tail
}

fn pattern_binding<'a>(pattern: &'a ResolvedMatchPattern, name: &str) -> &'a hir::ResolvedBinding {
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
    find(fields, name).unwrap_or_else(|| panic!("missing nested binding {name}"))
}

#[test]
fn exact_nested_owned_and_borrowed_patterns_retain_binding_modes_and_owner() {
    let parsed = parse(
        NESTED_MATCH_SOURCE,
        Path::new("nested-owned-record-exact-destructuring-v1.spx"),
    )
    .expect("nested match fixture parses");
    let report = verify::verify(&parsed);
    assert!(
        report
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "nested match fixture source verification failed: {report:?}"
    );
    let program = hir::resolve(&parsed).expect("nested match fixture resolves and validates");

    let consume = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "nested.match.consume")
        .expect("consume function");
    let ResolvedExprKind::Match { mode, arms, .. } = &block_tail(&consume.body).kind else {
        panic!("consume tail is not a match")
    };
    assert_eq!(*mode, ResolvedMatchMode::Own);
    for name in ["left_payload", "right_payload"] {
        let binding = pattern_binding(&arms[0].pattern, name);
        assert_eq!(binding.ownership, OwnershipMode::Own);
        assert_eq!(binding.ty, hir::ResolvedType::Bytes);
    }

    let inspect = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "nested.match.inspect")
        .expect("inspect function");
    let ResolvedExprKind::Block { statements, .. } = &inspect.body.kind else {
        panic!("inspect body is not a block")
    };
    let hir::ResolvedStatement::Let { value, .. } = &statements[0] else {
        panic!("inspect first statement is not a let")
    };
    let ResolvedExprKind::Match { mode, arms, .. } = &value.kind else {
        panic!("inspect binding is not a match")
    };
    assert_eq!(*mode, ResolvedMatchMode::Borrow);
    for name in ["left_payload", "right_payload"] {
        let binding = pattern_binding(&arms[0].pattern, name);
        assert_eq!(binding.ownership, OwnershipMode::Borrow);
        assert_eq!(binding.ty, hir::ResolvedType::Bytes);
    }
    let ResolvedExprKind::Block { statements, .. } = &arms[0].value.kind else {
        panic!("borrowed match arm is not a block")
    };
    let expected = [
        (
            "left_view",
            vec![
                "nested.match.envelope.left",
                "nested.match.branch.leaf",
                "nested.match.leaf.payload",
            ],
        ),
        (
            "right_view",
            vec![
                "nested.match.envelope.right",
                "nested.match.branch.leaf",
                "nested.match.leaf.payload",
            ],
        ),
    ];
    for (statement, (name, path)) in statements.iter().zip(expected) {
        let hir::ResolvedStatement::Let { binding, .. } = statement else {
            panic!("borrowed match view is not a let")
        };
        assert_eq!(binding.name, name);
        let provenance = program
            .declarations
            .byte_slice_provenance(&binding.id)
            .expect("borrowed pattern view has canonical provenance");
        assert_eq!(provenance.root, inspect.params[0].id);
        assert_eq!(projection_ids(&provenance.projections), path);
    }
}

const ORIGINAL_STRING_PATTERN: &str = r#"
module test.nested_pattern_closed_shape;
record Packet { payload: Bytes, text: string, }
fn invalid(packet: own Packet) -> i64 {
    match own packet { Packet { payload, text } => 0, }
}
fn main() -> i64 { 0 }
"#;

#[test]
fn original_mixed_string_pattern_is_admitted_by_the_current_owned_text_profile() {
    super::byte_record_algebra::assert_mixed_string_record_current_profile(ORIGINAL_STRING_PATTERN);
}

#[test]
fn excluded_owned_byte_record_shape_is_rejected_before_pattern_resolution() {
    let source =
        ORIGINAL_STRING_PATTERN.replace("text: string,", "text: string, unsupported: [u8; 1],");
    let errors = diagnostics(&source);
    // Arrays remain outside both the byte-only and newer text-record profiles.
    // The same stable declaration error precedes owned pattern resolution.
    let failures = errors
        .iter()
        .filter(|diagnostic| diagnostic.severity == semaprax::diagnostic::Severity::Error)
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), 1, "excluded record diagnostics: {errors:?}");
    assert_eq!(failures[0].code, "SPX-T268");
    assert_eq!(failures[0].message,
        "owned-Bytes record `Packet` must be a monomorphic acyclic record tree with only `Bytes` or direct Copy scalar leaves");
}

#[test]
fn nested_whole_owners_and_multi_projection_loans_retain_stable_field_paths() {
    let program = resolved();
    let inspect = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "nested.inspect")
        .expect("inspect function");
    let mut paths = inspect
        .loan_plan
        .loans
        .iter()
        .filter(|loan| loan.cause == LoanCause::SliceView)
        .map(|loan| projection_ids(&loan.origin.projections))
        .collect::<Vec<_>>();
    paths.sort_unstable();
    assert_eq!(
        paths,
        vec![
            vec![
                "nested.envelope.left",
                "nested.branch.leaf",
                "nested.leaf.payload",
            ],
            vec![
                "nested.envelope.right",
                "nested.branch.leaf",
                "nested.leaf.payload",
            ],
        ]
    );

    let identity = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "nested.identity")
        .expect("identity function");
    assert_eq!(identity.params[0].ty, identity.return_type);
}

#[test]
fn nested_projection_move_conflict_remains_path_exact() {
    let source = NESTED_SOURCE.replace(
        "byte_len(left) + byte_len(right)",
        "let moved = identity(packet); byte_len(left) + byte_len(right)",
    );
    assert!(
        diagnostics(&source)
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T265"),
        "parent transfer while nested loans remain live must fail"
    );
}

#[test]
fn forged_nested_stable_field_paths_fail_hir_replay() {
    let mut forged_loan = resolved();
    let inspect = forged_loan
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "nested.inspect")
        .expect("inspect function");
    let loan = inspect
        .loan_plan
        .loans
        .iter_mut()
        .find(|loan| loan.cause == LoanCause::SliceView)
        .expect("nested slice loan");
    *loan.origin.projections.last_mut().expect("leaf projection") =
        PlaceProjection::Field(DeclarationId::new("nested.leaf.marker"));
    assert_eq!(hir::validate(&forged_loan).unwrap_err().code, "SPX-H006");
}

#[test]
fn exact_named_nested_record_updates_and_flat_legacy_updates_are_admitted() {
    let nested = r#"
module test.nested_update_closed;
@id("update.leaf") record Leaf {
  @id("update.leaf.payload") payload: Bytes,
}
@id("update.root") record Root {
  @id("update.root.leaf") leaf: Leaf,
  @id("update.root.marker") marker: i64,
}
@id("update.nested") fn update(value: own Root) -> Root {
  value with { marker: 1 }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let errors = diagnostics(nested);
    assert!(
        errors
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "exact named nested owned-record updates are admitted: {errors:?}"
    );

    let flat = r#"
module test.flat_update_retained;
@id("update.packet") record Packet {
  @id("update.packet.payload") payload: Bytes,
  @id("update.packet.marker") marker: i64,
}
@id("update.flat") fn update(value: own Packet) -> Packet {
  value with { marker: 1 }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let errors = diagnostics(flat);
    assert!(
        errors
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "legacy flat owned-Bytes updates remain admitted: {errors:?}"
    );
}

const ORIGINAL_STRING_COMPANION: &str = r#"
module test.nested_string_closed;
record Root { payload: Bytes, text: string, }
fn main() -> i64 { 0 }
"#;

#[test]
fn original_nested_string_companion_is_admitted_by_the_current_owned_text_profile() {
    super::byte_record_algebra::assert_mixed_string_record_current_profile(
        ORIGINAL_STRING_COMPANION,
    );
}

#[test]
fn variants_generics_classes_and_noncopy_leaves_stay_closed() {
    let unsupported_string =
        ORIGINAL_STRING_COMPANION.replace("text: string,", "text: string, unsupported: [u8; 1],");
    let cases = [
        r#"
module test.nested_variant_closed;
variant Choice { Some { payload: Bytes, }, }
record Root { choice: Choice, }
fn main() -> i64 { 0 }
"#,
        r#"
module test.nested_generic_closed;
record Leaf { payload: Bytes, }
record Root<T> { leaf: Leaf, marker: T, }
fn main() -> i64 { 0 }
"#,
        r#"
module test.nested_class_closed;
class Metadata { marker: i64, }
record Root { payload: Bytes, metadata: Metadata, }
fn main() -> i64 { 0 }
"#,
        unsupported_string.as_str(),
    ];
    for source in cases {
        assert!(
            diagnostics(source)
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-T268"),
            "forbidden nested shape was admitted:\n{source}"
        );
    }
}

fn nested_depth_source(depth: usize) -> String {
    let mut source = String::from("module test.nested_depth_bound;\n");
    for index in (0..depth).rev() {
        source.push_str(&format!("@id(\"depth.r{index}\") record R{index} {{ "));
        if index + 1 == depth {
            source.push_str(&format!(
                "@id(\"depth.r{index}.payload\") payload: Bytes, }}\n"
            ));
        } else {
            source.push_str(&format!(
                "@id(\"depth.r{index}.next\") next: R{}, }}\n",
                index + 1
            ));
        }
    }
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");
    source
}

fn wide_record_source(fields: usize, bytes_fields: usize) -> String {
    let mut source =
        String::from("module test.nested_width_bound;\n@id(\"width.root\") record Root {\n");
    for index in 0..fields {
        let ty = if index < bytes_fields {
            "Bytes"
        } else {
            "bool"
        };
        source.push_str(&format!("@id(\"width.root.f{index}\") f{index}: {ty},\n"));
    }
    source.push_str("}\n@id(\"app.main\") fn main() -> i64 { 0 }\n");
    source
}

fn error_codes(source: &str) -> Vec<&'static str> {
    diagnostics(source)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn concrete_generic_owned_record_substitution_is_exact_in_hir_and_cleanup() {
    let source = r#"
module test.generic_owned_record_frontend_hir;
@id("generic.frontend.pair") record Pair<T, U> {
  @id("generic.frontend.pair.left") left: T,
  @id("generic.frontend.pair.right") right: U,
}

@id("generic.frontend.consume") fn consume(packet: own Pair<Bytes, bool>) -> i64 {
  match own packet {
    Pair { left: payload, right: enabled } =>
      if enabled && byte_len(bytes_as_slice(payload)) == 0usize { 1 } else { 0 },
  }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parsed = parse(source, Path::new("generic-owned-record-frontend-v1.spx")).unwrap();
    let report = verify::verify(&parsed);
    assert!(
        report
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "generic owned record source verification failed: {report:?}"
    );
    let program = hir::resolve(&parsed).expect("generic owned record resolves and validates");
    let consume = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "generic.frontend.consume")
        .expect("consume function");
    let expected_ty = hir::ResolvedType::Nominal {
        declaration: DeclarationId::new("generic.frontend.pair"),
        arguments: vec![hir::ResolvedType::Bytes, hir::ResolvedType::Bool],
    };
    assert_eq!(consume.params[0].ty, expected_ty);
    let slot = consume
        .cleanup
        .slots
        .iter()
        .find(|slot| slot.ty == expected_ty)
        .expect("concrete generic owner has a cleanup slot");
    let FieldLivenessShape::Record {
        declaration,
        fields,
    } = &slot.shape
    else {
        panic!("concrete generic owner does not retain its record shape")
    };
    assert_eq!(declaration.as_str(), "generic.frontend.pair");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].field.as_str(), "generic.frontend.pair.left");
    assert!(matches!(
        &fields[0].shape,
        FieldLivenessShape::Leaf { lifecycle, .. }
            if lifecycle.as_str() == BYTES_DROP_LIFECYCLE_ID
    ));
    assert!(matches!(fields[1].shape, FieldLivenessShape::NoDrop));

    let ResolvedExprKind::Match { arms, .. } = &block_tail(&consume.body).kind else {
        panic!("consume tail is not a match")
    };
    assert_eq!(
        pattern_binding(&arms[0].pattern, "payload").ty,
        hir::ResolvedType::Bytes
    );
    assert_eq!(
        pattern_binding(&arms[0].pattern, "enabled").ty,
        hir::ResolvedType::Bool
    );
}

#[test]
fn concrete_generic_owned_record_accepts_every_direct_copy_scalar_substitution() {
    let cases = [
        ("u8", hir::ResolvedType::U8),
        ("i32", hir::ResolvedType::I32),
        ("usize", hir::ResolvedType::Usize),
        ("char", hir::ResolvedType::Char),
        ("f32", hir::ResolvedType::F32),
        ("f64", hir::ResolvedType::F64),
    ];
    for (source_name, resolved_scalar) in cases {
        let source = format!(
            r#"
module test.generic_owned_record_copy_scalar;
@id("generic.copy.pair") record Pair<T, U> {{
  @id("generic.copy.pair.payload") payload: T,
  @id("generic.copy.pair.marker") marker: U,
}}
@id("generic.copy.consume") fn consume(packet: own Pair<Bytes, {source_name}>) -> i64 {{
  match own packet {{ Pair {{ payload: payload, marker: marker }} => 0, }}
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
        );
        let parsed = parse(
            &source,
            Path::new("generic-owned-record-copy-scalars-v1.spx"),
        )
        .unwrap();
        let report = verify::verify(&parsed);
        assert!(
            report
                .iter()
                .all(|diagnostic| !diagnostic.severity.is_error()),
            "{source_name}: {report:?}"
        );
        let program = hir::resolve(&parsed).unwrap();
        let consume = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "generic.copy.consume")
            .unwrap();
        let expected_ty = hir::ResolvedType::Nominal {
            declaration: DeclarationId::new("generic.copy.pair"),
            arguments: vec![hir::ResolvedType::Bytes, resolved_scalar.clone()],
        };
        assert_eq!(consume.params[0].ty, expected_ty);
        let slot = consume
            .cleanup
            .slots
            .iter()
            .find(|slot| slot.ty == expected_ty)
            .unwrap();
        let FieldLivenessShape::Record { fields, .. } = &slot.shape else {
            panic!("{source_name}: concrete owner shape is not a record")
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].field.as_str(), "generic.copy.pair.payload");
        assert_eq!(fields[1].field.as_str(), "generic.copy.pair.marker");
        assert!(matches!(
            &fields[0].shape,
            FieldLivenessShape::Leaf { lifecycle, .. }
                if lifecycle.as_str() == BYTES_DROP_LIFECYCLE_ID
        ));
        assert!(matches!(fields[1].shape, FieldLivenessShape::NoDrop));
        let ResolvedExprKind::Match { arms, .. } = &block_tail(&consume.body).kind else {
            panic!("{source_name}: consume tail is not a match")
        };
        assert_eq!(
            pattern_binding(&arms[0].pattern, "marker").ty,
            resolved_scalar
        );
    }
}

#[test]
fn generic_owned_record_profile_rejects_noncopy_nested_class_and_variant_arguments() {
    let cases = [
        (
            r#"
module test.generic_owned_string_closed;
record Box<T> { value: T, }
fn reject(value: own Box<string>) -> i64 { 0 }
fn main() -> i64 { 0 }
"#,
            "SPX-T223",
        ),
        (
            r#"
module test.generic_owned_class_closed;
class Box<T> { value: T, }
fn reject(value: own Box<Bytes>) -> i64 { 0 }
fn main() -> i64 { 0 }
"#,
            "SPX-T268",
        ),
        (
            r#"
module test.generic_owned_variant_closed;
variant Choice<T> { Some { value: T, }, }
fn reject(value: own Choice<Bytes>) -> i64 { 0 }
fn main() -> i64 { 0 }
"#,
            "SPX-T268",
        ),
    ];
    for (source, code) in cases {
        let errors = diagnostics(source);
        assert!(
            errors.iter().any(|diagnostic| diagnostic.code == code),
            "missing {code} for closed generic owned shape: {errors:?}"
        );
    }
}

#[test]
fn bounded_nested_concrete_generic_storage_is_admitted() {
    for source in [
        r#"
module test.generic_owned_nested;
record Box<T> { value: T, }
record Outer<T> { value: T, }
fn consume(value: own Outer<Box<Bytes>>) -> i64 { 0 }
fn main() -> i64 { 0 }
"#,
        r#"
module test.generic_owned_nested_storage;
record Box<T> { value: T, }
record Outer { value: Box<Bytes>, }
fn consume(value: own Outer) -> i64 { 0 }
fn main() -> i64 { 0 }
"#,
    ] {
        let errors = diagnostics(source);
        assert!(
            errors
                .iter()
                .all(|diagnostic| !diagnostic.severity.is_error()),
            "bounded concrete nested generic storage is admitted: {errors:?}"
        );
        let parsed = parse(source, Path::new("nested-concrete-generic-storage-v1.spx")).unwrap();
        hir::resolve(&parsed).expect("bounded concrete nested generic storage resolves");
    }
}

#[test]
fn nested_record_depth_and_owned_leaf_bounds_are_exact() {
    assert!(error_codes(&nested_depth_source(64)).is_empty());
    assert!(error_codes(&nested_depth_source(65)).contains(&"SPX-T268"));

    assert!(error_codes(&wide_record_source(256, 256)).is_empty());
    assert!(error_codes(&wide_record_source(257, 257)).contains(&"SPX-T268"));
}

#[test]
fn nested_record_visited_field_bound_is_exact() {
    assert!(error_codes(&wide_record_source(4_096, 1)).is_empty());
    assert!(error_codes(&wide_record_source(4_097, 1)).contains(&"SPX-T268"));
}

#[test]
fn recursive_destructuring_rejects_nonplace_own_and_concealed_owned_subtrees() {
    let prefix = r#"
module test.nested_pattern_exact_closure;
@id("exact.leaf") record Leaf {
  @id("exact.leaf.payload") payload: Bytes,
  @id("exact.leaf.marker") marker: i64,
}
@id("exact.branch") record Branch {
  @id("exact.branch.leaf") leaf: Leaf,
  @id("exact.branch.enabled") enabled: bool,
}
@id("exact.root") record Root {
  @id("exact.root.branch") branch: Branch,
  @id("exact.root.marker") marker: i64,
}
"#;
    let cases = [
        (
            r#"
@id("exact.projected") fn invalid(root: own Root) -> i64 {
  match own root.branch {
    Branch { leaf: Leaf { payload, marker: _ }, enabled: _ } => 0,
  }
}
"#,
            "nested `match own` requires an exact named owned record place",
        ),
        (
            r#"
@id("exact.whole-binding") fn invalid(root: borrow Root) -> i64 {
  match borrow root { Root { branch, marker: _ } => 0, }
}
"#,
            "nested owned-record fields require recursive record patterns",
        ),
        (
            r#"
@id("exact.borrow-wildcard") fn invalid(root: borrow Root) -> i64 {
  match borrow root { Root { branch: _, marker: _ } => 0, }
}
"#,
            "exact owned-record patterns cannot wildcard an owned field",
        ),
    ];
    for (body, expected) in cases {
        let source = format!("{prefix}{body}@id(\"app.main\") fn main() -> i64 {{ 0 }}\n");
        let errors = diagnostics(&source);
        assert!(
            errors.iter().any(|diagnostic| {
                diagnostic.code == "SPX-O117" && diagnostic.message.contains(expected)
            }),
            "missing exact recursive-pattern closure `{expected}`: {errors:?}"
        );
    }
}

#[test]
fn subtree_binding_counts_the_entire_hidden_type_closure_at_inherited_limits() {
    // The independently constructed type inventories retain every hidden field;
    // the match itself visits only Wrapper.child. No pattern-node discount.
    for (label, source, child, admitted) in [
        ("depth-exact", nested_depth_source(63), "R0", true),
        ("depth-short", nested_depth_source(64), "R0", false),
        ("leaves-exact", wide_record_source(256, 256), "Root", true),
        ("leaves-short", wide_record_source(257, 257), "Root", false),
        ("fields-exact", wide_record_source(4_095, 1), "Root", true),
        ("fields-short", wide_record_source(4_096, 1), "Root", false),
    ] {
        let source = format!("{source}\n@id(\"hidden.wrapper\") record Wrapper {{ @id(\"hidden.wrapper.child\") child:{child}, }}\n@id(\"hidden.take\") fn take(root:own Wrapper)->i64 {{match own root {{Wrapper{{child}}=>0,}}}}\n");
        let errors = diagnostics(&source);
        if admitted {
            assert!(
                errors.iter().all(|d| !d.severity.is_error()),
                "{label}: {errors:?}"
            );
            let ast = parse(&source, Path::new("hidden-subtree-limits.spx")).unwrap();
            let resolved = hir::resolve(&ast).unwrap();
            hir::validate(&resolved).unwrap();
        } else {
            assert!(
                errors.iter().any(|d| d.code == "SPX-T268"),
                "{label}: {errors:?}"
            );
        }
    }
}

fn assert_shape_recovery_diagnostic_fields(
    actual: &semaprax::diagnostic::Diagnostic,
    expected: &semaprax::diagnostic::Diagnostic,
) {
    assert_eq!(actual.code, expected.code);
    assert_eq!(actual.severity, expected.severity);
    assert_eq!(actual.message, expected.message);
    assert_eq!(actual.path, expected.path);
    assert_eq!(actual.span, expected.span);
    assert_eq!(actual.help, expected.help);
}

#[test]
fn excluded_record_recovery_retains_independent_arm_body_parameter_and_declaration_errors() {
    let excluded = ORIGINAL_STRING_PATTERN.replace("text: string,", "text: string, unsupported: [u8; 1],");
    let baseline = diagnostics(&excluded);
    let primary = baseline.iter().find(|d| d.code == "SPX-T268").unwrap();
    for (source, independent_codes) in [
        (excluded.replace("=> 0,", "=> independent_unknown,"), vec!["SPX-T202"]),
        (excluded.replace("packet: own Packet", "packet: own Packet, independent: Bytes"), vec!["SPX-T263"]),
        (format!("{excluded}\nfn unrelated()->i64{{outside_unknown}}\n"), vec!["SPX-T202"]),
        (format!("{excluded}\nrecord Unrelated {{ payload: Bytes, unsupported: [u8; 1], }}\n"), vec!["SPX-T268"]),
        (excluded.replace("Packet { payload, text }", "Packet { payload: same, text: same }"), vec!["SPX-M104"]),
    ] {
        // Blank only this excluded field, retaining every following byte offset.
        // The independent valid-record route therefore supplies exact six-field
        // expected diagnostics, without source-position normalization.
        let field = "unsupported: [u8; 1],";
        let valid = source.replacen(field, &" ".repeat(field.len()), 1);
        let actual = diagnostics(&source).into_iter().filter(|d| d.severity.is_error()).collect::<Vec<_>>();
        let expected = diagnostics(&valid).into_iter().filter(|d| d.severity.is_error()).collect::<Vec<_>>();
        assert_eq!(expected.iter().map(|d| d.code).collect::<Vec<_>>(), independent_codes);
        assert_eq!(actual.len(), expected.len() + 1, "{actual:?}");
        assert_shape_recovery_diagnostic_fields(&actual[0], primary);
        for (actual, expected) in actual[1..].iter().zip(&expected) {
            assert_shape_recovery_diagnostic_fields(actual, expected);
        }
    }
}

#[test]
fn valid_record_missing_field_and_plain_match_mode_diagnostics_remain_independent() {
    let missing = ORIGINAL_STRING_PATTERN.replace("Packet { payload, text }", "Packet { payload }");
    let failures = diagnostics(&missing).into_iter().filter(|d| d.severity.is_error()).collect::<Vec<_>>();
    assert_eq!(failures.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["SPX-M104"]);
    assert_eq!(failures[0].message, "record pattern `Packet` is missing field `text`");
    assert_eq!(failures[0].path.as_deref(), Some("nested-owned-records-v1.spx"));
    assert!(failures[0].span.is_some());
    assert_eq!(failures[0].help, None);

    let plain = ORIGINAL_STRING_PATTERN.replace("match own packet", "match packet");
    let failures = diagnostics(&plain).into_iter().filter(|d| d.severity.is_error()).collect::<Vec<_>>();
    assert_eq!(failures.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["SPX-O111", "SPX-M103"]);
    assert_eq!(failures[0].message, "plain record match requires a Copy scrutinee");
    assert_eq!(failures[1].message, "record pattern `Packet` is incompatible with `Packet`");
    for diagnostic in failures {
        assert_eq!(diagnostic.path.as_deref(), Some("nested-owned-records-v1.spx"));
        assert!(diagnostic.span.is_some());
        assert_eq!(diagnostic.help, None);
    }
}
