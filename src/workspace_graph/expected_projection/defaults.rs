//! Bounded non-executable import prototype defaults and their preflight costs.
use super::super::{
    active_builder_limit, graph_error, limit_error, reserve_builder_structure, resolve_type_id,
    AuthoredDeclaration,
};
use super::{checked_builder_sum, ExpandedDefaultCost, StructuralCost};
use crate::ast::{
    Expr, ExprKind, FieldInitializer, ModuleUseKind, Program, Span, Type, TypeDeclaration,
    TypeDeclarationKind,
};
use crate::diagnostic::Diagnostic;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn default_expr_expanded_cost(
    ty: &Type,
    module: &str,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    memo: &mut [BTreeMap<String, ExpandedDefaultCost>; 2],
    visiting: &mut BTreeSet<String>,
    allow_owned: bool,
) -> Result<ExpandedDefaultCost, Vec<Diagnostic>> {
    match ty {
        Type::I64
        | Type::I32
        | Type::Char
        | Type::U8
        | Type::Usize
        | Type::F32
        | Type::F64
        | Type::Bool
        | Type::String
        | Type::Str => Ok(ExpandedDefaultCost {
            bytes: std::mem::size_of::<Expr>(),
            string_bytes: 0,
            identity_slots: 0,
        }),
        Type::SliceU8 => Err(vec![graph_error(
            "SPX-G173",
            "borrowed `Slice<u8>` has no synthesizable workspace default",
        )]),
        Type::Bytes if allow_owned => Ok(ExpandedDefaultCost {
            bytes: 2 * std::mem::size_of::<Expr>() + crate::byte_ops::ZEROED_NAME.len(),
            string_bytes: crate::byte_ops::ZEROED_NAME.len(),
            identity_slots: 7,
        }),
        Type::ArrayU8(_) | Type::Bytes | Type::Function { .. } => Err(vec![graph_error(
            "SPX-G173",
            "internal byte-data types have no synthesizable workspace default",
        )]),
        Type::Named { name, arguments } if arguments.is_empty() => {
            let target_id = resolve_type_id(module, name, programs).ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "default-expression type identity cost lookup disagrees",
                )]
            })?;
            if let Some(cost) = memo[usize::from(allow_owned)].get(&target_id) {
                return Ok(*cost);
            }
            if !visiting.insert(crate::bounded_output::budgeted_clone(&target_id)) {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "default-expression type cost contains a recursive cycle",
                )]);
            }
            let target = authored.get(target_id.as_str()).ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "default-expression type authority is absent",
                )]
            })?;
            let declaration = target.ty.ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "default-expression type authority has the wrong kind",
                )]
            })?;
            let alias = caller
                .module_uses
                .iter()
                .find(|item| item.kind == ModuleUseKind::Type && item.persistent_id == target_id)
                .map(|item| item.alias.as_str())
                .ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G173",
                        "default-expression type lacks direct caller alias authority",
                    )]
                })?;
            let mut cost = StructuralCost::structure(std::mem::size_of::<Expr>());
            let mut identity_slots = 1usize;
            cost.string(alias)?;
            match &declaration.kind {
                TypeDeclarationKind::Record { fields } => {
                    for field in fields {
                        cost.add(std::mem::size_of::<FieldInitializer>())?;
                        cost.string(&field.name)?;
                        let nested = default_expr_expanded_cost(
                            &field.ty,
                            target.module,
                            caller,
                            authored,
                            programs,
                            memo,
                            visiting,
                            allow_owned,
                        )?;
                        cost.add_split(nested.bytes, nested.string_bytes)?;
                        identity_slots = checked_builder_sum(
                            identity_slots,
                            nested.identity_slots.checked_add(1).ok_or_else(|| {
                                vec![limit_error("builder_bytes", active_builder_limit())]
                            })?,
                        )?;
                    }
                }
                TypeDeclarationKind::Class { fields, .. } => {
                    for field in fields {
                        cost.add(std::mem::size_of::<FieldInitializer>())?;
                        cost.string(&field.name)?;
                        let nested = default_expr_expanded_cost(
                            &field.ty,
                            target.module,
                            caller,
                            authored,
                            programs,
                            memo,
                            visiting,
                            allow_owned,
                        )?;
                        cost.add_split(nested.bytes, nested.string_bytes)?;
                        identity_slots = checked_builder_sum(
                            identity_slots,
                            nested.identity_slots.checked_add(1).ok_or_else(|| {
                                vec![limit_error("builder_bytes", active_builder_limit())]
                            })?,
                        )?;
                    }
                }
                TypeDeclarationKind::Variant { cases } => {
                    let case = cases.first().ok_or_else(|| {
                        vec![graph_error("SPX-G172", "imported Copy variant has no case")]
                    })?;
                    cost.string(&case.name)?;
                    identity_slots = checked_builder_sum(identity_slots, 1)?;
                    for field in &case.fields {
                        cost.add(std::mem::size_of::<FieldInitializer>())?;
                        cost.string(&field.name)?;
                        let nested = default_expr_expanded_cost(
                            &field.ty,
                            target.module,
                            caller,
                            authored,
                            programs,
                            memo,
                            visiting,
                            allow_owned,
                        )?;
                        cost.add_split(nested.bytes, nested.string_bytes)?;
                        identity_slots = checked_builder_sum(
                            identity_slots,
                            nested.identity_slots.checked_add(1).ok_or_else(|| {
                                vec![limit_error("builder_bytes", active_builder_limit())]
                            })?,
                        )?;
                    }
                }
                TypeDeclarationKind::Resource { .. } => {
                    return Err(vec![graph_error(
                        "SPX-G172",
                        "resource return is not admitted",
                    )]);
                }
            }
            visiting.remove(&target_id);
            let expanded = ExpandedDefaultCost {
                bytes: cost.total,
                string_bytes: cost.string_bytes,
                identity_slots,
            };
            memo[usize::from(allow_owned)].insert(target_id, expanded);
            Ok(expanded)
        }
        Type::Named { .. } => Err(vec![graph_error(
            "SPX-G172",
            "generic return is not admitted",
        )]),
    }
}

