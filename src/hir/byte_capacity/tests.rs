//! Byte-data capacity inputs derived from resolved HIR.
//!
//! These pin the storage inventory the capacity authority consumes: how many
//! bytes an aggregate slot charges, which expressions earn a slot at all, the
//! order slots are reported in, and the exact frame boundary between an
//! admitted program and `SPX-T261`.

use std::path::Path;

use crate::byte_data_capacity::{ArrayStorageKind, ArrayStorageSlot, TranscriptSource};

use super::*;

/// Nested aggregate whose inline byte payload (3 + 4) is not visible from any
/// single field, so a walk that stops at the first level under-charges it.
const NESTED: &str = r#"
module test.hir_byte_capacity_payload;

@id("data.inner")
record Inner {
    @id("data.inner.tail")
    tail: [u8; 4],
}

@id("data.outer")
record Outer {
    @id("data.outer.head")
    head: [u8; 3],
    @id("data.outer.inner")
    inner: Inner,
    @id("data.outer.count")
    count: i64,
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

fn resolved(source: &str, path: &str) -> ResolvedProgram {
    let ast = crate::parse(source, Path::new(path)).expect("fixture parses");
    crate::hir::resolve(&ast).expect("fixture resolves")
}

fn nominal(id: &str) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments: Vec::new(),
    }
}

fn shape(slots: &[ArrayStorageSlot]) -> Vec<(ArrayStorageKind, u32)> {
    slots.iter().map(|slot| (slot.kind, slot.length)).collect()
}

#[test]
fn inline_array_payload_sums_every_nested_fixed_array_field() {
    let program = resolved(NESTED, "hir-byte-capacity-payload.spx");
    assert_eq!(
        inline_array_payload_bytes(&program, &nominal("data.outer")).unwrap(),
        7,
        "a nested record charges its own arrays plus its children's"
    );
    assert_eq!(
        inline_array_payload_bytes(&program, &nominal("data.inner")).unwrap(),
        4
    );
    assert_eq!(
        inline_array_payload_bytes(&program, &ResolvedType::ArrayU8(9)).unwrap(),
        9
    );
    // Scalars and borrowed views hold no inline bytes.
    assert_eq!(
        inline_array_payload_bytes(&program, &ResolvedType::I64).unwrap(),
        0
    );
    assert_eq!(
        inline_array_payload_bytes(&program, &ResolvedType::SliceU8).unwrap(),
        0
    );
}

#[test]
fn inline_array_payload_fails_closed_on_unknown_and_unsubstituted_types() {
    let program = resolved(NESTED, "hir-byte-capacity-payload.spx");
    // Reporting 0 bytes for a slot the compiler cannot size would silently
    // under-allocate a frame, so both cases must be diagnostics.
    assert_eq!(
        inline_array_payload_bytes(&program, &nominal("data.absent"))
            .expect_err("unknown nominal type fails closed")
            .code,
        "SPX-H006"
    );
    let unsubstituted = ResolvedType::TypeParameter {
        owner: DeclarationId::new("data.outer"),
        index: 0,
    };
    assert_eq!(
        inline_array_payload_bytes(&program, &unsubstituted)
            .expect_err("unresolved type parameter fails closed")
            .code,
        "SPX-H006"
    );
}

#[test]
fn only_byte_bearing_slots_are_recorded_and_the_empty_array_still_is() {
    let program = resolved(NESTED, "hir-byte-capacity-payload.spx");
    let mut slots = Vec::new();
    push_array_slot(
        &program,
        &mut slots,
        "empty".to_owned(),
        ArrayStorageKind::Binding,
        &ResolvedType::ArrayU8(0),
    )
    .unwrap();
    // `[u8; 0]` is a real byte slot with no bytes; it must stay in the
    // inventory so identity accounting keeps seeing it.
    assert_eq!(shape(&slots), vec![(ArrayStorageKind::Binding, 0)]);

    push_array_slot(
        &program,
        &mut slots,
        "scalar".to_owned(),
        ArrayStorageKind::Binding,
        &ResolvedType::I64,
    )
    .unwrap();
    assert_eq!(slots.len(), 1, "a scalar slot is not byte storage");

    push_array_slot(
        &program,
        &mut slots,
        "outer".to_owned(),
        ArrayStorageKind::Parameter,
        &nominal("data.outer"),
    )
    .unwrap();
    assert_eq!(
        shape(&slots),
        vec![
            (ArrayStorageKind::Binding, 0),
            (ArrayStorageKind::Parameter, 7),
        ]
    );
}

