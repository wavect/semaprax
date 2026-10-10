//! Source twin of the independent HIR field-path proof.
use super::*;

pub(super) fn admitted(
    place: &super::super::place::SourcePlace,
    variables: &HashMap<String, Binding>,
    types: &TypeTable<'_>,
) -> bool {
    let Some(root) = variables.get(&place.root) else {
        return false;
    };
    if place.projections.is_empty()
        || place.projections.len() > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
        || place.ty != Type::String
        || !matches!(place.mode, ParamMode::Own | ParamMode::Borrow)
        || !matches!(root.mode, ParamMode::Own | ParamMode::Borrow)
        || !super::super::declared_type::string_record::runtime_admitted(&root.ty, types)
    {
        return false;
    }
    let mut ty = &root.ty;
    for name in &place.projections {
        let Type::Named {
            name: record,
            arguments,
        } = ty
        else {
            return false;
        };
        let Some(declaration) = types.declaration(record) else {
            return false;
        };
        let crate::ast::TypeDeclarationKind::Record { fields } = &declaration.kind else {
            return false;
        };
        if !arguments.is_empty()
            || !declaration.type_parameters.is_empty()
            || !declaration.explicit_id
            || !declaration.invariants().is_empty()
        {
            return false;
        }
        let Some(field) = fields.iter().find(|f| &f.name == name) else {
            return false;
        };
        if !field.explicit_id {
            return false;
        }
        ty = &field.ty;
    }
    *ty == Type::String
}
