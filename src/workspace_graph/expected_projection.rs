//! Pure deterministic authored-program projection used to prebind builder
//! costs, reconstruct dependency depth, and independently reconstruct the
//! expected cross-file edge set.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{
    Expr, ExprKind, FieldInitializer, Function, ModuleUse, ModuleUseKind, Program, Span, Type,
    TypeDeclaration, TypeDeclarationKind,
};
use crate::diagnostic::Diagnostic;
use crate::{hir, prelude};

use super::{
    active_builder_limit, checked_usage, graph_error, limit_error, push_edge,
    reserve_builder_structure, resolve_type_id, visit_ast_call_sites, AuthoredDeclaration,
    WorkspaceEdge, HIR_FIXED_EXPANSION_FACTOR, HIR_IDENTITY_COPY_FACTOR, MAX_DEPENDENCY_DEPTH,
};

#[path = "expected_projection/cost.rs"]
mod cost;
use cost::{ExpandedDefaultCost, GenericInstanceCost, StructuralCost};

pub(super) struct SyntheticBuilderCosts {
    pub(super) raw_clone_and_hir: usize,
    pub(super) runtime: usize,
}

pub(super) fn synthetic_builder_bytes(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> Result<SyntheticBuilderCosts, Vec<Diagnostic>> {
    let mut raw = StructuralCost(0);
    ast_program_cost(program, &mut raw)?;
    let mut identity_slots = ast_program_identity_slots(program)?;
    let mut runtime = StructuralCost(0);
    let mut default_memo = BTreeMap::new();
    for module_use in &program.module_uses {
        if module_use.kind == ModuleUseKind::Protocol {
            continue;
        }
        let target = &authored[module_use.persistent_id.as_str()];
        runtime.string(&module_use.alias)?;
        if let Some(function) = target.function {
            ast_function_cost(function, &mut raw)?;
            identity_slots = checked_builder_sum(
                identity_slots,
                ast_type_identity_slots(&function.return_type)?,
            )?;
            for param in &function.params {
                identity_slots =
                    checked_builder_sum(identity_slots, ast_type_identity_slots(&param.ty)?)?;
                rewrite_type_runtime_cost(
                    &param.ty,
                    target.module,
                    program,
                    programs,
                    &mut runtime,
                )?;
            }
            rewrite_type_runtime_cost(
                &function.return_type,
                target.module,
                program,
                programs,
                &mut runtime,
            )?;
            let cost = default_expr_expanded_cost(
                &function.return_type,
                target.module,
                program,
                authored,
                programs,
                &mut default_memo,
                &mut BTreeSet::new(),
            )?;
            runtime.add(cost.bytes)?;
            identity_slots = checked_builder_sum(identity_slots, cost.identity_slots)?;
        } else {
            let declaration = target.ty.expect("validated type target");
            ast_type_declaration_cost(declaration, &mut raw)?;
            identity_slots = checked_builder_sum(
                identity_slots,
                ast_type_declaration_identity_slots(declaration)?,
            )?;
            rewrite_type_declaration_runtime_cost(
                declaration,
                target.module,
                program,
                programs,
                &mut runtime,
            )?;
        }
    }
    if !program
        .functions
        .iter()
        .any(|function| function.name == "main")
    {
        runtime.add(synthetic_main_runtime_cost(&program.module)?)?;
    }
    let generic_instances = generic_instance_source_cost(program)?;
    identity_slots = checked_builder_sum(identity_slots, generic_instances.identity_slots)?;
    let hir_input = checked_usage(raw.0, runtime.0, "builder_bytes", active_builder_limit())?;
    let hir_input = checked_usage(
        hir_input,
        generic_instances.bytes,
        "builder_bytes",
        active_builder_limit(),
    )?;
    let fixed_hir_upper = hir_input
        .checked_mul(HIR_FIXED_EXPANSION_FACTOR)
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    let maximum_identity_bytes = authored
        .keys()
        .map(|id| id.len())
        .chain(prelude::all_ids().into_iter().map(str::len))
        .max()
        .unwrap_or(0);
    let identity_occurrence_upper = identity_slots
        .checked_mul(maximum_identity_bytes)
        .and_then(|bytes| bytes.checked_mul(HIR_IDENTITY_COPY_FACTOR))
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    let hir_upper = fixed_hir_upper
        .checked_add(identity_occurrence_upper)
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    Ok(SyntheticBuilderCosts {
        raw_clone_and_hir: checked_usage(
            raw.0,
            hir_upper,
            "builder_bytes",
            active_builder_limit(),
        )?,
        runtime: runtime.0,
    })
}

fn generic_instance_source_cost(program: &Program) -> Result<GenericInstanceCost, Vec<Diagnostic>> {
    let mut templates = Vec::with_capacity(program.functions.len());
    for function in &program.functions {
        if function.type_parameters.is_empty() {
            continue;
        }
        let mut cost = StructuralCost(0);
        ast_function_cost(function, &mut cost)?;
        templates.push((
            function.name.as_str(),
            GenericInstanceCost {
                bytes: cost.0,
                identity_slots: ast_function_identity_slots(function)?,
            },
        ));
    }
    templates.sort_by(|left, right| left.0.cmp(right.0));
    let mut total = GenericInstanceCost {
        bytes: 0,
        identity_slots: 0,
    };
    for function in program
        .functions
        .iter()
        .filter(|function| function.type_parameters.is_empty())
    {
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            let mut overflowed = false;
            expression.visit_call_instances(&mut |name, arguments, _| {
                if arguments.is_empty() || overflowed {
                    return;
                }
                if let Ok(index) = templates.binary_search_by_key(&name, |(name, _)| *name) {
                    if let (Some(bytes), Some(identity_slots)) = (
                        total.bytes.checked_add(templates[index].1.bytes),
                        total
                            .identity_slots
                            .checked_add(templates[index].1.identity_slots),
                    ) {
                        total = GenericInstanceCost {
                            bytes,
                            identity_slots,
                        };
                    } else {
                        overflowed = true;
                    }
                }
            });
            if overflowed || total.bytes > active_builder_limit() {
                return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
            }
        }
    }
    Ok(total)
}

fn checked_builder_sum(left: usize, right: usize) -> Result<usize, Vec<Diagnostic>> {
    left.checked_add(right)
        .filter(|total| *total <= active_builder_limit())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])
}