#[test]
fn capacity_inputs_report_parameters_then_result_then_body_slots() {
    let source = r#"
module test.hir_byte_capacity_inventory;

@id("bytes.count")
fn count(input: [u8; 2]) -> usize
{
    let local = [1u8, 2u8, 3u8];
    byte_len(array_as_slice(local)) + byte_len(array_as_slice(input))
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;
    let program = resolved(source, "hir-byte-capacity-inventory.spx");
    let inputs = byte_data_capacity_inputs(&program).expect("capacity inputs derive");
    let count = inputs
        .iter()
        .find(|input| input.function == "bytes.count")
        .expect("function reported");
    // Parameter first, then the (zero-byte, therefore absent) provisional
    // result, then the body in authored order. A `let` bound directly to an
    // array literal is the literal's destination, so it charges one binding
    // slot rather than a binding plus a temporary.
    assert_eq!(
        shape(&count.array_slots),
        vec![
            (ArrayStorageKind::Parameter, 2),
            (ArrayStorageKind::Binding, 3),
        ]
    );
    assert!(
        count.array_slots[0].identity != count.array_slots[1].identity,
        "each slot is separately addressable"
    );
    // Inputs are keyed by stable identity, not by name, and follow resolution
    // order so the summaries line up with the functions they describe.
    assert_eq!(
        inputs
            .iter()
            .map(|input| input.function.clone())
            .collect::<Vec<_>>(),
        vec!["bytes.count".to_owned(), "app.main".to_owned()]
    );
}

#[test]
fn an_array_literal_argument_charges_both_staging_and_a_temporary() {
    let source = r#"
module test.hir_byte_capacity_staging;

@id("bytes.take")
fn take(value: [u8; 2]) -> i64
{
    0
}

@id("bytes.stage")
fn stage() -> i64
{
    take([1u8, 2u8])
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;
    let program = resolved(source, "hir-byte-capacity-staging.spx");
    let inputs = byte_data_capacity_inputs(&program).expect("capacity inputs derive");
    let stage = inputs
        .iter()
        .find(|input| input.function == "bytes.stage")
        .expect("function reported");
    // The argument is staged for the callee and also materialized as a
    // temporary, because a literal is not written into the staging slot in
    // place. Collapsing these two would under-report the caller's frame.
    assert_eq!(
        shape(&stage.array_slots),
        vec![
            (ArrayStorageKind::CallStaging, 2),
            (ArrayStorageKind::Temporary, 2),
        ]
    );
    assert!(stage.array_slots[0].identity.ends_with(".arg.0"));
}

fn frame_source(extra: &str) -> String {
    let mut source = String::new();
    source.push_str("module test.hir_byte_capacity_frame;\n\n@id(\"bytes.hold\")\n");
    source.push_str("fn hold(buffer: [u8; 65536]) -> i64\n{\n");
    source.push_str(extra);
    source.push_str("    0\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n");
    source
}

#[test]
fn a_frame_of_exactly_the_inline_array_limit_is_admitted() {
    let source = frame_source("");
    let ast = crate::parse(&source, Path::new("hir-byte-capacity-frame.spx")).expect("parses");
    let program = crate::hir::resolve(&ast).expect("a frame at the limit resolves");
    let inputs = byte_data_capacity_inputs(&program).expect("capacity inputs derive");
    let hold = inputs
        .iter()
        .find(|input| input.function == "bytes.hold")
        .expect("function reported");
    assert_eq!(
        u64::from(hold.array_slots.iter().map(|slot| slot.length).sum::<u32>()),
        crate::byte_data_capacity::MAX_INLINE_ARRAY_FRAME_BYTES,
        "the fixture sits exactly on the boundary it is testing"
    );
    assert!(analyze_byte_data_capacity(&program).is_ok());
}

#[test]
fn one_byte_past_the_inline_array_limit_is_rejected() {
    let source = frame_source("    let extra = [0u8; 1];\n");
    let ast = crate::parse(&source, Path::new("hir-byte-capacity-frame.spx")).expect("parses");
    let diagnostics = crate::hir::resolve(&ast).expect_err("one byte over is rejected");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T261"),
        "{diagnostics:#?}"
    );
}

fn stdout_argument<'a>(function: &'a ResolvedFunction) -> &'a ResolvedExpr {
    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, args, .. } = &expression.kind {
            if callee.as_str() == crate::host_io_ops::STDOUT_WRITE_ID {
                return &args[0];
            }
        }
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    panic!("fixture writes to stdout")
}

#[test]
fn transcript_source_is_fixed_for_a_known_array_root_and_unknown_otherwise() {
    let source = r#"
module test.hir_byte_capacity_transcript;

permit { process.stdout.write }

@id("bytes.fixed")
fn fixed() -> usize
    uses { process.stdout.write }
{
    let sample = [97u8, 98u8];
    let view = array_as_slice(sample);
    stdout_write(view)
}

@id("bytes.opaque")
fn opaque(view: borrow Slice<u8>) -> usize
    uses { process.stdout.write }
{
    stdout_write(view)
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;
    let program = resolved(source, "hir-byte-capacity-transcript.spx");
    let find = |id: &str| {
        program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .expect("function resolved")
    };
    // A view rooted in a local fixed array carries its exact length, so the
    // transcript budget can be charged statically.
    assert_eq!(
        byte_slice_transcript_source(&program, stdout_argument(find("bytes.fixed"))),
        TranscriptSource::Fixed(2)
    );
    // A borrowed parameter has no statically known extent, and reporting a
    // length for it would under-charge the transcript budget.
    assert_eq!(
        byte_slice_transcript_source(&program, stdout_argument(find("bytes.opaque"))),
        TranscriptSource::Unknown
    );
}

#[test]
fn call_targets_resolve_only_through_their_own_template() {
    let source = r#"
module test.hir_byte_capacity_targets;

@id("app.identity")
fn identity<T>(value: T) -> T
{
    value
}

@id("app.plain")
fn plain(value: i64) -> i64
{
    value
}

@id("app.main")
fn main() -> i64
{
    plain(identity<i64>(1))
}
"#;
    let program = resolved(source, "hir-byte-capacity-targets.spx");
    let plain = DeclarationId::new("app.plain");
    let identity = DeclarationId::new("app.identity");

    assert_eq!(
        program
            .resolve_call_target(&plain, None)
            .expect("monomorphic target resolves")
            .name,
        "plain"
    );
    // A generic template is not a callable function on its own; only one of
    // its instances is.
    assert!(program.resolve_call_target(&identity, None).is_none());

    let instance = program
        .function_instances
        .first()
        .expect("one instance discovered");
    assert_eq!(
        program
            .resolve_call_target(&identity, Some(&instance.id))
            .expect("instance target resolves")
            .return_type,
        ResolvedType::I64
    );
    // The template identity is part of the key: an instance must never be
    // reachable through an unrelated callee.
    assert!(program
        .resolve_call_target(&plain, Some(&instance.id))
        .is_none());
}
