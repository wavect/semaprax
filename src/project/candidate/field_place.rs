//! Bounded nominal facts for named field places. These identify source types;
//! they never establish ownership liveness, effect admission or loan validity.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{capacity, grammar, Result, MAX_WALK_NODES};
use crate::ast::{Expr, ExprKind, MatchPattern, ModuleUseKind, Param, Program, Statement, Type};
use crate::hir::{DeclarationId, ResolvedType, ResolvedTypeDeclarationKind};
use crate::project::ProjectRevision;
use serde_json::Value;

pub(in crate::project::candidate) type NominalScope = BTreeMap<String, Arc<ResolvedType>>;

pub(super) fn requested(value: &Value) -> bool {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            Value::Object(object) => {
                if object.get("kind").and_then(Value::as_str) == Some("field_place") {
                    return true;
                }
                stack.extend(object.values());
            }
            Value::Array(values) => stack.extend(values),
            _ => {}
        }
    }
    false
}

pub(in crate::project::candidate) fn parameter_nominal_scope(
    revision: &ProjectRevision,
    program: &Program,
    parameters: &[Param],
    request: &Value,
) -> Result<NominalScope> {
    let mut scope = BTreeMap::new();
    if !requested(request) {
        return Ok(scope);
    }
    let mut work = 0;
    for parameter in parameters {
        if let Some(ty) = ast_type(revision, program, &parameter.ty, &mut work, 0)? {
            scope.insert(parameter.name.clone(), Arc::new(ty));
        }
    }
    Ok(scope)
}

fn charge(work: &mut usize, count: usize) -> Result<()> {
    *work = work.saturating_add(count);
    if *work > MAX_WALK_NODES {
        return Err(capacity(
            "field place nominal fact propagation exceeds its work bound",
        ));
    }
    Ok(())
}

pub(super) fn insert_ast_type(
    revision: &ProjectRevision,
    program: &Program,
    scope: &mut NominalScope,
    name: &str,
    ty: &Type,
) -> Result<()> {
    if let Some(ty) = ast_type(revision, program, ty, &mut 0, 0)? {
        scope.insert(name.to_owned(), Arc::new(ty));
    } else {
        scope.remove(name);
    }
    Ok(())
}