fn rewrite_type_declaration_runtime_cost(
    declaration: &TypeDeclaration,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    match &declaration.kind {
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                rewrite_type_runtime_cost(&field.ty, target_module, caller, programs, cost)?;
            }
        }
        TypeDeclarationKind::Class { fields, methods } => {
            for field in fields {
                rewrite_type_runtime_cost(&field.ty, target_module, caller, programs, cost)?;
            }
            for method in methods {
                for param in &method.params {
                    rewrite_type_runtime_cost(&param.ty, target_module, caller, programs, cost)?;
                }
                rewrite_type_runtime_cost(
                    &method.return_type,
                    target_module,
                    caller,
                    programs,
                    cost,
                )?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &case.fields {
                    rewrite_type_runtime_cost(&field.ty, target_module, caller, programs, cost)?;
                }
            }
        }
        TypeDeclarationKind::Resource { .. } => unreachable!("resource imports are rejected"),
    }
    Ok(())
}

fn rewrite_type_runtime_cost(
    ty: &Type,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    let Type::Named { name, arguments } = ty else {
        return Ok(());
    };
    if !arguments.is_empty() {
        return Err(vec![graph_error(
            "SPX-G172",
            "generic cross-file types are not admitted",
        )]);
    }
    let target_id = resolve_type_id(target_module, name, programs).ok_or_else(|| {
        vec![graph_error(
            "SPX-G173",
            "cross-file type identity cost lookup disagrees",
        )]
    })?;
    let alias = caller
        .module_uses
        .iter()
        .find(|item| item.kind == ModuleUseKind::Type && item.persistent_id == target_id)
        .map(|item| item.alias.as_str())
        .ok_or_else(|| {
            vec![graph_error(
                "SPX-G172",
                crate::bounded_output::budgeted_format(format_args!(
                    "cross-file signature type `{target_id}` is not explicitly imported"
                )),
            )]
        })?;
    cost.string(alias)
}

fn synthetic_main_runtime_cost(module: &str) -> Result<usize, Vec<Diagnostic>> {
    let mut cost = StructuralCost(0);
    cost.add(std::mem::size_of::<Function>())?;
    cost.add(std::mem::size_of::<Expr>())?;
    cost.string("main")?;
    cost.add("workspace.synthetic.main.".len())?;
    cost.add(module.len())?;
    Ok(cost.0)
}

fn ast_program_cost(program: &Program, cost: &mut StructuralCost) -> Result<(), Vec<Diagnostic>> {
    cost.value(program)?;
    cost.string(&program.path)?;
    cost.string(&program.module)?;
    for module_use in &program.module_uses {
        cost.value(module_use)?;
        cost.string(&module_use.persistent_id)?;
        cost.string(&module_use.target_module)?;
        cost.string(&module_use.alias)?;
    }
    for permit in &program.permits {
        cost.string(permit)?;
    }
    for declaration in &program.types {
        ast_type_declaration_cost(declaration, cost)?;
    }
    for interface in &program.interfaces {
        cost.value(interface)?;
        cost.string(&interface.stable_id)?;
        cost.string(&interface.name)?;
        for permit in &interface.permits {
            cost.string(permit)?;
        }
        for import in &interface.imports {
            cost.value(import)?;
            cost.string(&import.stable_id)?;
            cost.string(&import.name)?;
            for param in &import.params {
                ast_param_cost(param, cost)?;
            }
            for effect in &import.effects {
                cost.string(effect)?;
            }
            if let crate::ast::ImportFailure::Status { domain_id } = &import.failure {
                cost.string(domain_id)?;
            }
            cost.string(&import.consumes)?;
        }
    }
    for function in &program.functions {
        ast_function_cost(function, cost)?;
    }
    Ok(())
}

fn ast_program_identity_slots(program: &Program) -> Result<usize, Vec<Diagnostic>> {
    let mut slots = 0usize;
    for declaration in &program.types {
        slots = checked_builder_sum(slots, ast_type_declaration_identity_slots(declaration)?)?;
    }
    for interface in &program.interfaces {
        for import in &interface.imports {
            for param in &import.params {
                slots = checked_builder_sum(slots, ast_type_identity_slots(&param.ty)?)?;
            }
        }
    }
    for function in &program.functions {
        slots = checked_builder_sum(slots, ast_function_identity_slots(function)?)?;
    }
    Ok(slots)
}

fn ast_type_declaration_identity_slots(
    declaration: &TypeDeclaration,
) -> Result<usize, Vec<Diagnostic>> {
    let mut slots = 0usize;
    match &declaration.kind {
        TypeDeclarationKind::Resource { .. } => {}
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                slots = checked_builder_sum(slots, ast_type_identity_slots(&field.ty)?)?;
            }
        }
        TypeDeclarationKind::Class { fields, methods } => {
            for field in fields {
                slots = checked_builder_sum(slots, ast_type_identity_slots(&field.ty)?)?;
            }
            for method in methods {
                slots = checked_builder_sum(slots, ast_function_identity_slots(method)?)?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &case.fields {
                    slots = checked_builder_sum(slots, ast_type_identity_slots(&field.ty)?)?;
                }
            }
        }
    }
    Ok(slots)
}

fn ast_function_identity_slots(function: &Function) -> Result<usize, Vec<Diagnostic>> {
    let mut slots = ast_type_identity_slots(&function.return_type)?;
    for param in &function.params {
        slots = checked_builder_sum(slots, ast_type_identity_slots(&param.ty)?)?;
    }
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        slots = checked_builder_sum(slots, ast_expr_identity_slots(expression)?)?;
    }
    Ok(slots)
}

fn ast_type_identity_slots(ty: &Type) -> Result<usize, Vec<Diagnostic>> {
    let Type::Named { arguments, .. } = ty else {
        return Ok(0);
    };
    let mut slots = 1usize;
    for argument in arguments {
        slots = checked_builder_sum(slots, ast_type_identity_slots(argument)?)?;
    }
    Ok(slots)
}

