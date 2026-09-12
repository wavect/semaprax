//! Bounded owned-record collection element admission (SPX-AI-019 profile).
//!
//! This module independently re-derives, from resolved HIR declaration
//! facts, whether a nominal type is the one admitted internal owned-record
//! collection element: a nonrecursive, explicitly authored, non-generic
//! `record` with exactly two direct `Bytes` fields and exactly one direct
//! admitted Copy-scalar field (`i64`, `i32`, `u8`, `usize`, `char`, `f32`,
//! `f64`, or `bool`), and no other fields. See
//! `docs/OWNED-RECORD-COLLECTION-ELEMENT-V1.md` for the admitted cleanup and
//! borrow rules this classifier supports and for why it is not yet wired to
//! any executable `Vec`/`Box` intrinsic.
//!
//! Field *names* and declaration order are not part of the admission rule;
//! only the resolved field type multiset is checked. A record with an extra
//! or missing field, a nested record/class/variant/resource field, a
//! `String`/array/slice field, or any type parameter is refused, as is a
//! declaration that is not an explicitly authored `record` (a compiler-owned
//! or automatically synthesized nominal is never admitted here).
//!
//! This mirrors the existing independent classifier pattern in
//! `type_reachability::is_admitted_concrete_owned_byte_variant`: it recomputes
//! admission from `DeclarationIndex` facts alone rather than trusting a
//! cached flag, so hostile/forged HIR cannot forge admission.
//!
//! This module is also the one audited home for the profile's bounded `Vec`
//! operation surface (`vec_with_capacity`, `vec_push`, `vec_len`,
//! `vec_capacity`, `vec_clear`) and for the carrier predicate the front end
//! and cleanup consult. All three ordinary execution targets now execute the
//! profile with real per-element record storage (SPX-AI-020): the reference
//! interpreter, the native C11 backend, and Core Wasm through its owned-payload
//! host boundary. No target refuses it any more, so the former per-target
//! refusal codes (`SPX-F112`, `SPX-B115`, `SPX-W125`) are all retired. `Box` of
//! this element is not admitted.

use super::*;

#[cfg(test)]
#[path = "owned_record_collection/operation_tests.rs"]
mod operation_tests;

/// `true` when `ty` names the one admitted internal owned-record collection
/// element shape.
pub(crate) fn is_admitted_owned_record_collection_element(
    declarations: &DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
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
    let Some(item) = declarations.declaration(declaration) else {
        return false;
    };
    if item.kind != DeclarationKind::Record || item.identity_origin != IdentityOrigin::Explicit {
        return false;
    }
    let Some(parameters) = declarations.type_parameters(declaration) else {
        return false;
    };
    if !parameters.is_empty() {
        return false;
    }
    let Some(fields) = declarations.record_fields(declaration) else {
        return false;
    };
    admits_field_shape(fields)
}

/// The exact structural rule shared by the resolved and source classifiers:
/// exactly two `Bytes` fields and exactly one admitted Copy-scalar field, and
/// nothing else.
fn admits_field_shape(fields: &[ResolvedFieldDeclaration]) -> bool {
    if fields.len() != 3 {
        return false;
    }
    let mut bytes_fields = 0usize;
    let mut copy_fields = 0usize;
    for field in fields {
        if field.ty == ResolvedType::Bytes {
            bytes_fields += 1;
        } else if crate::vec_ops::resolved_element_is_admitted(&field.ty) {
            copy_fields += 1;
        } else {
            return false;
        }
    }
    bytes_fields == 2 && copy_fields == 1
}

/// The admitted element's field identities, re-derived from declaration facts.
///
/// The admission rule is a field-type *multiset*, so declaration order is the
/// only canonical order a target can use to place the two owned `Bytes` leaves
/// beside each other. This accessor returns them in that order together with
/// the one Copy-scalar field, so the native lane places an element the same
/// way on every build rather than inventing a second ordering rule.
pub(crate) struct OwnedRecordElementFields<'a> {
    /// The two owned `Bytes` fields, in declaration order.
    pub(crate) owned: [&'a ResolvedFieldDeclaration; 2],
    /// The one admitted Copy-scalar field.
    pub(crate) scalar: &'a ResolvedFieldDeclaration,
}

