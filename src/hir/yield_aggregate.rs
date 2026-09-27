//! Issue #296 R20 (docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md §12.1, blocker
//! (1) of the Agent lifecycle migration assessment): the **design record**
//! for a bounded, flat, non-recursive record or variant of Copy scalars as a
//! future `yields` request/response channel type.
//!
//! This module states the exact shape and bound a later increment would
//! admit, so that increment can check its work against a concrete target
//! instead of re-deriving it. It is deliberately **not** wired into any live
//! admission decision today: a coordinator review of this module's first
//! use found that admitting the shape in `hir::resolve_yield` alone, with no
//! matching `resumable_effects` lowering, envelope, journal, or driver
//! support, let a checked program pass HIR resolution and then fail closed
//! only much later, at attempted lowering, with the unrelated generic
//! `SPX-H006` -- worse than the stable, dedicated `SPX-T307` refusal
//! `hir::resolve_yield` now always gives a record/variant `yields` channel
//! type instead, regardless of whether it fits the shape below. Widening
//! admission again requires wiring the runtime support first (or
//! alongside), never before it.
//!
//! Depth is fixed at exactly one level: a record's own fields, or a
//! variant's own case fields, must themselves be admitted Copy scalars --
//! never another record or variant. A field can therefore never reintroduce
//! the enclosing declaration (or any other aggregate), so the shape is
//! non-recursive by construction and needs no separate cycle check, and it
//! is always fully `Copy` (`hir::DeclarationIndex::type_facts` would agree,
//! though this check does not need to consult it: a flat tuple of Copy
//! scalars needs no cleanup-plan exit path for `yield`, exactly like a bare
//! scalar). Owned `Bytes` leaves are deliberately **not** part of this
//! shape either: a record or variant field of type `Bytes` still fails this
//! check, reserved for a still-later increment on top of this one.
//! `arguments.is_empty()` refuses any generic instantiation.

use super::{DeclarationKind, ResolvedType};

pub(super) const MAX_YIELD_AGGREGATE_FIELDS: usize = 8;
pub(super) const MAX_YIELD_AGGREGATE_CASES: usize = 8;

/// `Ok(())` when `ty` is an admitted bounded aggregate; otherwise the exact
/// bound or shape rule it fails, for `resolve_yield`'s `SPX-T307` message.
/// Callers only reach this for a `ResolvedType::Nominal`; every other type
/// keeps the pre-existing non-scalar refusal.
pub(super) fn bounded_aggregate_refusal(
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
            if fields
                .iter()
                .any(|field| !super::is_scalar_resolved_type(&field.ty))
            {
                return Err(
                    "a record field is not an admitted Copy scalar; nested records, variants, \
                     and owned types (including `Bytes`) are not yet admitted inside a yield \
                     aggregate",
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
            if cases
                .iter()
                .flat_map(|case| &case.fields)
                .any(|field| !super::is_scalar_resolved_type(&field.ty))
            {
                return Err(
                    "a variant case field is not an admitted Copy scalar; nested records, \
                     variants, and owned types (including `Bytes`) are not yet admitted inside \
                     a yield aggregate",
                );
            }
            Ok(())
        }
        _ => Err("only a record or variant is an admitted bounded aggregate"),
    }
}

#[cfg(test)]
mod tests {
    use super::bounded_aggregate_refusal;
    use crate::hir;
    use std::path::Path;

    fn declarations(source: &str) -> hir::DeclarationIndex {
        let program = crate::parse(source, Path::new("yield-aggregate-fixture.spx")).unwrap();
        hir::resolve(&program)
            .expect("fixture declarations resolve")
            .declarations
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
    fn a_bytes_leaf_does_not_fit_the_designed_shape() {
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
        assert!(bounded_aggregate_refusal(&declarations, &ty).is_err());
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