fn ast_expr_identity_slots(expression: &Expr) -> Result<usize, Vec<Diagnostic>> {
    // Eight covers the expression/result/callee, Try's six declaration IDs,
    // and one cleanup owner. Variable-size field/projection/pattern IDs are
    // debited separately below.
    let mut slots = 8usize;
    match &expression.kind {
        ExprKind::Call {
            type_arguments,
            args,
            ..
        } => {
            for ty in type_arguments {
                slots = checked_builder_sum(slots, ast_type_identity_slots(ty)?)?;
            }
            for argument in args {
                slots = checked_builder_sum(slots, ast_expr_identity_slots(argument)?)?;
            }
        }
        ExprKind::MethodCall {
            receiver,
            type_arguments,
            args,
            ..
        } => {
            for ty in type_arguments {
                slots = checked_builder_sum(slots, ast_type_identity_slots(ty)?)?;
            }
            slots = checked_builder_sum(slots, ast_expr_identity_slots(receiver)?)?;
            for argument in args {
                slots = checked_builder_sum(slots, ast_expr_identity_slots(argument)?)?;
            }
        }
        ExprKind::SuperMethod { args, .. } => {
            for argument in args {
                slots = checked_builder_sum(slots, ast_expr_identity_slots(argument)?)?;
            }
        }
        ExprKind::Unary { value, .. } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(value)?)?;
        }
        ExprKind::Binary { left, right, .. } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(left)?)?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(right)?)?;
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                // A `let` creates one new value identity (its binding); an
                // assignment reuses its target's existing identity.
                if matches!(statement, crate::ast::Statement::Let { .. }) {
                    slots = checked_builder_sum(slots, 1)?;
                }
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        slots = checked_builder_sum(slots, ast_expr_identity_slots(child)?)?;
                    }
                }
            }
            slots = checked_builder_sum(slots, ast_expr_identity_slots(tail)?)?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(condition)?)?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(then_branch)?)?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(else_branch)?)?;
        }
        ExprKind::ConstructRecord {
            type_arguments,
            fields,
            ..
        }
        | ExprKind::ConstructVariant {
            type_arguments,
            fields,
            ..
        } => {
            slots = checked_builder_sum(slots, fields.len())?;
            for ty in type_arguments {
                slots = checked_builder_sum(slots, ast_type_identity_slots(ty)?)?;
            }
            for field in fields {
                slots = checked_builder_sum(slots, ast_expr_identity_slots(&field.value)?)?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(scrutinee)?)?;
            for arm in arms {
                slots = checked_builder_sum(slots, ast_pattern_identity_slots(&arm.pattern)?)?;
                slots = checked_builder_sum(slots, ast_expr_identity_slots(&arm.value)?)?;
            }
        }
        ExprKind::Try { operand } => {
            slots = checked_builder_sum(slots, ast_expr_identity_slots(operand)?)?;
        }
        ExprKind::UpdateRecord { base, fields } => {
            slots = checked_builder_sum(slots, fields.len())?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(base)?)?;
            for field in fields {
                slots = checked_builder_sum(slots, ast_expr_identity_slots(&field.value)?)?;
            }
        }
        ExprKind::Project { base, .. } => {
            slots = checked_builder_sum(slots, 1)?;
            slots = checked_builder_sum(slots, ast_expr_identity_slots(base)?)?;
        }
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Var(_) => {}
    }
    Ok(slots)
}

fn ast_pattern_identity_slots(
    pattern: &crate::ast::MatchPattern,
) -> Result<usize, Vec<Diagnostic>> {
    match pattern {
        crate::ast::MatchPattern::Variant { fields, .. } => {
            let field_slots = fields
                .len()
                .checked_mul(2)
                .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
            checked_builder_sum(2, field_slots)
        }
        crate::ast::MatchPattern::Record { fields, .. } => {
            let mut slots = 1usize;
            for field in fields {
                slots = checked_builder_sum(slots, record_pattern_identity_slots(field)?)?;
            }
            Ok(slots)
        }
        crate::ast::MatchPattern::Wildcard { .. } | crate::ast::MatchPattern::Literal { .. } => {
            Ok(0)
        }
        // Refutable Match v1: a binding arm contributes one identity slot;
        // or-patterns contribute their alternatives.
        crate::ast::MatchPattern::Binding { .. } => Ok(1),
        crate::ast::MatchPattern::Or { alternatives, .. } => {
            let mut slots = 0usize;
            for alternative in alternatives {
                slots = checked_builder_sum(slots, ast_pattern_identity_slots(alternative)?)?;
            }
            Ok(slots)
        }
    }
}

fn record_pattern_identity_slots(
    field: &crate::ast::RecordMatchPatternField,
) -> Result<usize, Vec<Diagnostic>> {
    let mut slots = 1usize;
    if let crate::ast::RecordMatchFieldPattern::Record { fields, .. } = &field.pattern {
        slots = checked_builder_sum(slots, 1)?;
        for nested in fields {
            slots = checked_builder_sum(slots, record_pattern_identity_slots(nested)?)?;
        }
    }
    Ok(slots)
}

fn ast_type_declaration_cost(
    declaration: &TypeDeclaration,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(declaration)?;
    cost.string(&declaration.stable_id)?;
    cost.string(&declaration.name)?;
    for parameter in &declaration.type_parameters {
        cost.value(parameter)?;
        cost.string(&parameter.name)?;
    }
    match &declaration.kind {
        TypeDeclarationKind::Resource { lifecycles } => {
            for lifecycle in lifecycles {
                cost.value(lifecycle)?;
                if let Some(id) = &lifecycle.stable_id {
                    cost.string(id)?;
                }
                if let crate::ast::ResourceLifecycleKind::Imported { import_key } = &lifecycle.kind
                {
                    cost.string(import_key)?;
                }
            }
        }
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                ast_field_cost(field, cost)?;
            }
        }
        TypeDeclarationKind::Class { fields, methods } => {
            for field in fields {
                ast_field_cost(field, cost)?;
            }
            for method in methods {
                ast_function_cost(method, cost)?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                cost.value(case)?;
                cost.string(&case.stable_id)?;
                cost.string(&case.name)?;
                for field in &case.fields {
                    ast_field_cost(field, cost)?;
                }
            }
        }
    }
    Ok(())
}

fn ast_field_cost(
    field: &crate::ast::FieldDeclaration,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(field)?;
    cost.string(&field.stable_id)?;
    cost.string(&field.name)?;
    ast_type_cost(&field.ty, cost)
}

fn ast_function_cost(
    function: &Function,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(function)?;
    cost.string(&function.stable_id)?;
    cost.string(&function.name)?;
    for parameter in &function.type_parameters {
        cost.value(parameter)?;
        cost.string(&parameter.name)?;
    }
    for param in &function.params {
        ast_param_cost(param, cost)?;
    }
    ast_type_cost(&function.return_type, cost)?;
    for effect in &function.effects {
        cost.string(effect)?;
    }
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        ast_expr_cost(expression, cost)?;
    }
    Ok(())
}

fn ast_param_cost(
    param: &crate::ast::Param,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(param)?;
    cost.string(&param.name)?;
    ast_type_cost(&param.ty, cost)
}

fn ast_type_cost(ty: &Type, cost: &mut StructuralCost) -> Result<(), Vec<Diagnostic>> {
    cost.value(ty)?;
    if let Type::Named { name, arguments } = ty {
        cost.string(name)?;
        for argument in arguments {
            ast_type_cost(argument, cost)?;
        }
    }
    Ok(())
}