fn ast_type(
    revision: &ProjectRevision,
    program: &Program,
    ty: &Type,
    work: &mut usize,
    depth: usize,
) -> Result<Option<ResolvedType>> {
    charge(work, 1)?;
    if depth > 64 {
        return Err(capacity("field place nominal type exceeds its depth bound"));
    }
    let result = match ty {
        Type::I64 => ResolvedType::I64,
        Type::Bool => ResolvedType::Bool,
        Type::Named { name, arguments } => {
            let mut ids = BTreeSet::new();
            charge(work, program.types.len() + program.module_uses.len())?;
            for declaration in &program.types {
                if declaration.name == *name {
                    ids.insert(declaration.stable_id.as_str());
                }
            }
            for binding in &program.module_uses {
                if binding.kind == ModuleUseKind::Type && binding.alias == *name {
                    ids.insert(binding.persistent_id.as_str());
                }
            }
            if name == "Option" {
                ids.insert(crate::prelude::OPTION_ID);
            }
            if name == "Result" {
                ids.insert(crate::prelude::RESULT_ID);
            }
            if ids.len() != 1 {
                return Ok(None);
            }
            let id = ids.into_iter().next().unwrap();
            let mut values = Vec::new();
            for argument in arguments {
                let Some(value) = ast_type(revision, program, argument, work, depth + 1)? else {
                    return Ok(None);
                };
                values.push(value);
            }
            // Source bindings are already authenticated by the input Project;
            // explicit field ownership is checked separately at selection.
            ResolvedType::Nominal {
                declaration: DeclarationId::new(id),
                arguments: values,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(result))
}

/// Facts over the actual constructed AST, including compiler-inserted blocks.
/// Unknown initializers never acquire a type from a requested projection target
/// or from its generated local annotation. Branch joins require exact agreement.
pub(super) fn infer(
    revision: &ProjectRevision,
    program: &Program,
    bindings: &BTreeMap<String, String>,
    scope: &NominalScope,
    expression: &Expr,
    work: &mut usize,
    depth: usize,
) -> Result<Option<Arc<ResolvedType>>> {
    charge(work, 1)?;
    if depth > 128 {
        return Err(capacity(
            "field place nominal expression exceeds its depth bound",
        ));
    }
    let recurse = |expression: &Expr, scope: &NominalScope, work: &mut usize| {
        infer(
            revision,
            program,
            bindings,
            scope,
            expression,
            work,
            depth + 1,
        )
    };
    match &expression.kind {
        ExprKind::Var(name) => Ok(scope.get(name).cloned()),
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            ..
        }
        | ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            ..
        } => Ok(ast_type(
            revision,
            program,
            &Type::Named {
                name: type_name.clone(),
                arguments: type_arguments.clone(),
            },
            work,
            0,
        )?
        .map(Arc::new)),
        ExprKind::Call {
            name,
            type_arguments,
            ..
        } => {
            if !type_arguments.is_empty() {
                return Ok(None);
            }
            let Some(target) = bindings.get(name) else {
                return Ok(None);
            };
            let mut result = None;
            for module in revision.semantic.image_modules() {
                charge(work, module.functions().len())?;
                for function in module
                    .functions()
                    .iter()
                    .filter(|function| function.id.as_str() == target)
                {
                    if result
                        .replace(Arc::new(function.return_type.clone()))
                        .is_some()
                    {
                        return Err(grammar("field place call return identity is ambiguous"));
                    }
                }
            }
            Ok(result)
        }
        ExprKind::Project { base, field, .. } => {
            let Some(root) = recurse(base, scope, work)? else {
                return Ok(None);
            };
            field_type(revision, &root, None, field, work)
        }
        ExprKind::UpdateRecord { base, .. } => recurse(base, scope, work),
        ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            let left = recurse(then_branch, scope, work)?;
            let right = recurse(else_branch, scope, work)?;
            Ok(if left == right { left } else { None })
        }
        ExprKind::Block { statements, tail } => {
            charge(work, scope.len())?;
            let mut local = scope.clone();
            for statement in statements {
                let Statement::Let { name, value, .. } = statement else {
                    return Ok(None);
                };
                match recurse(value, &local, work)? {
                    Some(ty) => {
                        local.insert(name.clone(), ty);
                    }
                    None => {
                        local.remove(name);
                    }
                }
            }
            recurse(tail, &local, work)
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            let scrutinee_type = recurse(scrutinee, scope, work)?;
            let mut common = None;
            for (index, arm) in arms.iter().enumerate() {
                charge(work, scope.len())?;
                let mut local = scope.clone();
                match (&arm.pattern, &scrutinee_type) {
                    (
                        MatchPattern::Variant {
                            case_name, fields, ..
                        },
                        Some(ty),
                    ) => {
                        for field in fields {
                            if let Some(ty) =
                                field_type(revision, ty, Some(case_name), &field.name, work)?
                            {
                                local.insert(field.binding.clone(), ty);
                            } else {
                                local.remove(&field.binding);
                            }
                        }
                    }
                    (MatchPattern::Binding { name, .. }, Some(ty)) => {
                        local.insert(name.clone(), Arc::clone(ty));
                    }
                    (
                        MatchPattern::Wildcard { .. }
                        | MatchPattern::Literal { .. }
                        | MatchPattern::Or { .. },
                        _,
                    ) => {}
                    _ => return Ok(None),
                }
                let ty = recurse(&arm.value, &local, work)?;
                if ty.is_none() || (index > 0 && common != ty) {
                    return Ok(None);
                }
                common = ty;
            }
            Ok(common)
        }
        _ => Ok(None),
    }
}

pub(super) fn field_type(
    revision: &ProjectRevision,
    root: &ResolvedType,
    case: Option<&str>,
    name: &str,
    work: &mut usize,
) -> Result<Option<Arc<ResolvedType>>> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = root
    else {
        return Ok(None);
    };
    let mut selected = None;
    for module in revision.semantic.image_modules() {
        charge(work, module.types().len())?;
        for ty in module.types().iter().filter(|ty| ty.id == *declaration) {
            if ty.type_parameters.len() != arguments.len() {
                return Ok(None);
            }
            let fields = match (&ty.kind, case) {
                (ResolvedTypeDeclarationKind::Record { fields }, None) => fields,
                (ResolvedTypeDeclarationKind::Variant { cases }, Some(name)) => {
                    charge(work, cases.len())?;
                    let Some(case) = cases.iter().find(|case| case.name == name) else {
                        return Ok(None);
                    };
                    &case.fields
                }
                _ => return Ok(None),
            };
            charge(work, fields.len())?;
            for field in fields.iter().filter(|field| field.name == name) {
                let concrete = crate::hir::substitute_type(&field.ty, declaration, arguments)
                    .map_err(|error| vec![error])?;
                if selected.replace(Arc::new(concrete)).is_some() {
                    return Err(grammar("field place propagated field type is ambiguous"));
                }
            }
        }
    }
    Ok(selected)
}
