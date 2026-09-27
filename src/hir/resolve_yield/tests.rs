use std::path::Path;

use super::{
    AGGREGATE_NOT_YET_ADMITTED, BORROW_ACROSS_YIELD, EFFECTFUL_YIELDS, ILL_TYPED_YIELD,
    MAX_YIELD_AGGREGATE_FIELDS, NON_SCALAR_BODY, NON_SCALAR_SIGNATURE, RESOURCE_ACROSS_YIELD,
};
use crate::hir;

fn resolve(source: &str) -> Result<hir::ResolvedProgram, crate::diagnostic::Diagnostic> {
    let program = crate::parse(source, Path::new("resolve-yield-fixture.spx")).unwrap();
    hir::resolve(&program).map_err(|mut errors| errors.remove(0))
}

#[test]
fn a_well_typed_scalar_yield_resolves() {
    let source = r#"
module test.resolve_yield_ok;
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let answer = yield seed + 1;
    answer * 2
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let resolved = resolve(source).expect("well-typed scalar yield resolves");
    let ask = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.ask")
        .unwrap();
    let yields = ask.yields.as_ref().expect("declares yields");
    assert_eq!(yields.request_type, hir::ResolvedType::I64);
    assert_eq!(yields.response_type, hir::ResolvedType::I64);
}

#[test]
fn sequential_yields_share_the_declared_response_type() {
    let source = r#"
module test.resolve_sequential_yields;
@id("app.ask")
fn ask() -> bool
    yields i64 -> bool
{
    let first = yield 1;
    let second = yield 2;
    first && second
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let resolved = resolve(source).expect("sequential scalar yields resolve");
    let ask = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.ask")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &ask.body.kind else {
        panic!("resumable function body is a block")
    };
    assert_eq!(statements.len(), 2);
    for statement in statements {
        let hir::ResolvedStatement::Let { value, .. } = statement else {
            panic!("fixture contains only let statements")
        };
        assert!(matches!(&value.kind, hir::ResolvedExprKind::Yield { .. }));
        assert_eq!(value.ty, hir::ResolvedType::Bool);
    }
}

#[test]
fn a_distinct_response_type_assignment_is_retagged_before_assignment_validation() {
    let source = r#"
module test.resolve_yield_assignment_response;
@id("app.ask")
fn ask(seed: i64) -> bool yields i64 -> bool {
    let mut answer = false;
    answer = yield seed;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let parsed = crate::parse(source, Path::new("resolve-yield-assignment.spx")).unwrap();
    let source_diagnostics = crate::source_verify::verify(&parsed);
    assert!(
        source_diagnostics.is_empty(),
        "the valid source must pass verification before HIR retagging: {source_diagnostics:?}"
    );
    let resolved = hir::resolve(&parsed)
        .map_err(|mut diagnostics| diagnostics.remove(0))
        .expect("a direct assignment accepts the response type");
    hir::validate(&resolved).expect("the retagged assignment remains valid HIR");
    let ask = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.ask")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &ask.body.kind else {
        panic!("resumable function body is a block")
    };
    let hir::ResolvedStatement::Assign { binding, value, .. } = &statements[1] else {
        panic!("fixture's yielded response remains a direct assignment")
    };
    assert_eq!(binding.ty, hir::ResolvedType::Bool);
    assert_eq!(value.ty, hir::ResolvedType::Bool);
    assert!(matches!(&value.kind, hir::ResolvedExprKind::Yield { .. }));
}

#[test]
fn a_direct_yield_assignment_still_requires_the_declared_response_type() {
    let source = r#"
module test.resolve_yield_assignment_response_mismatch;
@id("app.ask")
fn ask(seed: i64) -> i64 yields i64 -> bool {
    let mut answer = 0;
    answer = yield seed;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let parsed = crate::parse(source, Path::new("resolve-yield-assignment-mismatch.spx")).unwrap();
    let source_diagnostics = crate::source_verify::verify(&parsed);
    assert!(
            source_diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != "SPX-U102"),
            "the placeholder mismatch must be deferred to the response-aware HIR check: {source_diagnostics:?}"
        );
    let error = hir::resolve(&parsed)
        .unwrap_err()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(error.code, ILL_TYPED_YIELD);
    assert!(error.message.contains("yielded response of type"));
}