fn ast_expr_cost(expression: &Expr, cost: &mut StructuralCost) -> Result<(), Vec<Diagnostic>> {
    cost.value(expression)?;
    match &expression.kind {
        ExprKind::Var(name) => cost.string(name)?,
        ExprKind::ArrayU8(values) => cost.add(values.len())?,
        ExprKind::RepeatArrayU8 { .. } => {}
        ExprKind::Call {
            name,
            type_arguments,
            args,
        } => {
            cost.string(name)?;
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for argument in args {
                ast_expr_cost(argument, cost)?;
            }
        }
        ExprKind::Unary { value, .. } => ast_expr_cost(value, cost)?,
        ExprKind::Binary { left, right, .. } => {
            ast_expr_cost(left, cost)?;
            ast_expr_cost(right, cost)?;
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                cost.value(statement)?;
                // Only `let` and assignment statements name a binding; unsafe
                // boundaries charge their verbatim audit summary instead and
                // while statements carry no binding at all.
                match statement.audit() {
                    Some(audit) => cost.string(audit)?,
                    None if matches!(statement, crate::ast::Statement::While { .. }) => {
                        cost.string("")?
                    }
                    None => cost.string(statement.name())?,
                }
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        ast_expr_cost(child, cost)?;
                    }
                }
            }
            ast_expr_cost(tail, cost)?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            ast_expr_cost(condition, cost)?;
            ast_expr_cost(then_branch, cost)?;
            ast_expr_cost(else_branch, cost)?;
        }
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            fields,
            ..
        }
        | ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            fields,
            ..
        } => {
            cost.string(type_name)?;
            if let ExprKind::ConstructVariant { case_name, .. } = &expression.kind {
                cost.string(case_name)?;
            }
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for field in fields {
                cost.value(field)?;
                cost.string(&field.name)?;
                ast_expr_cost(&field.value, cost)?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            ast_expr_cost(scrutinee, cost)?;
            for arm in arms {
                cost.value(arm)?;
                ast_pattern_cost(&arm.pattern, cost)?;
                ast_expr_cost(&arm.value, cost)?;
            }
        }
        ExprKind::Try { operand } => ast_expr_cost(operand, cost)?,
        ExprKind::UpdateRecord { base, fields } => {
            ast_expr_cost(base, cost)?;
            for field in fields {
                cost.value(field)?;
                cost.string(&field.name)?;
                ast_expr_cost(&field.value, cost)?;
            }
        }
        ExprKind::Project { base, field, .. } => {
            ast_expr_cost(base, cost)?;
            cost.string(field)?;
        }
        ExprKind::MethodCall {
            receiver,
            method,
            type_arguments,
            args,
            ..
        } => {
            ast_expr_cost(receiver, cost)?;
            cost.string(method)?;
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for argument in args {
                ast_expr_cost(argument, cost)?;
            }
        }
        ExprKind::SuperMethod { method, args, .. } => {
            cost.string(method)?;
            for argument in args {
                ast_expr_cost(argument, cost)?;
            }
        }
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_) => {}
    }
    Ok(())
}

fn ast_pattern_cost(
    pattern: &crate::ast::MatchPattern,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(pattern)?;
    match pattern {
        crate::ast::MatchPattern::Variant {
            type_name,
            case_name,
            fields,
            ..
        } => {
            cost.string(type_name)?;
            cost.string(case_name)?;
            for field in fields {
                cost.value(field)?;
                cost.string(&field.name)?;
                cost.string(&field.binding)?;
            }
        }
        crate::ast::MatchPattern::Record {
            type_name, fields, ..
        } => {
            cost.string(type_name)?;
            for field in fields {
                ast_record_pattern_field_cost(field, cost)?;
            }
        }
        crate::ast::MatchPattern::Wildcard { .. } => {}
        // Refutable Match v1: literal/or/binding structural costs.
        crate::ast::MatchPattern::Literal { value, .. } => {
            cost.string(value.type_text())?;
        }
        crate::ast::MatchPattern::Or { alternatives, .. } => {
            for alternative in alternatives {
                ast_pattern_cost(alternative, cost)?;
            }
        }
        crate::ast::MatchPattern::Binding { name, .. } => {
            cost.string(name)?;
        }
    }
    Ok(())
}

fn ast_record_pattern_field_cost(
    field: &crate::ast::RecordMatchPatternField,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(field)?;
    cost.string(&field.name)?;
    cost.value(&field.pattern)?;
    match &field.pattern {
        crate::ast::RecordMatchFieldPattern::Binding { name, .. } => cost.string(name)?,
        crate::ast::RecordMatchFieldPattern::Record {
            type_name, fields, ..
        } => {
            cost.string(type_name)?;
            for field in fields {
                ast_record_pattern_field_cost(field, cost)?;
            }
        }
        crate::ast::RecordMatchFieldPattern::Wildcard { .. } => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn default_expr_expanded_cost(
    ty: &Type,
    module: &str,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    memo: &mut BTreeMap<String, ExpandedDefaultCost>,
    visiting: &mut BTreeSet<String>,
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
            identity_slots: 0,
        }),
        Type::SliceU8 => Err(vec![graph_error(
            "SPX-G173",
            "borrowed `Slice<u8>` has no synthesizable workspace default",
        )]),
        Type::ArrayU8(_) | Type::Bytes => Err(vec![graph_error(
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
            if let Some(cost) = memo.get(&target_id) {
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
            let mut cost = StructuralCost(std::mem::size_of::<Expr>());
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
                        )?;
                        cost.add(nested.bytes)?;
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
                        )?;
                        cost.add(nested.bytes)?;
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
                        )?;
                        cost.add(nested.bytes)?;
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
                bytes: cost.0,
                identity_slots,
            };
            memo.insert(target_id, expanded);
            Ok(expanded)
        }
        Type::Named { .. } => Err(vec![graph_error(
            "SPX-G172",
            "generic return is not admitted",
        )]),
    }
}

