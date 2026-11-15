//! Borrowed Record-Field Uniqueness v1 descriptors. The operation reuses the
//! explicit sort-key layout derivation only; it never selects the consuming
//! sort opcode, renews an owner, or treats a reserved prefix as authority.
use super::sort_keys;
use crate::hir::{
    DeclarationId, DeclarationIndex, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType,
};
pub(crate) const NAME: &str = "vec_unique_by";
pub(crate) const PREFIX: &str = "core.vec.unique-by.v1:";
pub(crate) const MAX_FIELD_BYTES: usize = 256;
pub(crate) const MAX_DESCRIPTOR_BYTES: usize = sort_keys::MAX_DESCRIPTOR_BYTES;
pub(crate) const SHAPE: &str = "vec_unique_by requires an explicit monomorphic Copy or owned-leaf record, one literal String or non-floating Copy scalar field, and an available named Vec owner or borrow";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Descriptor {
    pub(crate) element: String,
    pub(crate) layout: String,
    pub(crate) field: DeclarationId,
}

/// The closed v1 key profile: exact-byte String plus integer-like Copy scalars.
/// Floating fields stay refused, so no numeric-equality contract is implied.
pub(crate) fn resolved_key(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::String
            | ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::Bool
            | ResolvedType::Char
    )
}
pub(crate) fn source_key(ty: &crate::ast::Type) -> bool {
    use crate::ast::Type;
    matches!(
        ty,
        Type::String | Type::I64 | Type::I32 | Type::U8 | Type::Usize | Type::Bool | Type::Char
    )
}

/// One ordinary ASCII identifier; the literal never becomes a runtime String.
pub(crate) fn literal(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FIELD_BYTES
        && !value.as_bytes()[0].is_ascii_digit()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
fn ascending(value: &str) -> Option<String> {
    if !literal(value) {
        return None;
    }
    // Bounded by MAX_FIELD_BYTES; a refused budget yields "" and fails closed.
    let selector = crate::bounded_output::budgeted_format(format_args!("{value}:asc"));
    (!selector.is_empty()).then_some(selector)
}

impl Descriptor {
    pub(super) fn reserve_identity(&self) -> Option<usize> {
        let length = PREFIX
            .len()
            .checked_add(sort_keys::part_len(&self.element)?)?
            .checked_add(sort_keys::part_len(&self.layout)?)?
            .checked_add(sort_keys::part_len(self.field.as_str())?)?;
        if length > MAX_DESCRIPTOR_BYTES || !crate::bounded_output::reserve_active_required(length)
        {
            return None;
        }
        Some(length)
    }

    pub(crate) fn identity(&self) -> Option<String> {
        let length = self.reserve_identity()?;
        let mut out = String::with_capacity(length);
        out.push_str(PREFIX);
        sort_keys::part(&mut out, &self.element);
        sort_keys::part(&mut out, &self.layout);
        sort_keys::part(&mut out, self.field.as_str());
        (out.len() == length).then_some(out)
    }
}

pub(crate) fn decode(identity: &str) -> Option<Descriptor> {
    if identity.len() > MAX_DESCRIPTOR_BYTES {
        return None;
    }
    let mut input = identity.strip_prefix(PREFIX)?;
    let element = sort_keys::take(&mut input)?.to_owned();
    let layout = sort_keys::take(&mut input)?.to_owned();
    let field = DeclarationId::new(sort_keys::take(&mut input)?);
    if !input.is_empty() {
        return None;
    }
    let result = Descriptor {
        element,
        layout,
        field,
    };
    // Every part was read with a canonical byte length. Keep the original
    // re-encoding reservation and sticky refusal without copying the wire.
    crate::bounded_output::reserve_active_required(identity.len()).then_some(result)
}

/// Resolved derivation from current declarations; the complete layout comes
/// from the explicit-sort derivation of the same one-field ascending selector.
pub(crate) fn resolve(
    index: &DeclarationIndex,
    element: &ResolvedType,
    value: &str,
) -> Option<Descriptor> {
    // The legacy two-Bytes carrier binds its host layout lazily; a borrowed
    // read may not bind, so that family stays outside v1.
    if crate::hir::owned_record_collection::is_admitted_owned_record_collection_element(
        index, element,
    ) {
        return None;
    }
    let selected = sort_keys::resolve(index, element, &ascending(value)?)?;
    let [key] = selected.keys.as_slice() else {
        return None;
    };
    let ResolvedType::Nominal { declaration, .. } = element else {
        return None;
    };
    let field = index
        .record_fields(declaration)?
        .iter()
        .find(|field| field.id == key.field)?;
    if !resolved_key(&field.ty) || index.declaration(&field.id)?.name != value {
        return None;
    }
    let descriptor = Descriptor {
        element: selected.element,
        layout: selected.layout,
        field: key.field.clone(),
    };
    descriptor.reserve_identity()?;
    Some(descriptor)
}

/// Independent source derivation; no resolved or generated facts enter.
pub(crate) fn source(declaration: &crate::ast::TypeDeclaration, value: &str) -> Option<Descriptor> {
    let selected = sort_keys::source(declaration, &ascending(value)?)?;
    let [key] = selected.keys.as_slice() else {
        return None;
    };
    let crate::ast::TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return None;
    };
    // Independent spelling of the legacy two-Bytes family closed above.
    if fields.len() == 3
        && fields
            .iter()
            .filter(|field| field.ty == crate::ast::Type::Bytes)
            .count()
            == 2
    {
        return None;
    }
    let field = fields
        .iter()
        .find(|field| field.stable_id == key.field.as_str())?;
    if field.name != value || !source_key(&field.ty) {
        return None;
    }
    let descriptor = Descriptor {
        element: selected.element,
        layout: selected.layout,
        field: key.field.clone(),
    };
    descriptor.reserve_identity()?;
    Some(descriptor)
}