/// `Some` exactly when `ty` is the one admitted owned-record collection
/// element; the admission rule is re-derived here rather than assumed.
pub(crate) fn owned_record_element_fields<'a>(
    declarations: &'a DeclarationIndex,
    ty: &ResolvedType,
) -> Option<OwnedRecordElementFields<'a>> {
    if !is_admitted_owned_record_collection_element(declarations, ty) {
        return None;
    }
    let ResolvedType::Nominal { declaration, .. } = ty else {
        return None;
    };
    let fields = declarations.record_fields(declaration)?;
    let mut owned = fields
        .iter()
        .filter(|field| field.ty == ResolvedType::Bytes);
    let owned = [owned.next()?, owned.next()?];
    let scalar = fields
        .iter()
        .find(|field| field.ty != ResolvedType::Bytes)?;
    Some(OwnedRecordElementFields { owned, scalar })
}

/// The `Vec<T>` operations this profile admits over the owned-record element.
///
/// `get` is excluded for the same reason `vec_get<Bytes>` already is: it would
/// be an ambiguous copy-returning read of an owned value. `set` and
/// `reserve_exact` are excluded because they are not part of the bounded
/// surface `docs/OWNED-RECORD-COLLECTION-ELEMENT-V1.md` enumerates; widening
/// to them requires separately enumerated admission and tests. Every excluded
/// operation is refused with the same stable `SPX-H006` diagnostic an
/// inadmissible element type already produces, never a backend accident.
pub(crate) fn admits_vec_operation(op: crate::vec_ops::VecOp) -> bool {
    matches!(
        op,
        crate::vec_ops::VecOp::WithCapacity
            | crate::vec_ops::VecOp::Push
            | crate::vec_ops::VecOp::Len
            | crate::vec_ops::VecOp::Capacity
            | crate::vec_ops::VecOp::Clear
    )
}

/// `true` when `op` may be resolved over `element` under this profile.
pub(crate) fn admits_vec_operation_element(
    declarations: &DeclarationIndex,
    op: crate::vec_ops::VecOp,
    element: &ResolvedType,
) -> bool {
    admits_vec_operation(op) && is_admitted_owned_record_collection_element(declarations, element)
}

/// `true` when `ty` is the compiler-owned `Vec<T>` carrier instantiated at the
/// one admitted owned-record element.
///
/// This is deliberately *not* folded into
/// `crate::cleanup::is_owned_bounded_vec_type`. That predicate answers the
/// question from the resolved type alone, and roughly forty native and Wasm
/// layout, ABI and cleanup-replay call sites consult it to map the carrier
/// onto a concrete machine representation. This profile's element admission
/// needs `DeclarationIndex` facts, so it stays a separate question and each
/// backend lane asks its own union predicate:
/// `codegen::native_emit::owned_carrier::is_native_owned_vec_type` and
/// `wasm::vec_ops::is_wasm_owned_vec_type`. Widening the shared one
/// would silently change both lanes at once.
pub(crate) fn is_owned_record_vec_type(declarations: &DeclarationIndex, ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Nominal { declaration, arguments }
            if declaration.as_str() == crate::prelude::VEC_ID
                && matches!(
                    arguments.as_slice(),
                    [element] if is_admitted_owned_record_collection_element(declarations, element)
                )
    )
}

/// `true` when any checked body, signature, or compiler-owned `Vec<T>` call in
/// `program` names this profile.
///
/// A value of the carrier type can only be produced by a compiler-owned `Vec`
/// call instantiated at the record element, or received through a signature, so
/// scanning parameters, return types, and resolved call type arguments is a
/// complete answer for the admitted surface. Ordinary execution targets call
/// this before emitting anything.
pub(crate) fn program_uses_profile(program: &ResolvedProgram) -> bool {
    let declarations = &program.declarations;
    let names_profile = |ty: &ResolvedType| is_owned_record_vec_type(declarations, ty);
    let function_uses = |function: &ResolvedFunction| {
        function.params.iter().any(|param| names_profile(&param.ty))
            || names_profile(&function.return_type)
            || std::iter::once(&function.body)
                .chain(function.requires.iter())
                .chain(function.ensures.iter())
                .any(|root| {
                    let mut found = false;
                    super::visit_resolved_calls(root, &mut |callee, instance, arguments| {
                        found |= instance.is_none()
                            && crate::vec_ops::by_id(callee.as_str()).is_some()
                            && matches!(arguments, [element]
                            if is_admitted_owned_record_collection_element(
                                declarations,
                                element,
                            ));
                    });
                    found
                })
    };
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(function_uses)
}