pub(super) fn validate_dependency_dag(
    programs: &[Program],
) -> Result<BTreeMap<&str, usize>, Vec<Diagnostic>> {
    let dependencies = programs
        .iter()
        .map(|program| {
            (
                program.module.as_str(),
                program
                    .module_uses
                    .iter()
                    .map(|item| item.target_module.as_str())
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let depths = dependency_depths(&dependencies)?;
    if depths.values().any(|depth| *depth > MAX_DEPENDENCY_DEPTH) {
        return Err(vec![limit_error("dependency_depth", MAX_DEPENDENCY_DEPTH)]);
    }
    Ok(depths)
}

pub(super) fn dependency_depths<'a>(
    dependencies: &BTreeMap<&'a str, BTreeSet<&'a str>>,
) -> Result<BTreeMap<&'a str, usize>, Vec<Diagnostic>> {
    fn visit<'a>(
        module: &'a str,
        dependencies: &BTreeMap<&'a str, BTreeSet<&'a str>>,
        stack: &mut Vec<&'a str>,
        depths: &mut BTreeMap<&'a str, usize>,
    ) -> Result<usize, Vec<Diagnostic>> {
        if let Some(index) = stack.iter().position(|item| *item == module) {
            let mut cycle = stack[index..].to_vec();
            cycle.push(module);
            let witness = canonical_cycle(&cycle);
            let witness = crate::bounded_output::budgeted_join(
                witness
                    .into_iter()
                    .map(crate::bounded_output::budgeted_clone),
                " -> ",
            );
            return Err(vec![graph_error(
                "SPX-G172",
                crate::bounded_output::budgeted_format(format_args!(
                    "workspace module dependency cycle: {witness}"
                )),
            )]);
        }
        if let Some(depth) = depths.get(module) {
            return Ok(*depth);
        }
        stack.push(module);
        let mut depth = 1usize;
        for dependency in dependencies.get(module).into_iter().flatten() {
            depth = depth.max(
                1usize
                    .checked_add(visit(dependency, dependencies, stack, depths)?)
                    .ok_or_else(|| vec![limit_error("dependency_depth", MAX_DEPENDENCY_DEPTH)])?,
            );
        }
        stack.pop();
        depths.insert(module, depth);
        Ok(depth)
    }
    let mut depths = BTreeMap::new();
    for module in dependencies.keys() {
        visit(module, dependencies, &mut Vec::new(), &mut depths)?;
    }
    Ok(depths)
}

fn canonical_cycle<'a>(cycle: &[&'a str]) -> Vec<&'a str> {
    let body = &cycle[..cycle.len().saturating_sub(1)];
    let start = body
        .iter()
        .enumerate()
        .min_by_key(|(_, module)| *module)
        .map_or(0, |(index, _)| index);
    let mut result = body[start..]
        .iter()
        .chain(&body[..start])
        .copied()
        .collect::<Vec<_>>();
    if let Some(first) = result.first().copied() {
        result.push(first);
    }
    result
}

pub(super) fn synthetic_program(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> Result<Program, Vec<Diagnostic>> {
    let mut synthetic = program.clone();
    synthetic.module_uses.clear();
    // Project validation already authenticated static implementation sidecars.
    // Runtime HIR has no witness or dispatch representation for them.
    synthetic.implementations.clear();
    for module_use in program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Type)
    {
        let target = &authored[module_use.persistent_id.as_str()];
        let mut ty = target.ty.expect("validated type target").clone();
        ty.name = crate::bounded_output::budgeted_clone(&module_use.alias);
        rewrite_type_declaration(&mut ty, target.module, program, programs)?;
        synthetic.types.push(ty);
    }
    let type_index_bytes = synthetic
        .types
        .len()
        .checked_mul(std::mem::size_of::<(&str, &TypeDeclaration)>())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    reserve_builder_structure(type_index_bytes)?;
    let mut type_declarations = Vec::with_capacity(synthetic.types.len());
    for declaration in &synthetic.types {
        type_declarations.push((declaration.name.as_str(), declaration));
    }
    type_declarations.sort_by(|left, right| left.0.cmp(right.0));
    if type_declarations
        .windows(2)
        .any(|items| items[0].0 == items[1].0)
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "synthetic workspace type-name index is not unique",
        )]);
    }
    for module_use in program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Function)
    {
        let target = &authored[module_use.persistent_id.as_str()];
        let target_function = target.function.expect("validated function target");
        let mut function = target_function.clone();
        function.name = crate::bounded_output::budgeted_clone(&module_use.alias);
        for param in &mut function.params {
            rewrite_type(&mut param.ty, target.module, program, programs)?;
        }
        rewrite_type(&mut function.return_type, target.module, program, programs)?;
        function.requires.clear();
        function.ensures.clear();
        function.body = default_expr(&function.return_type, &type_declarations)?;
        synthetic.functions.push(function);
    }
    if !synthetic
        .functions
        .iter()
        .any(|function| function.name == "main")
    {
        reserve_builder_structure(std::mem::size_of::<Function>())?;
        reserve_builder_structure(std::mem::size_of::<Expr>())?;
        synthetic.functions.push(Function {
            stable_id: crate::bounded_output::budgeted_format(format_args!(
                "workspace.synthetic.main.{}",
                synthetic.module
            )),
            explicit_id: true,
            name: crate::bounded_output::budgeted_clone("main"),
            name_span: Span::default(),
            type_parameters: Vec::new(),
            params: Vec::new(),
            return_type: Type::I64,
            effects: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            body: Expr {
                kind: ExprKind::Int(0),
                span: Span::default(),
            },
            span: Span::default(),
        });
    }
    Ok(synthetic)
}

fn rewrite_type_declaration(
    declaration: &mut TypeDeclaration,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
) -> Result<(), Vec<Diagnostic>> {
    match &mut declaration.kind {
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                rewrite_type(&mut field.ty, target_module, caller, programs)?;
            }
        }
        TypeDeclarationKind::Class { fields, methods } => {
            for field in fields {
                rewrite_type(&mut field.ty, target_module, caller, programs)?;
            }
            for method in methods {
                for param in &mut method.params {
                    rewrite_type(&mut param.ty, target_module, caller, programs)?;
                }
                rewrite_type(&mut method.return_type, target_module, caller, programs)?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &mut case.fields {
                    rewrite_type(&mut field.ty, target_module, caller, programs)?;
                }
            }
        }
        TypeDeclarationKind::Resource { .. } => unreachable!("resource imports are rejected"),
    }
    Ok(())
}

fn rewrite_type(
    ty: &mut Type,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
) -> Result<(), Vec<Diagnostic>> {
    let Type::Named { name, arguments } = ty else {
        return Ok(());
    };
    if !arguments.is_empty() {
        return Err(vec![graph_error(
            "SPX-G172",
            "generic cross-file types are not admitted",
        )]);
    }
    let target_id = resolve_type_id(target_module, name, programs).ok_or_else(|| {
        vec![graph_error(
            "SPX-G173",
            "cross-file type identity lookup disagrees",
        )]
    })?;
    let alias = caller
        .module_uses
        .iter()
        .find(|item| item.kind == ModuleUseKind::Type && item.persistent_id == target_id)
        .map(|item| item.alias.as_str())
        .ok_or_else(|| {
            vec![graph_error(
                "SPX-G172",
                crate::bounded_output::budgeted_format(format_args!(
                    "cross-file signature type `{target_id}` is not explicitly imported"
                )),
            )]
        })?;
    *name = crate::bounded_output::budgeted_clone(alias);
    Ok(())
}

