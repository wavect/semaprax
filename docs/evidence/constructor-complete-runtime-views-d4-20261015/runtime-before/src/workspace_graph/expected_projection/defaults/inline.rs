//! Same complete default evaluation with explicit internal error custody.
use super::inline_fields as field_directory;
use super::{
    checked_builder_sum, Span, StructuralCost, Type, TypeDeclaration, TypeDeclarationKind,
};
use crate::ast::{Expr, ExprKind};
use crate::diagnostic::Diagnostic;
use crate::workspace_graph::constructor_failure::{reserve, Failure};
pub(in crate::workspace_graph) fn default_expr(
    ty: &Type,
    declarations: &[(&str, &TypeDeclaration)],
    allow_owned: bool,
) -> Result<Expr, Failure> {
    reserve(std::mem::size_of::<Result<Expr, Failure>>())?;
    reserve(std::mem::size_of::<Expr>())?;
    let span = Span::default();
    let kind = match ty {
        Type::Never => {
            return Err(Failure::one(Diagnostic::io_static(
                "SPX-G172",
                "compiler-only completion marker has no default value",
            )))
        }
        ty if allow_owned
            && super::super::type_rewrite::default_vector_element(ty, declarations).is_some() =>
        {
            let element = super::super::type_rewrite::default_vector_element(ty, declarations)
                .expect("closed vector default");
            let mut cost = StructuralCost::new();
            super::super::declaration_cost::ast_type_cost(element, &mut cost)?;
            reserve(checked_builder_sum(
                cost.total,
                std::mem::size_of::<Expr>() + crate::vec_ops::WITH_CAPACITY_NAME.len(),
            )?)?;
            ExprKind::Call {
                name: crate::bounded_output::budgeted_clone(crate::vec_ops::WITH_CAPACITY_NAME),
                type_arguments: vec![element.clone()],
                args: vec![Expr {
                    kind: ExprKind::Usize(0),
                    span,
                }],
            }
        }
        ty if allow_owned && crate::map_ops::ast_collection(ty) => {
            let (name, arguments) = collection_constructor(ty);
            reserve(std::mem::size_of::<Expr>() + std::mem::size_of_val(arguments))?;
            ExprKind::Call {
                name: crate::bounded_output::budgeted_clone(name),
                type_arguments: arguments.to_vec(),
                args: vec![Expr {
                    kind: ExprKind::Usize(0),
                    span,
                }],
            }
        }
        Type::I64 => ExprKind::Int(0),
        Type::I32 => ExprKind::Int32(0),
        Type::Char => ExprKind::Char(0),
        Type::U8 => ExprKind::Uint8(0),
        Type::Usize => ExprKind::Usize(0),
        Type::F32 => ExprKind::Float32(0),
        Type::F64 => ExprKind::Float64(0),
        Type::Bool => ExprKind::Bool(false),
        Type::String => ExprKind::String(String::new()),
        Type::Str => {
            return Err(Failure::one(Diagnostic::io_static(
                "SPX-G173",
                "borrowed `str` has no synthesizable workspace default",
            )));
        }
        Type::SliceU8 | Type::StringMap => {
            return Err(Failure::one(Diagnostic::io_static(
                "SPX-G173",
                "borrowed `Slice<u8>` has no synthesizable workspace default",
            )));
        }
        Type::Bytes if allow_owned => {
            reserve(std::mem::size_of::<Expr>())?;
            ExprKind::Call {
                name: crate::bounded_output::budgeted_clone(crate::byte_ops::ZEROED_NAME),
                type_arguments: Vec::new(),
                args: vec![Expr {
                    kind: ExprKind::Usize(0),
                    span,
                }],
            }
        }
        Type::ArrayU8(_)
        | Type::Bytes
        | Type::OnceFunction
        | Type::OnceFunctionI64
        | Type::OnceFunctionI64Pair
        | Type::MutFunctionI64
        | Type::Function { .. } => {
            return Err(Failure::one(Diagnostic::io_static(
                "SPX-G173",
                "internal byte-data types have no synthesizable workspace default",
            )));
        }
        Type::Named { name, arguments } if arguments.is_empty() => {
            let declaration = declarations
                .binary_search_by_key(&name.as_str(), |(name, _)| *name)
                .map(|index| declarations[index].1)
                .map_err(|_| {
                    Failure::one(Diagnostic::io_static(
                        "SPX-G173",
                        "default imported type lookup disagrees",
                    ))
                })?;
            match &declaration.kind {
                TypeDeclarationKind::Record { fields }
                | TypeDeclarationKind::Class { fields, .. } => ExprKind::ConstructRecord {
                    type_name: crate::bounded_output::budgeted_clone(name),
                    type_span: span,
                    type_arguments: Vec::new(),
                    fields: field_directory::build(fields, declarations, allow_owned)?,
                },
                TypeDeclarationKind::Variant { cases } => {
                    let case = cases.first().ok_or_else(|| {
                        Failure::one(Diagnostic::io_static(
                            "SPX-G172",
                            "imported Copy variant has no case",
                        ))
                    })?;
                    ExprKind::ConstructVariant {
                        type_name: crate::bounded_output::budgeted_clone(name),
                        type_span: span,
                        type_arguments: Vec::new(),
                        case_name: crate::bounded_output::budgeted_clone(&case.name),
                        case_span: span,
                        fields: field_directory::build(&case.fields, declarations, allow_owned)?,
                    }
                }
                TypeDeclarationKind::Resource { .. } => {
                    return Err(Failure::one(Diagnostic::io_static(
                        "SPX-G172",
                        "resource return is not admitted",
                    )))
                }
            }
        }
        Type::Named { .. } => {
            return Err(Failure::one(Diagnostic::io_static(
                "SPX-G172",
                "generic return is not admitted",
            )))
        }
    };
    Ok(Expr { kind, span })
}

use super::collection_constructor;