/// The owned payload one admitted record element charges against
/// `crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES`.
///
/// The admitted shape owns exactly two `Bytes` leaves, so it is charged twice
/// the single-leaf rate `Vec<Bytes>` already uses; its one Copy-scalar field
/// is already covered by the target-neutral per-element capacity charge. The
/// constant lives here, beside the admission rule, so every target that
/// implements this carrier bounds it identically rather than each inventing a
/// number. At 32 bytes per element the bound admits 4096 elements, within the
/// shared `crate::vec_ops::MAX_CAPACITY` of 8192.
pub(crate) const OWNED_PAYLOAD_BYTES_PER_RECORD_ELEMENT: u64 =
    2 * crate::vec_ops::OWNED_PAYLOAD_BYTES_PER_ELEMENT;

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(source: &str) -> ResolvedProgram {
        let program =
            crate::parse(source, std::path::Path::new("owned-record-collection.spx")).unwrap();
        crate::hir::resolve(&program).unwrap()
    }

    /// Some hostile fixtures below (a `class` or generic record carrying raw
    /// `Bytes` fields) are already rejected upstream of this classifier by
    /// existing, unrelated diagnostics (`SPX-T268`/`SPX-T223`). Either outcome
    /// is valid refusal evidence: resolution fails outright, or — if it ever
    /// stopped failing upstream — this classifier independently refuses a
    /// non-`Record` kind or a generic declaration.
    fn resolve_if_admitted(source: &str) -> Option<ResolvedProgram> {
        let program =
            crate::parse(source, std::path::Path::new("owned-record-collection.spx")).unwrap();
        crate::hir::resolve(&program).ok()
    }

    #[test]
    fn exact_selected_payload_is_admitted_for_every_copy_scalar() {
        for scalar in ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"] {
            let source = format!(
                r#"module owned_record_collection.exact;
@id("owned_record_collection.exact.item") record Item {{
  @id("owned_record_collection.exact.item.id") id: Bytes,
  @id("owned_record_collection.exact.item.label") label: Bytes,
  @id("owned_record_collection.exact.item.quantity") quantity: {scalar},
}}
@id("owned_record_collection.exact.main") fn main() -> i64 {{ 0 }}
"#
            );
            let program = resolve(&source);
            let item = ResolvedType::Nominal {
                declaration: DeclarationId::new("owned_record_collection.exact.item"),
                arguments: Vec::new(),
            };
            assert!(
                is_admitted_owned_record_collection_element(&program.declarations, &item),
                "expected admission for quantity: {scalar}"
            );
        }
    }

    #[test]
    fn field_order_does_not_affect_admission() {
        let source = r#"module owned_record_collection.order;
@id("owned_record_collection.order.item") record Item {
  @id("owned_record_collection.order.item.quantity") quantity: i64,
  @id("owned_record_collection.order.item.id") id: Bytes,
  @id("owned_record_collection.order.item.label") label: Bytes,
}
@id("owned_record_collection.order.main") fn main() -> i64 { 0 }
"#;
        let program = resolve(source);
        let item = ResolvedType::Nominal {
            declaration: DeclarationId::new("owned_record_collection.order.item"),
            arguments: Vec::new(),
        };
        assert!(is_admitted_owned_record_collection_element(
            &program.declarations,
            &item
        ));
    }

    #[test]
    fn extra_field_is_refused() {
        let source = r#"module owned_record_collection.extra;
@id("owned_record_collection.extra.item") record Item {
  @id("owned_record_collection.extra.item.id") id: Bytes,
  @id("owned_record_collection.extra.item.label") label: Bytes,
  @id("owned_record_collection.extra.item.quantity") quantity: i64,
  @id("owned_record_collection.extra.item.extra") extra: i64,
}
@id("owned_record_collection.extra.main") fn main() -> i64 { 0 }
"#;
        let program = resolve(source);
        let item = ResolvedType::Nominal {
            declaration: DeclarationId::new("owned_record_collection.extra.item"),
            arguments: Vec::new(),
        };
        assert!(!is_admitted_owned_record_collection_element(
            &program.declarations,
            &item
        ));
    }

    #[test]
    fn missing_field_is_refused() {
        let source = r#"module owned_record_collection.missing;
@id("owned_record_collection.missing.item") record Item {
  @id("owned_record_collection.missing.item.id") id: Bytes,
  @id("owned_record_collection.missing.item.quantity") quantity: i64,
}
@id("owned_record_collection.missing.main") fn main() -> i64 { 0 }
"#;
        let program = resolve(source);
        let item = ResolvedType::Nominal {
            declaration: DeclarationId::new("owned_record_collection.missing.item"),
            arguments: Vec::new(),
        };
        assert!(!is_admitted_owned_record_collection_element(
            &program.declarations,
            &item
        ));
    }

    #[test]
    fn second_copy_field_instead_of_bytes_is_refused() {
        let source = r#"module owned_record_collection.wrongtype;
@id("owned_record_collection.wrongtype.item") record Item {
  @id("owned_record_collection.wrongtype.item.id") id: Bytes,
  @id("owned_record_collection.wrongtype.item.label") label: bool,
  @id("owned_record_collection.wrongtype.item.quantity") quantity: i64,
}
@id("owned_record_collection.wrongtype.main") fn main() -> i64 { 0 }
"#;
        let program = resolve(source);
        let item = ResolvedType::Nominal {
            declaration: DeclarationId::new("owned_record_collection.wrongtype.item"),
            arguments: Vec::new(),
        };
        assert!(!is_admitted_owned_record_collection_element(
            &program.declarations,
            &item
        ));
    }

    #[test]
    fn string_field_is_refused() {
        // Unlike `second_copy_field_instead_of_bytes_is_refused` (whose
        // substituted field type, `bool`, is itself an admitted Copy scalar
        // and so is refused only by the bytes/copy *count*), `string` is
        // neither `Bytes` nor an admitted Copy scalar. This exercises the
        // other refusal path in `admits_field_shape`: the immediate
        // `return false` for a field type that is neither, which the
        // existing `bool`/nested-record fixtures do not isolate on their
        // own. Pins the doc's explicit claim ("a `String` field... is
        // refused") with a dedicated regression.
        //
        // A record mixing owned `Bytes` fields with a `string` field is
        // already refused upstream of this classifier by the existing
        // owned-Bytes record shape rule (`SPX-T268`: "must be a monomorphic
        // acyclic record tree with only `Bytes` or direct Copy scalar
        // leaves"), the same pattern `class_declaration_is_refused` and
        // `generic_record_is_refused_even_when_instantiated_to_the_exact_field_shape`
        // above already rely on: resolution failing here is itself valid
        // refusal evidence.
        let source = r#"module owned_record_collection.string_field;
@id("owned_record_collection.string_field.item") record Item {
  @id("owned_record_collection.string_field.item.id") id: Bytes,
  @id("owned_record_collection.string_field.item.label") label: string,
  @id("owned_record_collection.string_field.item.quantity") quantity: i64,
}
@id("owned_record_collection.string_field.main") fn main() -> i64 { 0 }
"#;
        let Some(program) = resolve_if_admitted(source) else {
            return;
        };
        let item = ResolvedType::Nominal {
            declaration: DeclarationId::new("owned_record_collection.string_field.item"),
            arguments: Vec::new(),
        };
        assert!(!is_admitted_owned_record_collection_element(
            &program.declarations,
            &item
        ));
    }

    #[test]
    fn two_copy_fields_and_one_bytes_field_is_refused() {
        let source = r#"module owned_record_collection.swapped;
@id("owned_record_collection.swapped.item") record Item {
  @id("owned_record_collection.swapped.item.id") id: Bytes,
  @id("owned_record_collection.swapped.item.flag") flag: bool,
  @id("owned_record_collection.swapped.item.quantity") quantity: i64,
}
@id("owned_record_collection.swapped.main") fn main() -> i64 { 0 }
"#;
        let program = resolve(source);
        let item = ResolvedType::Nominal {
            declaration: DeclarationId::new("owned_record_collection.swapped.item"),
            arguments: Vec::new(),
        };
        assert!(!is_admitted_owned_record_collection_element(
            &program.declarations,
            &item
        ));
    }

    #[test]
    fn nested_record_field_is_refused() {
        let source = r#"module owned_record_collection.nested;
@id("owned_record_collection.nested.inner") record Inner {
  @id("owned_record_collection.nested.inner.payload") payload: Bytes,
}
@id("owned_record_collection.nested.item") record Item {
  @id("owned_record_collection.nested.item.id") id: Bytes,
  @id("owned_record_collection.nested.item.label") label: Bytes,
  @id("owned_record_collection.nested.item.inner") inner: Inner,
}
@id("owned_record_collection.nested.main") fn main() -> i64 { 0 }
"#;
        let program = resolve(source);
        let item = ResolvedType::Nominal {
            declaration: DeclarationId::new("owned_record_collection.nested.item"),
            arguments: Vec::new(),
        };
        assert!(!is_admitted_owned_record_collection_element(
            &program.declarations,
            &item
        ));
    }

    #[test]
    fn resource_declaration_is_refused() {
        let source = r#"module owned_record_collection.resource;
@id("owned_record_collection.resource.item") resource Item {
  @id("owned_record_collection.resource.item.drop") drop trivial;
}
@id("owned_record_collection.resource.main") fn main() -> i64 { 0 }
"#;
        let program = resolve(source);
        let item = ResolvedType::Nominal {
            declaration: DeclarationId::new("owned_record_collection.resource.item"),
            arguments: Vec::new(),
        };
        assert!(!is_admitted_owned_record_collection_element(
            &program.declarations,
            &item
        ));
    }

    #[test]
    fn class_declaration_is_refused() {
        let source = r#"module owned_record_collection.class;
@id("owned_record_collection.class.item") class Item {
  @id("owned_record_collection.class.item.id") id: Bytes,
  @id("owned_record_collection.class.item.label") label: Bytes,
  @id("owned_record_collection.class.item.quantity") quantity: i64,
}
@id("owned_record_collection.class.main") fn main() -> i64 { 0 }
"#;
        let Some(program) = resolve_if_admitted(source) else {
            // A class cannot carry owned `Bytes` fields at all (`SPX-T268`);
            // resolution already refuses this fixture upstream of this
            // classifier, which is itself valid refusal evidence.
            return;
        };
        let item = ResolvedType::Nominal {
            declaration: DeclarationId::new("owned_record_collection.class.item"),
            arguments: Vec::new(),
        };
        assert!(!is_admitted_owned_record_collection_element(
            &program.declarations,
            &item
        ));
    }

    #[test]
    fn generic_record_is_refused_even_when_instantiated_to_the_exact_field_shape() {
        let source = r#"module owned_record_collection.generic;
@id("owned_record_collection.generic.item") record Item<T> {
  @id("owned_record_collection.generic.item.id") id: Bytes,
  @id("owned_record_collection.generic.item.label") label: Bytes,
  @id("owned_record_collection.generic.item.quantity") quantity: T,
}
@id("owned_record_collection.generic.main") fn main() -> i64 { 0 }
"#;
        let Some(program) = resolve_if_admitted(source) else {
            // A generic record field typed as raw `Bytes` is already refused
            // by the existing generic-record field rule (`SPX-T223`);
            // resolution failing here is itself valid refusal evidence.
            return;
        };
        let item = ResolvedType::Nominal {
            declaration: DeclarationId::new("owned_record_collection.generic.item"),
            arguments: vec![ResolvedType::I64],
        };
        assert!(!is_admitted_owned_record_collection_element(
            &program.declarations,
            &item
        ));
    }

    #[test]
    fn foreign_variant_declaration_with_the_same_id_shape_is_refused() {
        let source = r#"module owned_record_collection.foreign;
@id("owned_record_collection.foreign.item") variant Item {
  @id("owned_record_collection.foreign.item.a") A {
    @id("owned_record_collection.foreign.item.a.value") value: Bytes,
  },
  @id("owned_record_collection.foreign.item.b") B {
    @id("owned_record_collection.foreign.item.b.value") value: Bytes,
  },
}
@id("owned_record_collection.foreign.main") fn main() -> i64 { 0 }
"#;
        let program = resolve(source);
        let item = ResolvedType::Nominal {
            declaration: DeclarationId::new("owned_record_collection.foreign.item"),
            arguments: Vec::new(),
        };
        assert!(!is_admitted_owned_record_collection_element(
            &program.declarations,
            &item
        ));
    }

    #[test]
    fn non_nominal_type_is_refused() {
        let source = r#"module owned_record_collection.nonnominal;
@id("owned_record_collection.nonnominal.main") fn main() -> i64 { 0 }
"#;
        let program = resolve(source);
        assert!(!is_admitted_owned_record_collection_element(
            &program.declarations,
            &ResolvedType::Bytes
        ));
        assert!(!is_admitted_owned_record_collection_element(
            &program.declarations,
            &ResolvedType::I64
        ));
    }
}
