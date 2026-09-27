//! Issue #296 R20 (docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md §12.1, blocker
//! (1) of the Agent lifecycle migration assessment): the exact shape and
//! bound of a bounded, flat, non-recursive record or variant of Copy scalars
//! as an admitted `yields` request/response channel type.
//!
//! An earlier R20 slice kept this module a design record only, refusing
//! every record/variant channel unconditionally
//! (`hir::resolve_yield`'s `SPX-T307`), because no matching
//! `resumable_effects` lowering, envelope, journal, or driver support existed
//! yet, and a checked program must either run or be refused with a stable
//! diagnostic before it ever reaches lowering -- never "admitted here,
//! `SPX-H006` at lowering". That runtime support now exists for the direct
//! top-level (sequential) `yield` placement
//! (`resumable_effects::lowering`'s `ResumableScalar::Record`/`Variant`, the
//! `interpreter::resumable` `ResumableChannelValue` boundary, the `v5`
//! `source_checkpoint` envelope, and the durable
//! `resumable_effects::continuation` journal's `_channel` entry points), so
//! `hir::resolve_yield` now admits exactly the shape this module states, for
//! that placement only. A control-dependent placement (`yield` inside
//! `if`/`else`/`while`) keeps `SPX-T307` regardless of shape:
//! `resumable_effects::lowering::control` has no aggregate-channel support,
//! only the owned-`Bytes`-carrying Copy-scalar profile it already had.
//! `bounded_aggregate_refusal` is `pub(crate)` so both the HIR admission
//! decision and `resumable_effects`'s own independent re-derivations
//! (`resumable_effects::lowering::require_scalar_expression_tree`,
//! `resumable_effects::lowering::control::check_resumable_profile`,
//! `resumable_effects::source_signature::derive_source_effect_signature`)
//! check their work against this one authoritative shape rule rather than
//! re-deriving it.
//!
//! Depth is fixed at exactly one level: a record's own fields, or a
//! variant's own case fields, must themselves be admitted Copy scalars or
//! direct owned `Bytes` leaves --
//! never another record or variant. A field can therefore never reintroduce
//! the enclosing declaration (or any other aggregate), so the shape is
//! non-recursive by construction and needs no separate cycle check, and it
//! is fixed-depth by construction. `Bytes` leaves carry their own bounded
//! external representation and are never copied through a scalar channel.
//! `arguments.is_empty()` refuses any generic instantiation.

use super::{DeclarationKind, ResolvedExprKind, ResolvedFunction, ResolvedType};
use crate::diagnostic::Diagnostic;

pub(crate) const MAX_YIELD_AGGREGATE_FIELDS: usize = 8;
pub(crate) const MAX_YIELD_AGGREGATE_CASES: usize = 8;
pub(crate) const MAX_YIELD_AGGREGATE_BYTES_LEAVES: usize = 8;

/// Recheck the v6 one-site bound on independently supplied or relinked HIR.
/// This is structural, so it does not trust attached cleanup metadata.
pub(crate) fn check_bytes_request_site_count(
    declarations: &super::DeclarationIndex,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let Some(yields) = &function.yields else {
        return Ok(());
    };
    if !has_bytes_leaf(declarations, &yields.request_type) {
        return Ok(());
    }
    let mut sites = 0usize;
    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        if matches!(expression.kind, ResolvedExprKind::Yield { .. }) {
            sites += 1;
        }
        super::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    if sites == 1 {
        Ok(())
    } else {
        Err(Diagnostic::io(
            "SPX-T307",
            format!(
                "function `{}` has {sites} bounded `Bytes` request sites; the v6 checkpoint profile admits exactly one",
                function.name
            ),
        ))
    }
}

/// `Ok(())` when `ty` is an admitted bounded aggregate; otherwise the exact
/// bound or shape rule it fails, for `resolve_yield`'s `SPX-T307` message.
/// Callers only reach this for a `ResolvedType::Nominal`; every other type
/// keeps the pre-existing non-scalar refusal.
pub(crate) fn bounded_aggregate_refusal(
    declarations: &super::DeclarationIndex,
    ty: &ResolvedType,
) -> Result<(), &'static str> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Err("not a record or variant");
    };
    if !arguments.is_empty() {
        return Err("a generic instantiation is not an admitted bounded aggregate");
    }
    let item = declarations
        .declaration(declaration)
        .ok_or("unresolved declaration")?;
    match item.kind {
        DeclarationKind::Record => {
            let fields = declarations
                .record_fields(declaration)
                .ok_or("unresolved record fields")?;
            if fields.len() > MAX_YIELD_AGGREGATE_FIELDS {
                return Err("a record exceeds the admitted bounded-aggregate field count");
            }
            if !bounded_fields(fields.iter().map(|field| &field.ty)) {
                return Err(
                    "a record field is not an admitted scalar or direct owned `Bytes` leaf",
                );
            }
            Ok(())
        }
        DeclarationKind::Variant => {
            let cases = declarations
                .variant_cases(declaration)
                .ok_or("unresolved variant cases")?;
            if cases.len() > MAX_YIELD_AGGREGATE_CASES {
                return Err("a variant exceeds the admitted bounded-aggregate case count");
            }
            if cases
                .iter()
                .any(|case| case.fields.len() > MAX_YIELD_AGGREGATE_FIELDS)
            {
                return Err("a variant case exceeds the admitted bounded-aggregate field count");
            }
            if !bounded_fields(
                cases
                    .iter()
                    .flat_map(|case| case.fields.iter())
                    .map(|field| &field.ty),
            ) {
                return Err(
                    "a variant case field is not an admitted scalar or direct owned `Bytes` leaf",
                );
            }
            Ok(())
        }
        _ => Err("only a record or variant is an admitted bounded aggregate"),
    }
}