pub(super) fn default_expr(
    ty: &Type,
    declarations: &[(&str, &TypeDeclaration)],
    allow_owned: bool,
) -> Result<Expr, Vec<Diagnostic>> {
    reserve_builder_structure(std::mem::size_of::<Expr>())?;
    let span = Span::default();
    let kind = match ty {
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
            return Err(vec![graph_error(
                "SPX-G173",
                "borrowed `str` has no synthesizable workspace default",
            )]);
        }
        Type::SliceU8 => {
            return Err(vec![graph_error(
                "SPX-G173",
                "borrowed `Slice<u8>` has no synthesizable workspace default",
            )]);
        }
        Type::Bytes if allow_owned => {
            reserve_builder_structure(std::mem::size_of::<Expr>())?;
            ExprKind::Call {
                name: crate::bounded_output::budgeted_clone(crate::byte_ops::ZEROED_NAME),
                type_arguments: Vec::new(),
                args: vec![Expr {
                    kind: ExprKind::Usize(0),
                    span,
                }],
            }
        }
        Type::ArrayU8(_) | Type::Bytes | Type::Function { .. } => {
            return Err(vec![graph_error(
                "SPX-G173",
                "internal byte-data types have no synthesizable workspace default",
            )]);
        }
        Type::Named { name, arguments } if arguments.is_empty() => {
            let declaration = declarations
                .binary_search_by_key(&name.as_str(), |(name, _)| *name)
                .map(|index| declarations[index].1)
                .map_err(|_| {
                    vec![graph_error(
                        "SPX-G173",
                        "default imported type lookup disagrees",
                    )]
                })?;
            match &declaration.kind {
                TypeDeclarationKind::Record { fields }
                | TypeDeclarationKind::Class { fields, .. } => ExprKind::ConstructRecord {
                    type_name: crate::bounded_output::budgeted_clone(name),
                    type_span: span,
                    type_arguments: Vec::new(),
                    fields: fields
                        .iter()
                        .map(|field| {
                            reserve_builder_structure(std::mem::size_of::<FieldInitializer>())?;
                            Ok(FieldInitializer {
                                name: crate::bounded_output::budgeted_clone(&field.name),
                                name_span: span,
                                value: default_expr(&field.ty, declarations, allow_owned)?,
                                span,
                            })
                        })
                        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?,
                },
                TypeDeclarationKind::Variant { cases } => {
                    let case = cases.first().ok_or_else(|| {
                        vec![graph_error("SPX-G172", "imported Copy variant has no case")]
                    })?;
                    ExprKind::ConstructVariant {
                        type_name: crate::bounded_output::budgeted_clone(name),
                        type_span: span,
                        type_arguments: Vec::new(),
                        case_name: crate::bounded_output::budgeted_clone(&case.name),
                        case_span: span,
                        fields: case
                            .fields
                            .iter()
                            .map(|field| {
                                reserve_builder_structure(std::mem::size_of::<FieldInitializer>())?;
                                Ok(FieldInitializer {
                                    name: crate::bounded_output::budgeted_clone(&field.name),
                                    name_span: span,
                                    value: default_expr(&field.ty, declarations, allow_owned)?,
                                    span,
                                })
                            })
                            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?,
                    }
                }
                TypeDeclarationKind::Resource { .. } => {
                    return Err(vec![graph_error(
                        "SPX-G172",
                        "resource return is not admitted",
                    )])
                }
            }
        }
        Type::Named { .. } => {
            return Err(vec![graph_error(
                "SPX-G172",
                "generic return is not admitted",
            )])
        }
    };
    Ok(Expr { kind, span })
}
