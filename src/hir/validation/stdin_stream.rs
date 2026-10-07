//! Independent authentication of the sealed streaming reader representation.
use super::*;

impl HirValidator<'_> {
    pub(super) fn validate_borrowed_view(
        &self,
        operation: &DeclarationId,
        place: &Place,
        span: crate::ast::Span,
        scope: &BTreeMap<ValueId, ValidationBinding>,
    ) -> Result<ResolvedType, Diagnostic> {
        if operation.as_str() == crate::stdin_stream_ops::CHUNK_ID {
            let binding = scope
                .get(&place.root)
                .ok_or_else(|| hir_error_at_span(span, "streaming reader is out of scope"))?;
            if !place.projections.is_empty()
                || !crate::stdin_stream_ops::is_reader(&binding.ty)
                || !matches!(
                    binding.ownership,
                    OwnershipMode::Own | OwnershipMode::Borrow
                )
                || binding.availability != Availability::Available
            {
                return Err(hir_error_at_span(
                    span,
                    "streaming chunk requires one available unprojected reader",
                ));
            }
            return Ok(ResolvedType::SliceU8);
        }
        let op = crate::byte_ops::by_id(operation.as_str())
            .filter(|op| op.is_view())
            .ok_or_else(|| {
                hir_error_at_span(
                    span,
                    "borrowed view has an invalid compiler-owned operation",
                )
            })?;
        self.validate_byte_view_place(op, place, span, scope)?;
        Ok(op.return_type())
    }
}

pub(super) fn reject_sealed_escape(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    use crate::stdin_stream_ops::{is_reader, resolved_type_uses};
    for declaration in program.declarations.declarations() {
        if crate::stdin_stream_ops::pure_by_id(declaration.id.as_str()).is_some()
            || crate::stdin_stream_ops::pure_by_name(&declaration.name).is_some()
        {
            return Err(hir_error(
                "authored declaration aliases streaming stdin inspection",
            ));
        }
    }
    for declaration in &program.types {
        if declaration.id.as_str() == crate::stdin_stream_ops::READER_ID {
            if declaration.name != "StdinReader"
                || !declaration.type_parameters.is_empty()
                || !matches!(&declaration.kind, ResolvedTypeDeclarationKind::Record { fields } if fields.is_empty())
                || !program
                    .declarations
                    .declaration(&declaration.id)
                    .is_some_and(|d| d.identity_origin == IdentityOrigin::CompilerOwned)
            {
                return Err(hir_error(
                    "streaming reader declaration is not sealed compiler-owned metadata",
                ));
            }
            continue;
        }
        let fields: Vec<_> = match &declaration.kind {
            ResolvedTypeDeclarationKind::Record { fields }
            | ResolvedTypeDeclarationKind::Class { fields, .. } => fields.iter().collect(),
            ResolvedTypeDeclarationKind::Variant { cases } => {
                cases.iter().flat_map(|case| &case.fields).collect()
            }
            ResolvedTypeDeclarationKind::Resource { .. } => Vec::new(),
        };
        if fields.iter().any(|field| resolved_type_uses(&field.ty)) {
            return Err(hir_error(
                "streaming reader cannot be stored in an aggregate",
            ));
        }
    }
    for import in program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
    {
        if matches!(&import.result.kind, ResolvedImportResultKind::OwnedResource { resource } | ResolvedImportResultKind::OwnedResultResourceI64 { resource } | ResolvedImportResultKind::BorrowedStr { resource } if resource.as_str() == crate::stdin_stream_ops::READER_ID)
            || import.parameters.iter().any(|p| resolved_type_uses(&p.ty))
        {
            return Err(hir_error(
                "streaming reader cannot cross an authored import ABI",
            ));
        }
    }
    for template in &program.function_templates {
        if resolved_type_uses(&template.return_type)
            || template.params.iter().any(|p| resolved_type_uses(&p.ty))
        {
            return Err(hir_error(
                "streaming reader cannot appear in a generic function template",
            ));
        }
    }
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        if resolved_type_uses(&function.return_type) && !is_reader(&function.return_type) {
            return Err(hir_error(
                "streaming reader cannot appear inside a return carrier",
            ));
        }
        for param in &function.params {
            if resolved_type_uses(&param.ty)
                && (!is_reader(&param.ty)
                    || !matches!(param.ownership, OwnershipMode::Own | OwnershipMode::Borrow))
            {
                return Err(hir_error(
                    "streaming reader parameters require exact own or borrow mode",
                ));
            }
        }
        if crate::stdin_stream_ops::resolved_function_uses(function) && function.yields.is_some() {
            return Err(hir_error("streaming reader functions must be synchronous"));
        }
        if function
            .requires
            .iter()
            .chain(&function.ensures)
            .any(crate::stdin_stream_ops::resolved_expression_uses)
        {
            return Err(hir_error(
                "streaming stdin operations are not admitted in contracts",
            ));
        }
        let mut pending = vec![&function.body];
        pending.extend(&function.requires);
        pending.extend(&function.ensures);
        while let Some(expression) = pending.pop() {
            if resolved_type_uses(&expression.ty) && !is_reader(&expression.ty) {
                return Err(hir_error(
                    "streaming reader cannot appear in a generic or callable carrier",
                ));
            }
            match &expression.kind {
                ResolvedExprKind::ConstructRecord { .. } | ResolvedExprKind::ConstructVariant { .. }
                | ResolvedExprKind::UpdateRecord { .. } | ResolvedExprKind::If { .. }
                    if is_reader(&expression.ty) => return Err(hir_error("streaming reader has no construction, update, or conditional-result operation")),
                ResolvedExprKind::Match { scrutinee, .. } if is_reader(&scrutinee.ty) => return Err(hir_error("streaming reader cannot be pattern matched")),
                ResolvedExprKind::Project { base, .. } if is_reader(&base.ty) => return Err(hir_error("streaming reader cannot be projected")),
                ResolvedExprKind::Call { type_arguments, .. } if type_arguments.iter().any(resolved_type_uses) => return Err(hir_error("streaming reader cannot instantiate a generic call")),
                ResolvedExprKind::Closure { captures, body, .. } => {
                    if captures.iter().any(|capture| resolved_type_uses(&capture.value.ty)) { return Err(hir_error("streaming reader cannot be captured")); }
                    pending.push(body);
                }
                ResolvedExprKind::Yield { request } if resolved_type_uses(&request.ty) => return Err(hir_error("streaming reader cannot cross suspension")),
                _ => {}
            }
            crate::hir::push_resolved_expression_children_in_authored_order(
                expression,
                &mut pending,
            );
        }
    }
    crate::stdin_stream_ops::analysis::derive(program)?;
    Ok(())
}

pub(super) fn reopen_reader(
    scope: &mut BTreeMap<ValueId, ValidationBinding>,
    owner: &ValueId,
) -> Result<(), Diagnostic> {
    let binding = scope
        .get_mut(owner)
        .ok_or_else(|| hir_error("streaming renewal owner is absent"))?;
    if !crate::stdin_stream_ops::is_reader(&binding.ty)
        || binding.ownership != OwnershipMode::Own
        || binding.availability != Availability::Moved
        || !binding.moved_places.is_empty()
    {
        return Err(hir_error(
            "streaming renewal must restore one exactly consumed reader",
        ));
    }
    binding.availability = Availability::Available;
    Ok(())
}