fn default_expr(
    ty: &Type,
    declarations: &[(&str, &TypeDeclaration)],
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
        Type::ArrayU8(_) | Type::Bytes => {
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
                                value: default_expr(&field.ty, declarations)?,
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
                                    value: default_expr(&field.ty, declarations)?,
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

pub(super) fn collect_expected_edges(
    program: &Program,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    let function_uses = program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Function)
        .map(|item| (item.alias.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let type_uses = program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Type)
        .map(|item| (item.alias.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    for (index, permit) in program.permits.iter().enumerate() {
        let path = crate::bounded_output::budgeted_format(format_args!("permit.{index}"));
        push_edge(
            edges,
            WorkspaceEdge {
                caller_path: crate::bounded_output::budgeted_clone(&program.path),
                caller: crate::bounded_output::budgeted_clone(&program.module),
                target_path: crate::bounded_output::budgeted_clone(&program.path),
                target: crate::bounded_output::budgeted_clone(permit),
                kind: "capability_authority",
                site: "module",
                expression: crate::bounded_output::budgeted_clone(&path),
                ast_path: path,
                alias: String::new(),
                ordinal: index,
            },
        )?;
    }
    for declaration in &program.types {
        let declaration_type_uses = ScopedTypeUses {
            uses: &type_uses,
            shadowed: &declaration.type_parameters,
        };
        match &declaration.kind {
            TypeDeclarationKind::Resource { .. } => {}
            TypeDeclarationKind::Record { fields } => {
                for (index, field) in fields.iter().enumerate() {
                    collect_type_reference_edge(
                        program,
                        &declaration.stable_id,
                        &field.ty,
                        &crate::bounded_output::budgeted_format(format_args!(
                            "type.{}.field.{index}",
                            declaration.stable_id
                        )),
                        declaration_type_uses,
                        module_paths,
                        authored,
                        edges,
                    )?;
                }
            }
            TypeDeclarationKind::Class { fields, methods } => {
                for (index, field) in fields.iter().enumerate() {
                    collect_type_reference_edge(
                        program,
                        &declaration.stable_id,
                        &field.ty,
                        &crate::bounded_output::budgeted_format(format_args!(
                            "type.{}.field.{index}",
                            declaration.stable_id
                        )),
                        declaration_type_uses,
                        module_paths,
                        authored,
                        edges,
                    )?;
                }
                for method in methods {
                    for (param_index, param) in method.params.iter().enumerate() {
                        collect_type_reference_edge(
                            program,
                            &method.stable_id,
                            &param.ty,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "fn.{}.param.{param_index}",
                                method.stable_id
                            )),
                            declaration_type_uses,
                            module_paths,
                            authored,
                            edges,
                        )?;
                    }
                    collect_type_reference_edge(
                        program,
                        &method.stable_id,
                        &method.return_type,
                        &crate::bounded_output::budgeted_format(format_args!(
                            "fn.{}.return",
                            method.stable_id
                        )),
                        declaration_type_uses,
                        module_paths,
                        authored,
                        edges,
                    )?;
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                for (case_index, case) in cases.iter().enumerate() {
                    for (field_index, field) in case.fields.iter().enumerate() {
                        collect_type_reference_edge(
                            program,
                            &declaration.stable_id,
                            &field.ty,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "type.{}.case.{case_index}.field.{field_index}",
                                declaration.stable_id
                            )),
                            declaration_type_uses,
                            module_paths,
                            authored,
                            edges,
                        )?;
                    }
                }
            }
        }
    }
    for function in &program.functions {
        let function_type_uses = ScopedTypeUses {
            uses: &type_uses,
            shadowed: &function.type_parameters,
        };
        for (index, param) in function.params.iter().enumerate() {
            collect_type_reference_edge(
                program,
                &function.stable_id,
                &param.ty,
                &crate::bounded_output::budgeted_format(format_args!(
                    "function.{}.param.{index}",
                    function.stable_id
                )),
                function_type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        collect_type_reference_edge(
            program,
            &function.stable_id,
            &function.return_type,
            &crate::bounded_output::budgeted_format(format_args!(
                "function.{}.return",
                function.stable_id
            )),
            function_type_uses,
            module_paths,
            authored,
            edges,
        )?;
        for (site, expressions) in [
            ("requires", function.requires.as_slice()),
            ("body", std::slice::from_ref(&function.body)),
            ("ensures", function.ensures.as_slice()),
        ] {
            for (root_index, expression) in expressions.iter().enumerate() {
                let root = match site {
                    "requires" => crate::bounded_output::budgeted_format(format_args!(
                        "requires.{root_index}"
                    )),
                    "body" => crate::bounded_output::budgeted_clone("body"),
                    "ensures" => {
                        crate::bounded_output::budgeted_format(format_args!("ensures.{root_index}"))
                    }
                    _ => unreachable!(),
                };
                let mut call_ordinal = 0usize;
                visit_ast_call_sites(expression, &root, &mut |name, path| {
                    let ordinal = call_ordinal;
                    call_ordinal += 1;
                    if let Some(module_use) = function_uses.get(name) {
                        let target = &authored[module_use.persistent_id.as_str()];
                        let edge = WorkspaceEdge {
                            caller_path: crate::bounded_output::budgeted_clone(&program.path),
                            caller: crate::bounded_output::budgeted_clone(&function.stable_id),
                            target_path: crate::bounded_output::budgeted_clone(
                                module_paths[target.module],
                            ),
                            target: crate::bounded_output::budgeted_clone(
                                &module_use.persistent_id,
                            ),
                            kind: "call",
                            site,
                            expression: hir::workspace_expression_identity(
                                &hir::DeclarationId::new(crate::bounded_output::budgeted_clone(
                                    &function.stable_id,
                                )),
                                path,
                            ),
                            ast_path: crate::bounded_output::budgeted_clone(path),
                            alias: crate::bounded_output::budgeted_clone(&module_use.alias),
                            ordinal,
                        };
                        push_edge(edges, edge)?;
                        if let Some(target_function) = target.function {
                            for effect in &target_function.effects {
                                push_edge(
                                    edges,
                                    WorkspaceEdge {
                                        caller_path: crate::bounded_output::budgeted_clone(
                                            &program.path,
                                        ),
                                        caller: crate::bounded_output::budgeted_clone(
                                            &function.stable_id,
                                        ),
                                        target_path: crate::bounded_output::budgeted_clone(
                                            target.path,
                                        ),
                                        target: crate::bounded_output::budgeted_clone(effect),
                                        kind: "effect_requirement",
                                        site,
                                        expression: hir::workspace_expression_identity(
                                            &hir::DeclarationId::new(
                                                crate::bounded_output::budgeted_clone(
                                                    &function.stable_id,
                                                ),
                                            ),
                                            path,
                                        ),
                                        ast_path: crate::bounded_output::budgeted_clone(path),
                                        alias: crate::bounded_output::budgeted_clone(
                                            &module_use.alias,
                                        ),
                                        ordinal,
                                    },
                                )?;
                            }
                        }
                    }
                    Ok(())
                })?;
                collect_expression_type_edges(
                    program,
                    &function.stable_id,
                    expression,
                    &root,
                    function_type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
    }
    for (ordinal, module_use) in program.module_uses.iter().enumerate() {
        if module_use.kind == ModuleUseKind::Protocol {
            continue;
        }
        let target = &authored[module_use.persistent_id.as_str()];
        push_edge(
            edges,
            WorkspaceEdge {
                caller_path: crate::bounded_output::budgeted_clone(&program.path),
                caller: crate::bounded_output::budgeted_clone(&program.module),
                target_path: crate::bounded_output::budgeted_clone(module_paths[target.module]),
                target: crate::bounded_output::budgeted_clone(&module_use.persistent_id),
                kind: match module_use.kind {
                    ModuleUseKind::Function => "function_import",
                    ModuleUseKind::Type => "type_import",
                    ModuleUseKind::Protocol => unreachable!(),
                },
                site: "module",
                expression: crate::bounded_output::budgeted_format(format_args!("use.{ordinal}")),
                ast_path: crate::bounded_output::budgeted_format(format_args!("use.{ordinal}")),
                alias: crate::bounded_output::budgeted_clone(&module_use.alias),
                ordinal,
            },
        )?;
    }
    edges.sort();
    Ok(())
}

#[derive(Clone, Copy)]
struct ScopedTypeUses<'a> {
    uses: &'a BTreeMap<&'a str, &'a ModuleUse>,
    shadowed: &'a [crate::ast::TypeParameterDeclaration],
}

impl<'a> ScopedTypeUses<'a> {
    fn get(self, name: &str) -> Option<&'a ModuleUse> {
        if self.shadowed.iter().any(|parameter| parameter.name == name) {
            None
        } else {
            self.uses.get(name).copied()
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_type_reference_edge(
    program: &Program,
    owner: &str,
    ty: &Type,
    path: &str,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    collect_type_reference_edge_at(
        program,
        owner,
        ty,
        path,
        None,
        type_uses,
        module_paths,
        authored,
        edges,
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_type_reference_edge_at(
    program: &Program,
    owner: &str,
    ty: &Type,
    path: &str,
    expression: Option<&str>,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    let Type::Named { name, arguments } = ty else {
        return Ok(());
    };
    collect_named_type_reference_edge_at(
        program,
        owner,
        name,
        arguments,
        path,
        expression,
        type_uses,
        module_paths,
        authored,
        edges,
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_named_type_reference_edge_at(
    program: &Program,
    owner: &str,
    name: &str,
    arguments: &[Type],
    path: &str,
    expression: Option<&str>,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    if let Some(module_use) = type_uses.get(name) {
        let target = &authored[module_use.persistent_id.as_str()];
        push_edge(
            edges,
            WorkspaceEdge {
                caller_path: crate::bounded_output::budgeted_clone(&program.path),
                caller: crate::bounded_output::budgeted_clone(owner),
                target_path: crate::bounded_output::budgeted_clone(module_paths[target.module]),
                target: crate::bounded_output::budgeted_clone(&module_use.persistent_id),
                kind: "type_reference",
                site: "type",
                expression: crate::bounded_output::budgeted_clone(expression.unwrap_or(path)),
                ast_path: crate::bounded_output::budgeted_clone(path),
                alias: crate::bounded_output::budgeted_clone(&module_use.alias),
                ordinal: edges.len(),
            },
        )?;
    }
    for (index, argument) in arguments.iter().enumerate() {
        let argument_path =
            crate::bounded_output::budgeted_format(format_args!("{path}.argument.{index}"));
        collect_type_reference_edge_at(
            program,
            owner,
            argument,
            &argument_path,
            expression,
            type_uses,
            module_paths,
            authored,
            edges,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_expression_type_edges(
    program: &Program,
    owner: &str,
    expression: &Expr,
    path: &str,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    let expression_id = hir::workspace_expression_identity(
        &hir::DeclarationId::new(crate::bounded_output::budgeted_clone(owner)),
        path,
    );
    match &expression.kind {
        ExprKind::Call {
            type_arguments,
            args,
            ..
        } => {
            for (index, argument) in type_arguments.iter().enumerate() {
                let type_path = crate::bounded_output::budgeted_format(format_args!(
                    "{path}.type_argument.{index}"
                ));
                collect_type_reference_edge_at(
                    program,
                    owner,
                    argument,
                    &type_path,
                    Some(&expression_id),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
            for (index, argument) in args.iter().enumerate() {
                let child =
                    crate::bounded_output::budgeted_format(format_args!("{path}.arg.{index}"));
                collect_expression_type_edges(
                    program,
                    owner,
                    argument,
                    &child,
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::MethodCall {
            receiver,
            type_arguments,
            args,
            ..
        } => {
            for (index, argument) in type_arguments.iter().enumerate() {
                let type_path = crate::bounded_output::budgeted_format(format_args!(
                    "{path}.type_argument.{index}"
                ));
                collect_type_reference_edge_at(
                    program,
                    owner,
                    argument,
                    &type_path,
                    Some(&expression_id),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
            collect_expression_type_edges(
                program,
                owner,
                receiver,
                &crate::bounded_output::budgeted_format(format_args!("{path}.receiver")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            for (index, argument) in args.iter().enumerate() {
                let child =
                    crate::bounded_output::budgeted_format(format_args!("{path}.arg.{index}"));
                collect_expression_type_edges(
                    program,
                    owner,
                    argument,
                    &child,
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::SuperMethod { args, .. } => {
            for (index, argument) in args.iter().enumerate() {
                let child =
                    crate::bounded_output::budgeted_format(format_args!("{path}.arg.{index}"));
                collect_expression_type_edges(
                    program,
                    owner,
                    argument,
                    &child,
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::Unary { value, .. } => collect_expression_type_edges(
            program,
            owner,
            value,
            &crate::bounded_output::budgeted_format(format_args!("{path}.value")),
            type_uses,
            module_paths,
            authored,
            edges,
        )?,
        ExprKind::Binary { left, right, .. } => {
            collect_expression_type_edges(
                program,
                owner,
                left,
                &crate::bounded_output::budgeted_format(format_args!("{path}.left")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            collect_expression_type_edges(
                program,
                owner,
                right,
                &crate::bounded_output::budgeted_format(format_args!("{path}.right")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        ExprKind::Block { statements, tail } => {
            for (index, statement) in statements.iter().enumerate() {
                let child_count = statement.child_count();
                for child_index in 0..child_count {
                    let Some(child) = statement.child(child_index) else {
                        continue;
                    };
                    let segment = if matches!(statement, crate::ast::Statement::While { .. }) {
                        if child_index == 0 {
                            "condition"
                        } else {
                            "body"
                        }
                    } else {
                        "value"
                    };
                    collect_expression_type_edges(
                        program,
                        owner,
                        child,
                        &crate::bounded_output::budgeted_format(format_args!(
                            "{path}.s{index}.{segment}"
                        )),
                        type_uses,
                        module_paths,
                        authored,
                        edges,
                    )?;
                }
            }
            collect_expression_type_edges(
                program,
                owner,
                tail,
                &crate::bounded_output::budgeted_format(format_args!("{path}.tail")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            for (suffix, child) in [
                ("condition", condition.as_ref()),
                ("then", then_branch.as_ref()),
                ("else", else_branch.as_ref()),
            ] {
                collect_expression_type_edges(
                    program,
                    owner,
                    child,
                    &crate::bounded_output::budgeted_format(format_args!("{path}.{suffix}")),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            fields,
            ..
        }
        | ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            fields,
            ..
        } => {
            collect_named_type_reference_edge_at(
                program,
                owner,
                type_name,
                type_arguments,
                &crate::bounded_output::budgeted_format(format_args!("{path}.type")),
                Some(&expression_id),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            for (index, field) in fields.iter().enumerate() {
                collect_expression_type_edges(
                    program,
                    owner,
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_expression_type_edges(
                program,
                owner,
                scrutinee,
                &crate::bounded_output::budgeted_format(format_args!("{path}.scrutinee")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            for (index, arm) in arms.iter().enumerate() {
                let pattern_path = crate::bounded_output::budgeted_format(format_args!(
                    "{path}.arm.{index}.pattern"
                ));
                collect_match_pattern_type_edges(
                    program,
                    owner,
                    &arm.pattern,
                    &pattern_path,
                    &expression_id,
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
                collect_expression_type_edges(
                    program,
                    owner,
                    &arm.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.arm.{index}.value"
                    )),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::Try { operand } => collect_expression_type_edges(
            program,
            owner,
            operand,
            &crate::bounded_output::budgeted_format(format_args!("{path}.operand")),
            type_uses,
            module_paths,
            authored,
            edges,
        )?,
        ExprKind::UpdateRecord { base, fields } => {
            collect_expression_type_edges(
                program,
                owner,
                base,
                &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            for (index, field) in fields.iter().enumerate() {
                collect_expression_type_edges(
                    program,
                    owner,
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::Project { base, .. } => collect_expression_type_edges(
            program,
            owner,
            base,
            &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
            type_uses,
            module_paths,
            authored,
            edges,
        )?,
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Var(_) => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_match_pattern_type_edges(
    program: &Program,
    owner: &str,
    pattern: &crate::ast::MatchPattern,
    path: &str,
    expression: &str,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    match pattern {
        crate::ast::MatchPattern::Variant { type_name, .. }
        | crate::ast::MatchPattern::Record { type_name, .. } => {
            collect_named_type_reference_edge_at(
                program,
                owner,
                type_name,
                &[],
                path,
                Some(expression),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        crate::ast::MatchPattern::Wildcard { .. } => {}
        // Refutable Match v1: literal/binding patterns reference no named
        // types; or-patterns recurse into their literal alternatives.
        crate::ast::MatchPattern::Literal { .. } | crate::ast::MatchPattern::Binding { .. } => {}
        crate::ast::MatchPattern::Or { alternatives, .. } => {
            for (index, alternative) in alternatives.iter().enumerate() {
                collect_match_pattern_type_edges(
                    program,
                    owner,
                    alternative,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.alternative.{index}"
                    )),
                    expression,
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
    }
    if let crate::ast::MatchPattern::Record { fields, .. } = pattern {
        for (index, field) in fields.iter().enumerate() {
            collect_record_pattern_type_edges(
                program,
                owner,
                &field.pattern,
                &crate::bounded_output::budgeted_format(format_args!(
                    "{path}.field.{index}.pattern"
                )),
                expression,
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_record_pattern_type_edges(
    program: &Program,
    owner: &str,
    pattern: &crate::ast::RecordMatchFieldPattern,
    path: &str,
    expression: &str,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    let crate::ast::RecordMatchFieldPattern::Record {
        type_name, fields, ..
    } = pattern
    else {
        return Ok(());
    };
    collect_named_type_reference_edge_at(
        program,
        owner,
        type_name,
        &[],
        path,
        Some(expression),
        type_uses,
        module_paths,
        authored,
        edges,
    )?;
    for (index, field) in fields.iter().enumerate() {
        collect_record_pattern_type_edges(
            program,
            owner,
            &field.pattern,
            &crate::bounded_output::budgeted_format(format_args!("{path}.field.{index}.pattern")),
            expression,
            type_uses,
            module_paths,
            authored,
            edges,
        )?;
    }
    Ok(())
}

pub(super) fn verify_resolved_call_edges(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    let aliases = program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Function)
        .map(|item| (item.alias.as_str(), item.persistent_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut expected = Vec::new();
    for function in &program.functions {
        let owner =
            hir::DeclarationId::new(crate::bounded_output::budgeted_clone(&function.stable_id));
        for (root, expression) in function
            .requires
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                (
                    crate::bounded_output::budgeted_format(format_args!("requires.{index}")),
                    expression,
                )
            })
            .chain(std::iter::once((
                crate::bounded_output::budgeted_clone("body"),
                &function.body,
            )))
            .chain(
                function
                    .ensures
                    .iter()
                    .enumerate()
                    .map(|(index, expression)| {
                        (
                            crate::bounded_output::budgeted_format(format_args!("ensures.{index}")),
                            expression,
                        )
                    }),
            )
        {
            visit_ast_call_sites(expression, &root, &mut |name, path| {
                if let Some(target) = aliases.get(name) {
                    reserve_builder_structure(std::mem::size_of::<(
                        hir::DeclarationId,
                        hir::ExpressionId,
                        hir::DeclarationId,
                    )>())?;
                    expected.push((
                        hir::DeclarationId::new(crate::bounded_output::budgeted_clone(
                            owner.as_str(),
                        )),
                        hir::workspace_expression_identity(&owner, path),
                        hir::DeclarationId::new(crate::bounded_output::budgeted_clone(target)),
                    ));
                }
                Ok(())
            })?;
        }
    }
    let target_ids = program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Function)
        .map(|item| {
            hir::DeclarationId::new(crate::bounded_output::budgeted_clone(&item.persistent_id))
        })
        .collect::<BTreeSet<_>>();
    let mut actual = hir::workspace_call_sites(resolved);
    actual.retain(|(_, _, target)| target_ids.contains(target));
    expected.sort();
    actual.sort();
    if expected != actual
        || expected
            .iter()
            .any(|(_, _, target)| !authored.contains_key(target.as_str()))
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "independent workspace call-edge reconstruction disagrees with HIR",
        )]);
    }
    Ok(())
}
