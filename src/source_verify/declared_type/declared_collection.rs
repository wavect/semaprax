//! Independently checked logical collection field types; never runtime values.
use super::*;
use crate::ast::ExprKind;

pub(in crate::source_verify) fn text_element(types: &TypeTable<'_>, ty: &Type) -> bool {
    if *ty == Type::String {
        return true;
    }
    let Type::Named { name, arguments } = ty else {
        return false;
    };
    let Some(declaration) = types.declaration(name) else {
        return false;
    };
    if !arguments.is_empty() || !declaration.explicit_id || !declaration.type_parameters.is_empty()
    {
        return false;
    }
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return false;
    };
    (1..=8).contains(&fields.len())
        && fields.iter().filter(|f| f.ty == Type::String).count() == 1
        && fields
            .iter()
            .all(|f| f.ty == Type::String || crate::vec_ops::ast_element_is_admitted(&f.ty))
}
pub(in crate::source_verify) fn vector(types: &TypeTable<'_>, ty: &Type) -> bool {
    matches!(ty,Type::Named{name,arguments} if name=="Vec"
        && types.declaration(name).is_some_and(|d|d.stable_id==crate::prelude::VEC_ID)
        && matches!(arguments.as_slice(),[element] if text_element(types,element)))
}
fn contains(types: &TypeTable<'_>, root: &Type) -> bool {
    if !matches!(root, Type::Named { .. } | Type::Function { .. }) {
        return false;
    }
    let mut pending = vec![root.clone()];
    let mut seen = HashSet::new();
    while let Some(ty) = pending.pop() {
        if vector(types, &ty) {
            return true;
        }
        if !matches!(&ty, Type::Named { .. } | Type::Function { .. }) {
            continue;
        }
        if !seen.insert(ty.to_string()) {
            continue;
        }
        match ty {
            Type::Function { parameters, result } => {
                pending.extend(parameters);
                pending.push(*result);
            }
            Type::Named { name, arguments } => {
                if let Some(d) = types.declaration(&name) {
                    let fields: Box<dyn Iterator<Item = &FieldDeclaration>> = match &d.kind {
                        TypeDeclarationKind::Record { fields }
                        | TypeDeclarationKind::Class { fields, .. } => Box::new(fields.iter()),
                        TypeDeclarationKind::Variant { cases } => {
                            Box::new(cases.iter().flat_map(|c| &c.fields))
                        }
                        _ => Box::new(std::iter::empty()),
                    };
                    for field in fields {
                        pending.push(field.ty.clone());
                    }
                }
                pending.extend(arguments);
            }
            _ => {}
        }
    }
    false
}
pub(in crate::source_verify) fn reject_function(
    program: &Program,
    f: &Function,
    types: &TypeTable<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut rejected = contains(types, &f.return_type)
        || f.params.iter().any(|p| contains(types, &p.ty))
        || f.yields
            .as_ref()
            .is_some_and(|y| contains(types, &y.request_type) || contains(types, &y.response_type));
    for root in std::iter::once(&f.body)
        .chain(f.requires.iter())
        .chain(f.ensures.iter())
    {
        root.visit_all_nodes(&mut |e|{
            match &e.kind {
                ExprKind::ConstructRecord{type_name,type_arguments,..}|ExprKind::ConstructVariant{type_name,type_arguments,..}=>{
                    rejected|=contains(types,&Type::Named{name:type_name.clone(),arguments:type_arguments.clone()});
                }
                ExprKind::Call{type_arguments,..}|ExprKind::MethodCall{type_arguments,..}=>{rejected|=type_arguments.iter().any(|ty|contains(types,ty));}
                ExprKind::Closure{params,return_type,..}=>{rejected|=contains(types,return_type)||params.iter().any(|p|contains(types,&p.ty));}
                ExprKind::Block{statements,..}=>{
                    rejected|=statements.iter().any(|s|matches!(s,crate::ast::Statement::Let{declared:Some(ty),..} if contains(types,ty)));
                }
                _=>{}
            }
        });
    }
    if rejected {
        diagnostics.push(error(program,"SPX-T281","declaration-only text collection cannot occur in an executable signature or body",f.span)
            .with_help("derive an explicit bounded view carrier, or use an admitted runtime collection element"));
    }
}