/// Whether an already-admitted bounded aggregate carries any direct `Bytes` leaf.
/// This is a checked schema fact, used to choose the v6 sequential envelope.
pub(crate) fn has_bytes_leaf(declarations: &super::DeclarationIndex, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    if !arguments.is_empty() {
        return false;
    }
    declarations
        .record_fields(declaration)
        .into_iter()
        .flatten()
        .any(|field| field.ty == ResolvedType::Bytes)
        || declarations
            .variant_cases(declaration)
            .into_iter()
            .flatten()
            .flat_map(|case| case.fields.iter())
            .any(|field| field.ty == ResolvedType::Bytes)
}

fn bounded_fields<'a>(mut types: impl Iterator<Item = &'a ResolvedType>) -> bool {
    let mut bytes = 0usize;
    types.all(|ty| {
        if *ty == ResolvedType::Bytes {
            bytes += 1;
            bytes <= MAX_YIELD_AGGREGATE_BYTES_LEAVES
        } else {
            super::is_scalar_resolved_type(ty)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{bounded_aggregate_refusal, check_bytes_request_site_count};
    use crate::hir;
    use std::path::Path;

    fn declarations(source: &str) -> hir::DeclarationIndex {
        let program = crate::parse(source, Path::new("yield-aggregate-fixture.spx")).unwrap();
        hir::resolve(&program)
            .expect("fixture declarations resolve")
            .declarations
    }

    #[test]
    fn relinked_hir_cannot_add_a_second_bytes_request_site() {
        let source = r#"
module test.yield_bytes_site_count;
@id("bytes.make") fn make_buf() -> Bytes { let raw = [1u8]; bytes_copy(array_as_slice(raw)) }
@id("app.prompt") record Prompt { @id("app.prompt.payload") payload: Bytes, }
@id("app.ask") fn ask() -> i64 yields Prompt -> i64 {
    let answer = yield Prompt { payload: make_buf() };
    answer
}
@id("app.main") fn main() -> i64 { 0 }
"#;
        let ast = crate::parse(source, Path::new("yield-bytes-sites.spx")).unwrap();
        let mut program = hir::resolve(&ast).unwrap();
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "app.ask")
            .unwrap();
        let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!("fixture body is a block")
        };
        let duplicated = statements[0].clone();
        statements.push(duplicated);
        assert_eq!(
            check_bytes_request_site_count(&program.declarations, function)
                .unwrap_err()
                .code,
            "SPX-T307"
        );
    }

    #[test]
    fn a_flat_record_of_copy_scalars_fits_the_designed_shape() {
        let declarations = declarations(
            r#"
module test.yield_aggregate_record;
@id("app.prompt")
record Prompt {
    @id("app.prompt.seed") seed: i64,
    @id("app.prompt.urgent") urgent: bool,
}
@id("app.main")
fn main() -> i64 { 0 }
"#,
        );
        let ty = hir::ResolvedType::Nominal {
            declaration: hir::DeclarationId::new("app.prompt"),
            arguments: Vec::new(),
        };
        assert_eq!(bounded_aggregate_refusal(&declarations, &ty), Ok(()));
    }

    #[test]
    fn a_flat_variant_of_copy_scalars_fits_the_designed_shape() {
        let declarations = declarations(
            r#"
module test.yield_aggregate_variant;
@id("app.step")
variant Step {
    @id("app.step.continue")
    Continue { @id("app.step.continue.round") round: i64, },
    @id("app.step.done")
    Done { @id("app.step.done.ok") ok: bool, },
}
@id("app.main")
fn main() -> i64 { 0 }
"#,
        );
        let ty = hir::ResolvedType::Nominal {
            declaration: hir::DeclarationId::new("app.step"),
            arguments: Vec::new(),
        };
        assert_eq!(bounded_aggregate_refusal(&declarations, &ty), Ok(()));
    }

    #[test]
    fn a_direct_bytes_leaf_fits_the_bounded_shape() {
        let declarations = declarations(
            r#"
module test.yield_aggregate_bytes_leaf;
@id("app.prompt")
record Prompt {
    @id("app.prompt.seed") seed: i64,
    @id("app.prompt.note") note: Bytes,
}
@id("app.main")
fn main() -> i64 { 0 }
"#,
        );
        let ty = hir::ResolvedType::Nominal {
            declaration: hir::DeclarationId::new("app.prompt"),
            arguments: Vec::new(),
        };
        assert_eq!(bounded_aggregate_refusal(&declarations, &ty), Ok(()));
    }

    #[test]
    fn a_generic_instantiation_does_not_fit_the_designed_shape() {
        let declarations = declarations(
            r#"
module test.yield_aggregate_generic;
@id("app.boxed")
record Boxed<T> { @id("app.boxed.value") value: T, }
@id("app.main")
fn main() -> i64 { 0 }
"#,
        );
        let ty = hir::ResolvedType::Nominal {
            declaration: hir::DeclarationId::new("app.boxed"),
            arguments: vec![hir::ResolvedType::I64],
        };
        assert!(bounded_aggregate_refusal(&declarations, &ty).is_err());
    }

    #[test]
    fn a_scalar_type_is_not_a_record_or_variant() {
        let declarations = declarations(
            r#"
module test.yield_aggregate_scalar;
@id("app.main")
fn main() -> i64 { 0 }
"#,
        );
        assert!(bounded_aggregate_refusal(&declarations, &hir::ResolvedType::I64).is_err());
    }
}
