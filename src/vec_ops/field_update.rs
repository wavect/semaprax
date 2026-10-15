//! Checked scalar slot updates. Descriptors authenticate the complete current
//! nominal layout; a reserved prefix alone never grants storage authority.
use super::sort_keys;
use crate::hir::{
    DeclarationId, DeclarationIndex, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType,
};
pub(crate) const NAME: &str = "vec_update_field";
pub(crate) const PREFIX: &str = "core.vec.update-field.v1:";
pub(crate) const SHAPE: &str = "vec_update_field requires an invariant-free explicit monomorphic flat owned-leaf record, a usize index, an exact Copy scalar value, and a literal scalar field name as its last argument";
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Descriptor {
    pub(crate) element: String,
    pub(crate) layout: String,
    pub(crate) field: DeclarationId,
}
impl Descriptor {
    pub(crate) fn identity(&self) -> Option<String> {
        let length = PREFIX
            .len()
            .checked_add(sort_keys::part_len(&self.element)?)?
            .checked_add(sort_keys::part_len(&self.layout)?)?
            .checked_add(sort_keys::part_len(self.field.as_str())?)?;
        if length > sort_keys::MAX_DESCRIPTOR_BYTES
            || !crate::bounded_output::reserve_active_required(length)
        {
            return None;
        }
        let mut out = String::with_capacity(length);
        out.push_str(PREFIX);
        sort_keys::part(&mut out, &self.element);
        sort_keys::part(&mut out, &self.layout);
        sort_keys::part(&mut out, self.field.as_str());
        (out.len() == length).then_some(out)
    }
}
pub(crate) fn decode(identity: &str) -> Option<Descriptor> {
    if identity.len() > sort_keys::MAX_DESCRIPTOR_BYTES {
        return None;
    }
    let mut input = identity.strip_prefix(PREFIX)?;
    let result = Descriptor {
        element: sort_keys::take(&mut input)?.to_owned(),
        layout: sort_keys::take(&mut input)?.to_owned(),
        field: DeclarationId::new(sort_keys::take(&mut input)?),
    };
    (input.is_empty() && result.identity()?.as_str() == identity).then_some(result)
}
pub(crate) fn resolve(
    index: &DeclarationIndex,
    element: &ResolvedType,
    name: &str,
) -> Option<Descriptor> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = element
    else {
        return None;
    };
    if !arguments.is_empty()
        || index.has_record_invariant(declaration)
        || !super::field_unique::literal(name)
    {
        return None;
    }
    let fields = crate::hir::owned_leaf_collection::layout(index, element)?.fields?;
    let field = fields
        .iter()
        .find(|f| index.declaration(&f.id).is_some_and(|d| d.name == name))?;
    if !super::resolved_element_is_admitted(&field.ty) {
        return None;
    }
    let selected = sort_keys::resolve(
        index,
        element,
        &crate::bounded_output::budgeted_format(format_args!("{name}:asc")),
    )?;
    let result = Descriptor {
        element: selected.element,
        layout: selected.layout,
        field: field.id.clone(),
    };
    result.identity()?;
    Some(result)
}
/// Source-owned independent derivation; never accepts resolved layout facts.
pub(crate) fn source(declaration: &crate::ast::TypeDeclaration, name: &str) -> Option<Descriptor> {
    if !declaration.invariants().is_empty()
        || !crate::source_verify::owned_leaf_declaration_admitted(declaration)
        || !super::field_unique::literal(name)
    {
        return None;
    }
    let crate::ast::TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return None;
    };
    let field = fields.iter().find(|f| f.name == name)?;
    if !super::ast_element_is_admitted(&field.ty) {
        return None;
    }
    let selected = sort_keys::source(
        declaration,
        &crate::bounded_output::budgeted_format(format_args!("{name}:asc")),
    )?;
    let result = Descriptor {
        element: selected.element,
        layout: selected.layout,
        field: DeclarationId::new(&field.stable_id),
    };
    result.identity()?;
    Some(result)
}
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
        .find(|f| f.id == descriptor.field)?;
    (resolve(index, element, &index.declaration(&field.id)?.name)? == descriptor)
        .then_some(descriptor)
}
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
pub(crate) fn arguments(
    args: &[ResolvedExpr],
    element: &ResolvedType,
    field: &ResolvedType,
) -> bool {
    matches!(args, [source,index,value] if super::resolved_vec_matches(&source.ty,element) && source.ownership == OwnershipMode::Own && index.ty == ResolvedType::Usize && index.ownership == OwnershipMode::Value && value.ty == *field && value.ownership == OwnershipMode::Value && super::resolved_element_is_admitted(field))
}
pub(crate) struct Use<'a> {
    pub(crate) element: &'a ResolvedType,
    pub(crate) source: &'a ResolvedExpr,
    pub(crate) index: &'a ResolvedExpr,
    pub(crate) value: &'a ResolvedExpr,
    pub(crate) identity: &'a str,
}
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
    let [source, index, value] = args.as_slice() else {
        return None;
    };
    if !callee.as_str().starts_with(PREFIX)
        || !arguments(args, element, &value.ty)
        || expression.ownership != OwnershipMode::Own
        || !super::resolved_vec_matches(&expression.ty, element)
    {
        return None;
    }
    Some(Use {
        element,
        source,
        index,
        value,
        identity: callee.as_str(),
    })
}
pub(crate) fn uses(program: &crate::hir::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|f| {
            f.requires
                .iter()
                .chain(std::iter::once(&f.body))
                .chain(&f.ensures)
                .any(|root| {
                    let mut found = false;
                    crate::hir::visit_resolved_calls(root, &mut |id, _, _| {
                        found |= id.as_str().starts_with(PREFIX)
                    });
                    found
                })
        })
}
pub(crate) fn source_uses(program: &dyn crate::ast::program_source::ProgramSource) -> bool {
    let uses = |f: &crate::ast::Function| {
        f.requires
            .iter()
            .chain(std::iter::once(&f.body))
            .chain(&f.ensures)
            .any(|root| {
                let mut found = false;
                root.visit_calls(&mut |name, _| found |= name == NAME);
                found
            })
    };
    program.functions().iter().any(uses) || program.types().iter().any(|t| matches!(&t.kind,crate::ast::TypeDeclarationKind::Class {methods,..} if methods.iter().any(uses)))
}

/// Complete ordinary-call signature, including the descriptor's exact RHS type.
pub(crate) fn params(
    index: &DeclarationIndex,
    element: &ResolvedType,
    identity: &str,
) -> Option<Vec<crate::hir::ResolvedParam>> {
    let (_, field) = position(index, element, identity)?;
    let mut params = super::resolved_params_in(index, super::VecOp::UpdateField, element);
    params[2].ty = field;
    Some(params)
}

#[cfg(test)]
mod tests;