/// Complete current replay: the stored descriptor must equal a fresh
/// derivation of its own field name over the current declarations.
pub(crate) fn replay(
    index: &DeclarationIndex,
    element: &ResolvedType,
    identity: &str,
) -> Option<Descriptor> {
    let descriptor = decode(identity)?;
    let ResolvedType::Nominal { declaration, .. } = element else {
        return None;
    };
    let field = index
        .record_fields(declaration)?
        .iter()
        .find(|field| field.id == descriptor.field)?;
    let name = &index.declaration(&field.id)?.name;
    (resolve(index, element, name)? == descriptor).then_some(descriptor)
}

/// Declaration-order position and type of the replayed field.
pub(crate) fn position(
    index: &DeclarationIndex,
    element: &ResolvedType,
    identity: &str,
) -> Option<(usize, ResolvedType)> {
    let descriptor = replay(index, element, identity)?;
    let ResolvedType::Nominal { declaration, .. } = element else {
        return None;
    };
    let fields = index.record_fields(declaration)?;
    let position = fields.iter().position(|f| f.id == descriptor.field)?;
    Some((position, fields[position].ty.clone()))
}

/// The one runtime child: an available named Vec owner or borrow, no path.
pub(crate) fn arguments(args: &[ResolvedExpr], element: &ResolvedType) -> bool {
    matches!(args, [source]
        if super::resolved_vec_matches(&source.ty, element)
        && matches!(source.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        && matches!(&source.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()))
}

pub(crate) struct Use<'a> {
    pub(crate) element: &'a ResolvedType,
    pub(crate) source: &'a ResolvedExpr,
    pub(crate) identity: &'a str,
}

/// Exact HIR shape only; callers that read storage must also `replay`.
pub(crate) fn inspect(expression: &ResolvedExpr) -> Option<Use<'_>> {
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &expression.kind
    else {
        return None;
    };
    let [element] = type_arguments.as_slice() else {
        return None;
    };
    if !callee.as_str().starts_with(PREFIX)
        || !arguments(args, element)
        || expression.ty != ResolvedType::Bool
        || expression.ownership != OwnershipMode::Value
    {
        return None;
    }
    Some(Use {
        element,
        source: &args[0],
        identity: callee.as_str(),
    })
}

/// Loan receiver: the whole named Vec root is borrowed for the call interval.
pub(crate) fn borrowed_source(call: &ResolvedExpr, position: usize) -> bool {
    position == 0 && inspect(call).is_some()
}

/// Any requested descriptor-prefixed call; selection never proves validity.
pub(crate) fn uses(program: &crate::hir::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| {
            function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(function.ensures.iter())
                .any(|root| {
                    let mut found = false;
                    crate::hir::visit_resolved_calls(root, &mut |callee, _, _| {
                        found |= callee.as_str().starts_with(PREFIX)
                    });
                    found
                })
        })
}

/// Source-level selection for the additive prelude contract.
pub(crate) fn source_uses(program: &dyn crate::ast::program_source::ProgramSource) -> bool {
    let uses = |function: &crate::ast::Function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(|root| {
                let mut found = false;
                // Reuse the exhaustive AST visitor and its existing overflow
                // route. Within its fixed-depth contour the query has no heap
                // frontier; this is presence only, never admission authority.
                root.visit_calls(&mut |name, _| found |= name == NAME);
                found
            })
    };
    program.functions().profile_iter().any(uses)
        || program.types().iter().any(|ty| {
            matches!(&ty.kind, crate::ast::TypeDeclarationKind::Class { methods, .. }
                if methods.iter().any(uses))
        })
}

/// Literal call routing shared by the iterative and reference resolvers.
pub(crate) fn literal_call(name: &str) -> bool {
    name == NAME || name == sort_keys::NAME
}

/// Test-only coherent descriptor substitution. The scratch-failure status
/// source binds the full descriptor callee, so a swapped callee is refused
/// against its stale lowered plan (`SPX-H006` status-source sequence) and is
/// independently valid HIR only after every derived ownership proof is rebuilt.
#[cfg(test)]
pub(crate) fn rederive_substituted(program: &mut crate::hir::ResolvedProgram, function: &str) {
    let stale = crate::hir::validate(program).unwrap_err();
    assert_eq!(stale.code, "SPX-H006");
    assert!(
        stale
            .message
            .contains("non-canonical status-source sequence"),
        "{stale:?}"
    );
    let index = program
        .functions
        .iter()
        .position(|candidate| candidate.id.as_str() == function)
        .unwrap();
    program.functions[index].loan_plan =
        crate::loan_plan::build_plan(&*program, &program.functions[index]).unwrap();
    program.functions[index].cleanup =
        crate::cleanup::build_inventory(&*program, &program.functions[index]).unwrap();
    program.functions[index].cleanup_plan =
        crate::cleanup_plan::build_plan(&*program, &program.functions[index]).unwrap();
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod source_probe_tests;
