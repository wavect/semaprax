//! Re-derived indexed-field provenance, including named Str aliases.
use super::*;

pub(super) fn origin(
    expression: &ResolvedExpr,
    declarations: &DeclarationIndex,
    strings: &BTreeMap<ValueId, ByteSliceProvenance>,
) -> Option<ByteSliceProvenance> {
    if let ResolvedExprKind::BorrowPlace { operation, place } = &expression.kind {
        if operation.as_str() == crate::byte_ops::STR_AS_BYTES_ID && place.projections.is_empty() {
            let mut fact = strings.get(&place.root)?.clone();
            fact.producer = Some(expression.id.clone());
            return Some(fact);
        }
    }
    let ResolvedExprKind::VecFieldRead {
        element,
        field,
        bytes,
        args,
    } = &expression.kind
    else {
        return None;
    };
    let [source, index] = args.as_slice() else {
        return None;
    };
    let ResolvedExprKind::Place(place) = &source.kind else {
        return None;
    };
    let selected = crate::hir::vec_field::field(declarations, element, field)?;
    let result = selected.result_type(*bytes)?;
    if !matches!(result, ResolvedType::Str | ResolvedType::SliceU8)
        || expression.ty != result
        || expression.ownership != OwnershipMode::Borrow
        || source.ty != crate::vec_ops::resolved_vec(element.clone())
        || !matches!(source.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        || index.ty != ResolvedType::Usize
        || index.ownership != OwnershipMode::Value
    {
        return None;
    }
    Some(ByteSliceProvenance {
        vector_field: Some(Box::new(crate::hir::VectorFieldProvenance {
            element: element.clone(),
            field: field.clone(),
            index: index.id.clone(),
        })),
        root: place.root.clone(),
        projections: place.projections.clone(),
        projected_type: selected.declaration.ty.clone(),
        root_kind: ByteSliceRootKind::OwnedVectorField,
        root_length: ByteSliceExtent::ValueLength,
        offset: ByteSliceExtent::Constant(0),
        length: ByteSliceExtent::ValueLength,
        producer: Some(expression.id.clone()),
        ranges: Vec::new(),
    })
}

pub(super) fn string_origins(
    aliases: &[(&ResolvedBinding, bool, &ResolvedExpr)],
    declarations: &DeclarationIndex,
) -> BTreeMap<ValueId, ByteSliceProvenance> {
    let mut facts = BTreeMap::new();
    loop {
        let before = facts.len();
        for (binding, mutable, value) in aliases {
            if *mutable {
                continue;
            }
            let fact = if let ResolvedExprKind::Place(place) = &value.kind {
                if place.projections.is_empty() {
                    facts.get(&place.root).cloned()
                } else {
                    None
                }
            } else {
                origin(value, declarations, &facts)
            };
            if let Some(fact) = fact {
                facts.insert(binding.id.clone(), fact);
            }
        }
        if before == facts.len() {
            return facts;
        }
    }
}