#[test]
fn a_distinct_response_type_can_be_the_function_tail() {
    let source = r#"
module test.resolve_yield_tail_response;
@id("app.ask")
fn ask(seed: i64) -> bool yields i64 -> bool { yield seed }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let resolved = resolve(source).expect("yield response types the function tail");
    hir::validate(&resolved).unwrap();
}

#[test]
fn a_yield_operand_of_the_wrong_type_is_refused() {
    let source = r#"
module test.resolve_yield_ill_typed;
@id("app.ask")
fn ask() -> i64
    yields i64 -> i64
{
    let answer = yield true;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let error = resolve(source).unwrap_err();
    assert_eq!(error.code, ILL_TYPED_YIELD);
}

#[test]
fn a_later_yield_operand_of_the_wrong_type_is_refused() {
    let source = r#"
module test.resolve_sequential_yield_ill_typed;
@id("app.ask")
fn ask() -> i64
    yields i64 -> i64
{
    let first = yield 1;
    let second = yield false;
    first + second
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let error = resolve(source).unwrap_err();
    assert_eq!(error.code, ILL_TYPED_YIELD);
}

#[test]
fn a_generic_function_cannot_declare_yields() {
    let source = r#"
module test.resolve_yield_generic;
@id("app.ask")
fn ask<T>() -> i64
    yields i64 -> i64
{
    let answer = yield 1;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let error = resolve(source).unwrap_err();
    // `source_verify::declared_type` refuses any generic function whose
    // body reaches a `Yield` node before `hir::resolve` runs its own
    // checks; see the module doc for why this module adds no second,
    // unreachable check for the same case.
    assert_eq!(error.code, "SPX-T226");
}

#[test]
fn a_function_with_uses_effects_cannot_also_declare_yields() {
    let source = r#"
module test.resolve_yield_effectful;
permit { clock.read }
@id("app.ask")
fn ask() -> i64
    uses { clock.read }
    yields i64 -> i64
{
    let answer = yield 1;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let error = resolve(source).unwrap_err();
    assert_eq!(error.code, EFFECTFUL_YIELDS);
}

#[test]
fn an_owned_yields_signature_that_is_not_a_record_or_variant_is_refused() {
    // `Bytes` itself is not an admitted bounded aggregate (it is not a
    // record or variant at all), so it keeps the original catch-all
    // refusal rather than the new, more specific `SPX-T307`.
    let source = r#"
module test.resolve_yield_non_scalar;
@id("app.ask")
fn ask() -> i64
    yields Bytes -> i64
{
    let answer = yield 1;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let error = resolve(source).unwrap_err();
    assert_eq!(error.code, NON_SCALAR_SIGNATURE);
}

#[test]
fn a_record_yields_signature_with_a_non_scalar_field_is_still_refused() {
    // Even a record that also fails `bounded_aggregate_refusal`'s own
    // future-admission shape (here, an owned `Bytes` leaf) is refused
    // with the same dedicated `SPX-T307` as one that would fit it: no
    // record/variant channel type is admitted today regardless.
    let source = r#"
module test.resolve_yield_non_scalar;
@id("app.prompt")
record Prompt {
    @id("app.prompt.seed") seed: i64,
    @id("app.prompt.note") note: Bytes,
}
@id("app.ask")
fn ask() -> i64
    yields Prompt -> i64
{
    let answer = yield Prompt { seed: 1, note: bytes_zeroed(1usize) };
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let error = resolve(source).unwrap_err();
    assert_eq!(error.code, AGGREGATE_NOT_YET_ADMITTED);
}

#[test]
fn a_bounded_copy_scalar_record_yields_signature_is_admitted_for_the_sequential_placement() {
    // Issue #296 R20: a flat record of Copy scalars fits
    // `yield_aggregate::bounded_aggregate_refusal`'s designed shape
    // (docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md §12.1) and is now
    // admitted as a `yields` request/response type for the direct
    // top-level (sequential) placement: `resumable_effects::lowering`,
    // the interpreter's `ResumableChannelValue` boundary, the `v5`
    // checkpoint envelope, and the durable journal's `_channel` entry
    // points all run it end to end (see
    // `crate::interpreter::resumable::channel`). A later expression that
    // uses the received answer (`answer.seed`) stays admitted too: the
    // whole-body scalar walk (`check_scalar`) now also admits any value
    // of exactly the declared request or response type, not only the
    // direct `yield` site.
    let source = r#"
module test.resolve_yield_record_aggregate;
@id("app.prompt")
record Prompt {
    @id("app.prompt.seed") seed: i64,
    @id("app.prompt.urgent") urgent: bool,
}
@id("app.ask")
fn ask() -> i64
    yields Prompt -> Prompt
{
    let answer = yield Prompt { seed: 1, urgent: true };
    answer.seed
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    resolve(source).expect("bounded record channel resolves for the sequential placement");
}

#[test]
fn a_bounded_copy_scalar_variant_yields_signature_is_admitted_for_the_sequential_placement() {
    let source = r#"
module test.resolve_yield_variant_aggregate;
@id("app.step")
variant Step {
    @id("app.step.continue")
    Continue { @id("app.step.continue.round") round: i64, },
    @id("app.step.done")
    Done { @id("app.step.done.ok") ok: bool, },
}
@id("app.ask")
fn ask() -> i64
    yields i64 -> Step
{
    let answer = yield 1;
    match answer {
        Step::Continue { round: round } => round,
        Step::Done { ok: ok } => if ok { 1 } else { 0 },
    }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    resolve(source).expect("bounded variant channel resolves for the sequential placement");
}

#[test]
fn a_bounded_aggregate_channel_inside_if_or_while_is_still_refused() {
    // Issue #296 R20: an aggregate channel is admitted only for the
    // direct top-level (sequential) placement. A `yield` reachable only
    // through `if`/`else` or `while` keeps `SPX-T307`, since
    // `resumable_effects::lowering::control` has no aggregate-channel
    // runtime support, regardless of whether the shape itself fits the
    // bound.
    let source = r#"
module test.resolve_yield_control_dependent_aggregate;
@id("app.prompt")
record Prompt {
    @id("app.prompt.seed") seed: i64,
    @id("app.prompt.urgent") urgent: bool,
}
@id("app.ask")
fn ask(seed: i64) -> i64
    yields Prompt -> Prompt
{
    if seed > 0 {
        let answer = yield Prompt { seed: seed, urgent: false };
        answer.seed
    } else {
        0
    }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let error = resolve(source).unwrap_err();
    assert_eq!(error.code, AGGREGATE_NOT_YET_ADMITTED);
}

#[test]
fn a_record_yields_signature_past_the_field_bound_is_refused() {
    let declared_fields: String = (0..=MAX_YIELD_AGGREGATE_FIELDS)
        .map(|index| format!("    @id(\"app.wide.f{index}\") f{index}: i64,\n"))
        .collect();
    let constructed_fields: String = (0..=MAX_YIELD_AGGREGATE_FIELDS)
        .map(|index| format!("f{index}: {index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
            "module test.resolve_yield_wide_record;\n@id(\"app.wide\")\nrecord Wide {{\n{declared_fields}}}\n@id(\"app.ask\")\nfn ask() -> i64 yields Wide -> i64 {{ let answer = yield Wide {{ {constructed_fields} }}; answer }}\n@id(\"app.main\")\nfn main() -> i64 {{ 0 }}\n"
        );
    let error = resolve(&source).unwrap_err();
    assert_eq!(error.code, AGGREGATE_NOT_YET_ADMITTED);
}

#[test]
fn a_generic_record_yields_signature_is_refused() {
    let source = r#"
module test.resolve_yield_generic_record;
@id("app.boxed")
record Boxed<T> { @id("app.boxed.value") value: T, }
@id("app.ask")
fn ask() -> i64
    yields Boxed<i64> -> i64
{
    let answer = yield Boxed<i64> { value: 1 };
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let error = resolve(source).unwrap_err();
    assert_eq!(error.code, AGGREGATE_NOT_YET_ADMITTED);
}

#[test]
fn a_record_nested_inside_a_record_yields_signature_is_refused() {
    let source = r#"
module test.resolve_yield_nested_record;
@id("app.inner")
record Inner { @id("app.inner.seed") seed: i64, }
@id("app.outer")
record Outer { @id("app.outer.inner") inner: Inner, }
@id("app.ask")
fn ask() -> i64
    yields Outer -> i64
{
    let answer = yield Outer { inner: Inner { seed: 1 } };
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let error = resolve(source).unwrap_err();
    assert_eq!(error.code, AGGREGATE_NOT_YET_ADMITTED);
}

#[test]
fn a_bounded_aggregate_intermediate_value_in_a_yields_function_body_is_still_refused() {
    // Issue #296 R20 widened `check_scalar` to also admit a value of
    // *exactly* the declared request or response type (see
    // `a_bounded_copy_scalar_record_yields_signature_is_admitted_for_the_sequential_placement`),
    // never any other record/variant: this function's channel is a bare
    // `i64`, so `Prompt` -- used only as unrelated body scratch, never
    // the `yield` channel -- keeps the original, unconditional
    // `SPX-T303`.
    let source = r#"
module test.resolve_yield_body_aggregate;
@id("app.prompt")
record Prompt {
    @id("app.prompt.seed") seed: i64,
    @id("app.prompt.urgent") urgent: bool,
}
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let prompt = Prompt { seed: seed, urgent: false };
    let answer = yield prompt.seed;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let error = resolve(source).unwrap_err();
    assert_eq!(error.code, NON_SCALAR_BODY);
}

#[test]
fn yields_in_branches_and_loops_resolve_with_the_response_type() {
    let source = r#"
module test.resolve_yield_control;
@id("app.ask")
fn ask(limit: i64) -> bool
    yields i64 -> bool
{
    let mut round = 0;
    let mut accepted = false;
    while round < limit {
        let ok = yield round;
        accepted = ok;
        round = round + 1;
        round > 0
    }
    let last = if accepted {
        let again = yield round;
        again
    } else {
        false
    };
    last
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = resolve(source).unwrap();
    hir::validate(&program).unwrap();
}

fn refused(parameter: &str, declarations: &str) -> String {
    let source = format!(
            "module test.resolve_yield_refusal;\n{declarations}\n@id(\"app.ask\")\nfn ask({parameter}) -> i64\n    yields i64 -> i64\n{{\n    let answer = yield 1;\n    answer\n}}\n@id(\"app.main\")\nfn main() -> i64 {{ 0 }}\n"
        );
    resolve(&source).unwrap_err().code.to_string()
}

#[test]
fn borrows_resources_and_owned_values_have_stable_refusals() {
    assert_eq!(refused("text: borrow str", ""), BORROW_ACROSS_YIELD);
    let token =
        "@id(\"app.token\")\nresource Token {\n    @id(\"app.token.drop\")\n    drop trivial;\n}";
    assert_eq!(refused("token: borrow Token", token), BORROW_ACROSS_YIELD);
    assert_eq!(refused("token: own Token", token), RESOURCE_ACROSS_YIELD);
    let owned = r#"
module test.resolve_yield_owned;
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let text = "owned";
    let answer = yield seed;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    assert_eq!(resolve(owned).unwrap_err().code, NON_SCALAR_BODY);
}

#[test]
fn a_resource_nested_inside_a_plain_record_field_is_still_the_resource_refusal() {
    // Issue #296 review: `profile_refusal` used to classify a resource
    // only when the checked type was itself directly the `resource`
    // declaration; a record that merely *contains* one (no generic
    // arguments, so `TypeFacts` computes recursively) fell through to
    // the generic `SPX-T303`. It must get the more precise `SPX-T306`,
    // the same as a bare resource parameter.
    let declarations = "@id(\"app.token\")\nresource Token {\n    @id(\"app.token.drop\")\n    drop trivial;\n}\n@id(\"app.wrapper\")\nrecord Wrapper {\n    @id(\"app.wrapper.token\")\n    token: Token,\n}";
    assert_eq!(
        refused("wrapper: own Wrapper", declarations),
        RESOURCE_ACROSS_YIELD
    );
}

#[test]
fn a_bounded_bytes_response_is_refused_at_hir_admission() {
    let source = r#"
module test.resolve_yield_bytes_response;
@id("app.answer") record Answer { @id("app.answer.payload") payload: Bytes, }
@id("app.ask") fn ask() -> i64 yields i64 -> Answer {
    let answer = yield 1;
    0
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let error = resolve(source).unwrap_err();
    assert_eq!(error.code, AGGREGATE_NOT_YET_ADMITTED);
}
