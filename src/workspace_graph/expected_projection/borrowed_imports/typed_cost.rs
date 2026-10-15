//! Complete raw declaration/expression/pattern census, with unchanged order
//! and formulas. Actual controls and payments use the original affine receiver.
use crate::ast::{Expr, ExprKind, Function, Type, TypeDeclaration, TypeDeclarationKind};
use crate::workspace_graph::constructor_failure::receiving::{Receiver, Setup};
use crate::workspace_graph::expected_projection::cost::constructor::Cost;
use std::mem::{size_of, size_of_val};

pub(in crate::workspace_graph) fn ast_field_cost(
    field: &crate::ast::FieldDeclaration,
    cost: &mut Cost<'_>,
) -> Result<(), Setup> {
    cost.frame(size_of_val(&field) + size_of_val(&cost))?;
    cost.inline_type_parent(field)?;
    cost.embedded_string(&field.stable_id)?;
    cost.embedded_string(&field.name)?;
    ast_type_cost(&field.ty, cost)
}

pub(in crate::workspace_graph) fn ast_function_cost(
    function: &Function,
    cost: &mut Cost<'_>,
) -> Result<(), Setup> {
    cost.frame(size_of_val(&function) + size_of_val(&cost))?;
    ast_function_signature_cost(function, cost)?;
    ast_function_contract_cost_into(function, cost)
}

/// The part of a function every retained projection keeps: its identity, its
/// name, its type parameters, its parameters, its return type, and its effect
/// row. An imported function is retained as exactly this much plus a synthetic
/// default body.
pub(in crate::workspace_graph) fn ast_function_signature_cost(
    function: &Function,
    cost: &mut Cost<'_>,
) -> Result<(), Setup> {
    cost.frame(size_of_val(&function) + size_of_val(&cost))?;
    cost.function_signature(function)?;
    cost.embedded_string(&function.stable_id)?;
    cost.embedded_string(&function.name)?;
    for parameter in &function.type_parameters {
        cost.value(parameter)?;
        cost.embedded_string(&parameter.name)?;
    }
    for param in &function.params {
        ast_param_cost(param, cost)?;
    }
    ast_type_cost(&function.return_type, cost)?;
    for effect in &function.effects {
        cost.string(effect)?;
    }
    Ok(())
}

/// The preconditions, body, and postconditions of a function. An imported
/// function's contract is cloned transiently and then discarded, so it is
/// charged as a peak rather than as retained structure.
fn ast_function_contract_cost_into(function: &Function, cost: &mut Cost<'_>) -> Result<(), Setup> {
    cost.frame(size_of_val(&function) + size_of_val(&cost))?;
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

pub(in crate::workspace_graph) fn ast_param_cost(
    param: &crate::ast::Param,
    cost: &mut Cost<'_>,
) -> Result<(), Setup> {
    cost.frame(size_of_val(&param) + size_of_val(&cost))?;
    cost.inline_type_parent(param)?;
    cost.embedded_string(&param.name)?;
    ast_type_cost(&param.ty, cost)
}

pub(in crate::workspace_graph) fn ast_type_cost(
    ty: &Type,
    cost: &mut Cost<'_>,
) -> Result<(), Setup> {
    cost.frame(size_of_val(&ty) + size_of_val(&cost))?;
    cost.value(ty)?;
    if let Type::Named { name, arguments } = ty {
        cost.embedded_string(name)?;
        for argument in arguments {
            ast_type_cost(argument, cost)?;
        }
    }
    if cost.is_raw_fallback() {
        if let Type::Function { parameters, result } = ty {
            for parameter in parameters {
                ast_type_cost(parameter, cost)?;
            }
            ast_type_cost(result, cost)?;
        }
    }
    Ok(())
}

/// Where the parent places a child's resolved root. `CarrierFree` is used only
/// for a `Box` slot of a unary, binary, `if` or block-tail node, or an element
/// of a free-function call's argument list (every intrinsic route only moves
/// or boxes its arguments): no binding, statement, field initializer, arm or
/// pattern field is constructed for it there. Only literal/unary/binary and
/// `if` roots in such a slot are censused; variable roots are also censused
/// outside every authored closure subtree, retaining all their storage.
/// Block roots in those same positions are also carrier-free: their statements
/// remain independently charged and their tail is boxed by finish_block. Every
/// other child, and every root reached through `ast_expr_cost`, is `Delegated` and keeps its complete
/// bundle.
#[derive(Clone, Copy, Eq, PartialEq)]
enum RootPosition {
    CarrierFree,
    Delegated,
}

pub(in crate::workspace_graph) fn ast_expr_cost(
    expression: &Expr,
    cost: &mut Cost<'_>,
) -> Result<(), Setup> {
    cost.frame(size_of_val(&expression) + size_of_val(&cost))?;
    ast_expr_cost_at(expression, cost, RootPosition::Delegated, true)
}

fn ast_expr_cost_at(
    expression: &Expr,
    cost: &mut Cost<'_>,
    position: RootPosition,
    outside_closure: bool,
) -> Result<(), Setup> {
    cost.frame(
        size_of_val(&expression)
            + size_of_val(&cost)
            + size_of_val(&position)
            + size_of_val(&outside_closure),
    )?;
    use RootPosition::CarrierFree;
    cost.value(expression)?;
    // `super` allocates the initial receiver before recreating its source,
    // upcast wrapper and call root; the discarded receiver still costs bytes.
    cost.add_identity_carriers(if matches!(expression.kind, ExprKind::SuperMethod { .. }) {
        4
    } else {
        3
    })?;
    // These resolver branches construct one root ExpressionId. Child trees,
    // imported defaults, generic expansions, and desugared statements keep
    // their independent bounds; other shapes keep the three-carrier ceiling.
    if matches!(
        expression.kind,
        ExprKind::Int(_)
            | ExprKind::String(_)
            | ExprKind::BorrowedUtf8Literal(_)
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_)
            | ExprKind::Var(_)
            | ExprKind::Unary { .. }
            | ExprKind::Binary { .. }
            | ExprKind::If { .. }
            | ExprKind::Block { .. }
    ) {
        cost.note_single_identity_carrier()?;
    }
    cost.note_exact_expr_fixed_bundle()?;
    cost.account_scalar_identity(&expression.kind)?;
    cost.account_exact_absent_storage(&expression.kind)?;
    cost.account_carrier_free_root(&expression.kind, position == CarrierFree, outside_closure)?;
    match &expression.kind {
        ExprKind::Closure {
            params,
            return_type,
            body,
            ..
        } => {
            for parameter in params {
                cost.inline_type_parent(parameter)?;
                cost.embedded_string(&parameter.name)?;
                ast_type_cost(&parameter.ty, cost)?;
            }
            ast_type_cost(return_type, cost)?;
            ast_expr_cost_at(body, cost, RootPosition::Delegated, false)?;
        }
        ExprKind::Var(name) => cost.embedded_string(name)?,
        ExprKind::ArrayU8(values) => cost.add(values.len())?,
        ExprKind::RepeatArrayU8 { .. } => {}
        ExprKind::Call {
            name,
            type_arguments,
            args,
        } => {
            cost.embedded_string(name)?;
            // Complete synthetic key/layout identities are additional HIR text.
            // Bound every transient replay buffer before entering the builder;
            // descriptor construction itself charges its actual allocation.
            if crate::vec_ops::field_unique::literal_call(name)
                || name == crate::vec_ops::field_update::NAME
            {
                cost.add_split(
                    8 * crate::vec_ops::sort_keys::MAX_DESCRIPTOR_BYTES,
                    8 * crate::vec_ops::sort_keys::MAX_DESCRIPTOR_BYTES,
                )?;
            }
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for argument in args {
                ast_expr_cost_at(argument, cost, CarrierFree, outside_closure)?;
            }
        }
        ExprKind::LexicalExit { value: None, .. } => {}
        ExprKind::LexicalExit {
            value: Some(value), ..
        } => ast_expr_cost_at(value, cost, RootPosition::Delegated, outside_closure)?,
        ExprKind::Unary { value, .. } => {
            ast_expr_cost_at(value, cost, CarrierFree, outside_closure)?
        }
        ExprKind::VecConstructor { element, value, .. } => {
            ast_type_cost(element, cost)?;
            ast_expr_cost_at(value, cost, RootPosition::Delegated, outside_closure)?;
        }
        ExprKind::Binary { left, right, .. } => {
            ast_expr_cost_at(left, cost, CarrierFree, outside_closure)?;
            ast_expr_cost_at(right, cost, CarrierFree, outside_closure)?;
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                if matches!(
                    statement,
                    crate::ast::Statement::Let { .. } | crate::ast::Statement::Assign { .. }
                ) {
                    cost.inline_expr_parent(statement)?;
                } else {
                    cost.value(statement)?;
                }
                if matches!(
                    statement,
                    crate::ast::Statement::For { .. }
                        | crate::ast::Statement::ForBorrow { .. }
                        | crate::ast::Statement::ForOwn { .. }
                ) {
                    cost.add_identity_carriers(16)?;
                }
                // Charge the string every statement actually carries. Unsafe
                // boundaries carry their verbatim audit summary, `while`
                // carries no binding at all, bounded `for` traversal carries
                // its item binding, and `let`/assignment carry their name.
                // Matching the statement exhaustively keeps a later variant a
                // compile error here rather than a panic at check time:
                // `Statement::name` panics for every statement that is not a
                // `let` or an assignment.
                match statement {
                    crate::ast::Statement::Unsafe { audit, .. } => cost.embedded_string(audit)?,
                    crate::ast::Statement::While { .. } => cost.string("")?,
                    crate::ast::Statement::For { item, .. }
                    | crate::ast::Statement::ForOwn { item, .. } => cost.embedded_string(item)?,
                    crate::ast::Statement::ForBorrow { item, index, .. } => {
                        cost.embedded_string(item)?;
                        if let Some(index) = index {
                            cost.embedded_string(&index.name)?;
                        }
                    }
                    crate::ast::Statement::Let { name, .. }
                    | crate::ast::Statement::Assign { name, .. } => cost.embedded_string(name)?,
                }
                if cost.is_raw_fallback() {
                    match statement {
                        crate::ast::Statement::Assign {
                            field: Some(field), ..
                        } => cost.embedded_string(&field.name)?,
                        crate::ast::Statement::Let {
                            declared: Some(ty), ..
                        } => ast_type_cost(ty, cost)?,
                        _ => {}
                    }
                }
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        ast_expr_cost_at(child, cost, RootPosition::Delegated, outside_closure)?;
                    }
                }
            }
            ast_expr_cost_at(tail, cost, CarrierFree, outside_closure)?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            ast_expr_cost_at(condition, cost, CarrierFree, outside_closure)?;
            ast_expr_cost_at(then_branch, cost, CarrierFree, outside_closure)?;
            ast_expr_cost_at(else_branch, cost, CarrierFree, outside_closure)?;
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
            cost.embedded_string(type_name)?;
            if let ExprKind::ConstructVariant { case_name, .. } = &expression.kind {
                cost.embedded_string(case_name)?;
            }
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for field in fields {
                cost.inline_expr_parent(field)?;
                cost.embedded_string(&field.name)?;
                ast_expr_cost_at(&field.value, cost, RootPosition::Delegated, outside_closure)?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            ast_expr_cost_at(scrutinee, cost, RootPosition::Delegated, outside_closure)?;
            for arm in arms {
                cost.match_arm(arm)?;
                ast_pattern_cost(&arm.pattern, cost)?;
                // The boxed guard is a complete resolved expression tree too;
                // `match_arm` charges only its pointer.
                if let Some(guard) = &arm.guard {
                    ast_expr_cost_at(guard, cost, RootPosition::Delegated, outside_closure)?;
                }
                ast_expr_cost_at(&arm.value, cost, RootPosition::Delegated, outside_closure)?;
            }
        }
        ExprKind::Try { operand } | ExprKind::Yield { request: operand } => {
            ast_expr_cost_at(operand, cost, RootPosition::Delegated, outside_closure)?
        }
        ExprKind::UpdateRecord { base, fields } => {
            ast_expr_cost_at(base, cost, RootPosition::Delegated, outside_closure)?;
            for field in fields {
                cost.inline_expr_parent(field)?;
                cost.embedded_string(&field.name)?;
                ast_expr_cost_at(&field.value, cost, RootPosition::Delegated, outside_closure)?;
            }
        }
        ExprKind::Project { base, field, .. } => {
            ast_expr_cost_at(base, cost, RootPosition::Delegated, outside_closure)?;
            cost.embedded_string(field)?;
        }
        ExprKind::MethodCall {
            receiver,
            method,
            type_arguments,
            args,
            ..
        } => {
            ast_expr_cost_at(receiver, cost, RootPosition::Delegated, outside_closure)?;
            cost.embedded_string(method)?;
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for argument in args {
                ast_expr_cost_at(argument, cost, RootPosition::Delegated, outside_closure)?;
            }
        }
        ExprKind::SuperMethod { method, args, .. } => {
            cost.embedded_string(method)?;
            for argument in args {
                ast_expr_cost_at(argument, cost, RootPosition::Delegated, outside_closure)?;
            }
        }
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_) => {}
        ExprKind::String(value) | ExprKind::BorrowedUtf8Literal(value) => {
            if cost.is_raw_fallback() {
                cost.embedded_string(value)?;
            }
        }
    }
    Ok(())
}

fn ast_type_declaration_cost(
    declaration: &TypeDeclaration,
    cost: &mut Cost<'_>,
) -> Result<(), Setup> {
    cost.frame(size_of_val(&declaration) + size_of_val(&cost))?;
    cost.value(declaration)?;
    cost.embedded_string(&declaration.stable_id)?;
    cost.embedded_string(&declaration.name)?;
    for parameter in &declaration.type_parameters {
        cost.value(parameter)?;
        cost.embedded_string(&parameter.name)?;
    }
    match &declaration.kind {
        TypeDeclarationKind::Resource { lifecycles } => {
            for lifecycle in lifecycles {
                cost.value(lifecycle)?;
                if let Some(id) = &lifecycle.stable_id {
                    cost.embedded_string(id)?;
                }
                if let crate::ast::ResourceLifecycleKind::Imported { import_key } = &lifecycle.kind
                {
                    cost.embedded_string(import_key)?;
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
                cost.embedded_string(&case.stable_id)?;
                cost.embedded_string(&case.name)?;
                for field in &case.fields {
                    ast_field_cost(field, cost)?;
                }
            }
        }
    }
    Ok(())
}
fn ast_pattern_cost(pattern: &crate::ast::MatchPattern, cost: &mut Cost<'_>) -> Result<(), Setup> {
    cost.frame(size_of_val(&pattern) + size_of_val(&cost))?;
    cost.value(pattern)?;
    match pattern {
        crate::ast::MatchPattern::Variant {
            type_name,
            case_name,
            fields,
            ..
        } => {
            cost.embedded_string(type_name)?;
            cost.embedded_string(case_name)?;
            for field in fields {
                cost.value(field)?;
                cost.embedded_string(&field.name)?;
                cost.embedded_string(&field.binding)?;
            }
        }
        crate::ast::MatchPattern::Record {
            type_name, fields, ..
        } => {
            cost.embedded_string(type_name)?;
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
            cost.embedded_string(name)?;
        }
    }
    Ok(())
}
fn ast_record_pattern_field_cost(
    field: &crate::ast::RecordMatchPatternField,
    cost: &mut Cost<'_>,
) -> Result<(), Setup> {
    cost.frame(size_of_val(&field) + size_of_val(&cost))?;
    cost.inline_pattern_parent(field, &field.pattern)?;
    cost.embedded_string(&field.name)?;
    cost.value(&field.pattern)?;
    match &field.pattern {
        crate::ast::RecordMatchFieldPattern::Binding { name, .. } => cost.embedded_string(name)?,
        crate::ast::RecordMatchFieldPattern::Record {
            type_name, fields, ..
        } => {
            cost.embedded_string(type_name)?;
            for field in fields {
                ast_record_pattern_field_cost(field, cost)?;
            }
        }
        crate::ast::RecordMatchFieldPattern::Wildcard { .. } => {}
    }
    Ok(())
}

pub(super) fn pay_type(declaration: &TypeDeclaration, owner: &mut Receiver) -> Result<(), Setup> {
    owner.reserve(
        size_of::<Result<(), Setup>>() + size_of_val(&declaration) + size_of_val(&owner),
    )?;
    let mut cost = Cost::new(owner, true)?;
    ast_type_declaration_cost(declaration, &mut cost)?;
    cost.pay()
}
pub(super) fn pay_signature(function: &Function, owner: &mut Receiver) -> Result<(), Setup> {
    owner.reserve(size_of::<Result<(), Setup>>() + size_of_val(&function) + size_of_val(&owner))?;
    let mut cost = Cost::new(owner, true)?;
    ast_function_signature_cost(function, &mut cost)?;
    cost.pay()
}
pub(super) fn teardown_type(
    declaration: &TypeDeclaration,
    owner: &mut Receiver,
) -> Result<(), Setup> {
    owner.reserve(
        size_of::<Result<(), Setup>>() + size_of_val(&declaration) + size_of_val(&owner),
    )?;
    let mut cost = Cost::new(owner, true)?;
    ast_type_declaration_cost(declaration, &mut cost)?;
    cost.teardown()
}
pub(super) fn teardown_function(
    function: &Function,
    retained: bool,
    owner: &mut Receiver,
) -> Result<(), Setup> {
    owner.reserve(
        size_of::<Result<(), Setup>>()
            + size_of_val(&function)
            + size_of_val(&retained)
            + size_of_val(&owner),
    )?;
    let mut cost = Cost::new(owner, true)?;
    if retained {
        ast_function_cost(function, &mut cost)?;
        cost.pay()?;
    } else {
        ast_function_signature_cost(function, &mut cost)?;
    }
    cost.teardown()
}

pub(super) fn pay_default_teardown(
    ty: &Type,
    declarations: &[(&str, &TypeDeclaration)],
    owner: &mut Receiver,
) -> Result<(), Setup> {
    owner.reserve(
        size_of::<Result<(), Setup>>()
            + size_of_val(&ty)
            + size_of_val(&declarations)
            + size_of_val(&owner),
    )?;
    fn census(
        ty: &Type,
        declarations: &[(&str, &TypeDeclaration)],
        owner: &mut Receiver,
    ) -> Result<usize, Setup> {
        owner.reserve(
            size_of::<Result<usize, Setup>>()
                + size_of_val(&ty)
                + size_of_val(&declarations)
                + size_of_val(&owner)
                + size_of::<usize>()
                + size_of::<Option<usize>>(),
        )?;
        let mut bytes = std::mem::size_of::<Expr>();
        if let Type::Named { name, arguments } = ty {
            if arguments.is_empty() {
                if let Ok(index) =
                    declarations.binary_search_by_key(&name.as_str(), |(name, _)| *name)
                {
                    let fields: &[crate::ast::FieldDeclaration] = match &declarations[index].1.kind
                    {
                        TypeDeclarationKind::Record { fields }
                        | TypeDeclarationKind::Class { fields, .. } => fields,
                        TypeDeclarationKind::Variant { cases } => {
                            cases.first().map_or(&[], |case| &case.fields)
                        }
                        TypeDeclarationKind::Resource { .. } => &[],
                    };
                    for field in fields {
                        bytes = bytes
                            .checked_add(census(&field.ty, declarations, owner)?)
                            .ok_or_else(|| owner.limit())?;
                    }
                }
            } else {
                // Accepted compiler collection defaults have a root call,
                // one scalar size operand and copied type arguments. Generic
                // authored defaults still receive their old closed refusal.
                bytes = bytes
                    .checked_add(std::mem::size_of::<Expr>())
                    .ok_or_else(|| owner.limit())?;
                let mut cost = Cost::new(owner, false)?;
                ast_type_cost(ty, &mut cost)?;
                let total = cost.raw.total;
                drop(cost);
                bytes = bytes.checked_add(total).ok_or_else(|| owner.limit())?;
            }
        } else if *ty == Type::Bytes || *ty == Type::StringMap {
            bytes = bytes
                .checked_add(std::mem::size_of::<Expr>())
                .ok_or_else(|| owner.limit())?;
        }
        Ok(bytes)
    }
    let bytes = census(ty, declarations, owner)?
        .checked_mul(4)
        .ok_or_else(|| owner.limit())?;
    owner.reserve(bytes)
}

pub(super) fn pay_teardown_minimum(owner: &mut Receiver) -> Result<(), Setup> {
    owner.reserve(
        size_of::<Result<(), Setup>>()
            + size_of::<&mut Receiver>()
            + size_of::<usize>()
            + size_of::<Option<usize>>()
            + size_of::<[usize; 5]>(),
    )?;
    let bytes = [
        std::mem::size_of::<Function>(),
        std::mem::size_of::<Expr>(),
        std::mem::size_of::<Type>(),
        std::mem::size_of::<crate::ast::MatchPattern>(),
        std::mem::size_of::<crate::ast::RecordMatchPatternField>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, size| sum.checked_add(size))
    .ok_or_else(|| owner.limit())?;
    let bytes = bytes.checked_mul(4).ok_or_else(|| owner.limit())?;
    owner.reserve(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::{current_metrics, measure};
    use crate::workspace_graph::constructor_failure::typed::Failure;
    use crate::workspace_graph::expected_projection::{self, StructuralCost};
    const LIMIT: usize = 64 * 1024 * 1024;
    fn facts(raw: &StructuralCost) -> [usize; 15] {
        [
            raw.total,
            raw.string_bytes,
            raw.scalar_identity_discount,
            raw.literal_fixed_discount,
            raw.identity_carriers,
            raw.single_identity_carriers,
            raw.exact_fixed_discount,
            raw.exact_expressions,
            raw.exact_absent_roots[0],
            raw.exact_absent_roots[1],
            raw.exact_absent_roots[2],
            raw.exact_carrier_free_roots,
            raw.exact_carrier_free_ifs,
            raw.exact_carrier_free_vars,
            raw.exact_carrier_free_blocks,
        ]
    }
    #[test]
    fn typed_constructor_census_keeps_all_original_raw_fields_order_and_teardown_formulas() {
        let sources = [
            "module test; @id(\"main\") fn main()->i64 { let x=1; x }",
            "module test; @id(\"main\") fn main()->i64 { while false {0} 0 }",
            "module test; @id(\"main\") fn main()->i64 { if true {1 + -2} else {callee(3)} }",
            "module test; @id(\"main\") fn main()->i64 { let mut value=0; value.x=1; 0 }",
            "module test; @id(\"main\") fn main()->i64 { let x:Input=0; 0 }",
            "module test; @id(\"row\") record Row {@id(\"x\") x:i64,} @id(\"main\") fn main()->i64 {0}",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/std/collections/src/collections.spx"
            )),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/std/mem/src/mem.spx")),
        ];
        for source in sources {
            let program = crate::parse(source, "typed-census.spx").unwrap();
            for inline in [false, true] {
                for function in &program.functions {
                    let mut original = StructuralCost::raw_ast(true).with_inline_values(inline);
                    expected_projection::ast_function_cost(function, &mut original).unwrap();
                    let expected = facts(&original);
                    let (((), physical), refused, used, first) =
                        with_limit_usage_refusal(LIMIT, || {
                            measure(|| {
                                let mut owner = Receiver::acquire().unwrap();
                                let mut cost = Cost::new(&mut owner, inline).unwrap();
                                ast_function_cost(function, &mut cost).unwrap();
                                assert_eq!(facts(&cost.raw), expected);
                                cost.teardown().unwrap();
                                drop(cost);
                                drop(owner);
                            })
                        });
                    assert!(!refused && first.is_none());
                    assert!(
                        !physical.invalid
                            && physical.live == 0
                            && physical.allocated == physical.freed
                    );
                    assert!(physical.allocated <= used);
                }
                for declaration in &program.types {
                    let mut original = StructuralCost::raw_ast(true).with_inline_values(inline);
                    expected_projection::ast_type_declaration_cost(declaration, &mut original)
                        .unwrap();
                    let expected = facts(&original);
                    let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
                        let mut owner = Receiver::acquire().unwrap();
                        let mut cost = Cost::new(&mut owner, inline).unwrap();
                        ast_type_declaration_cost(declaration, &mut cost).unwrap();
                        assert_eq!(facts(&cost.raw), expected);
                        drop(cost);
                        drop(owner);
                    });
                    assert!(!refused && first.is_none());
                    let index = [(declaration.name.as_str(), declaration)];
                    let ty = Type::Named {
                        name: declaration.name.clone(),
                        arguments: Vec::new(),
                    };
                    let ((), refused, old, first) = with_limit_usage_refusal(LIMIT, || {
                        super::super::super::pay_default_teardown(&ty, &index).unwrap();
                    });
                    assert!(!refused && first.is_none());
                    let ((), refused, new, first) = with_limit_usage_refusal(LIMIT, || {
                        let mut owner = Receiver::acquire().unwrap();
                        pay_default_teardown(&ty, &index, &mut owner).unwrap();
                        drop(owner);
                    });
                    assert!(!refused && first.is_none());
                    assert!(
                        new > old,
                        "actual controls are additional; no old fee credit"
                    );
                }
            }
        }
    }

    #[test]
    fn typed_constructor_cost_every_byte_frontier_and_retry_allocate_nothing_after_refusal() {
        let program = crate::parse(
            "module test; @id(\"main\") fn main()->i64 {1 + -2}",
            "frontier.spx",
        )
        .unwrap();
        let function = &program.functions[0];
        let ((), refused, exact, first) = with_limit_usage_refusal(LIMIT, || {
            let mut owner = Receiver::acquire().unwrap();
            teardown_function(function, true, &mut owner).unwrap();
            drop(owner);
        });
        assert!(!refused && first.is_none());
        for limit in 0..=exact {
            let (((), physical), refused, used, first) = with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut owner = match Receiver::acquire() {
                        Ok(owner) => owner,
                        Err(Setup::Budget { .. }) => return,
                        Err(other) => panic!("unexpected owner setup: {other:?}"),
                    };
                    match teardown_function(function, true, &mut owner) {
                        Ok(()) => assert_eq!(limit, exact),
                        Err(selected @ Setup::Budget { .. }) => {
                            let stop = current_metrics().allocated;
                            assert_eq!(
                                teardown_function(function, true, &mut owner),
                                Err(selected)
                            );
                            drop(owner);
                            assert_eq!(current_metrics().allocated, stop);
                            return;
                        }
                        Err(other) => panic!("unexpected census setup: {other:?}"),
                    }
                    drop(owner);
                })
            });
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_some(), refused);
            assert!(used <= limit);
            assert!(
                !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
            );
            assert!(physical.allocated <= used);
        }
    }

    #[test]
    fn typed_constructor_logical_cost_cap_has_no_invented_receipt_and_foreign_stays_detached() {
        let maximum = crate::workspace_graph::active_builder_limit();
        let expected =
            crate::workspace_graph::checked_usage(usize::MAX, 1, "builder_bytes", maximum)
                .unwrap_err();
        let ((failure, physical), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let mut owner = Receiver::acquire().unwrap();
                let selected = owner.sum(usize::MAX, 1).unwrap_err();
                assert_eq!(selected, Setup::Budget { maximum });
                let stop = current_metrics().allocated;
                assert_eq!(owner.sum(0, 0), Err(selected));
                let result = owner.receive_typed(Failure::Setup(selected));
                assert_eq!(current_metrics().allocated, stop);
                result
            })
        });
        assert!(!refused && first.is_none());
        assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
        // The allocating legacy adapter remains explicit and outside payment.
        assert_eq!(failure.into_legacy_unfunded(), expected);
        let ((), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            let mut owner = Receiver::acquire().unwrap();
            let (((), physical), refused, used, first) = with_limit_usage_refusal(7, || {
                measure(|| {
                    assert_eq!(owner.sum(usize::MAX, 1), Err(Setup::Detached));
                    assert_eq!(owner.limit(), Setup::Detached);
                })
            });
            assert!(!refused && used == 0 && first.is_none());
            assert_eq!(physical.allocated, 0);
            assert_eq!(owner.sum(0, 0), Err(Setup::Detached));
            drop(owner);
        });
        assert!(refused && first.is_some());
    }
    #[test]
    fn typed_constructor_cost_complete_originals_live_callers_and_all_source_inverses_are_bound() {
        use sha2::{Digest, Sha256};
        let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-cost-propagation-825/source-inverses.json"
        )))
        .unwrap();
        let sources = [
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/workspace_graph/constructor_failure/receiving/typed.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-cost-propagation-825/original/00.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/workspace_graph/expected_projection/borrowed_imports/typed.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-cost-propagation-825/original/01.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/workspace_graph/expected_projection/cost.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-cost-propagation-825/original/02.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/workspace_graph/expected_projection/borrowed_imports/typed_cost.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-cost-propagation-825/original/03.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/workspace_graph/expected_projection/cost/constructor.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-cost-propagation-825/original/04.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/workspace/semantic_graph/source_inventory.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-cost-propagation-825/original/05.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/workspace/semantic_graph.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-cost-propagation-825/original/06.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/CONSTRUCTOR-FAILURE-RECEIVING-V1.md"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-cost-propagation-825/original/07.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/workspace_graph/expected_projection.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-cost-propagation-825/original/08.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/workspace_graph/expected_projection/declaration_cost.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-cost-propagation-825/original/09.before"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/workspace_graph/expected_projection/borrowed_imports.rs"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/constructor-cost-propagation-825/original/10.before"
                )),
            ),
        ];
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), sources.len());
        for (row, (current, original)) in rows.iter().zip(sources) {
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
                ),
                row["current_sha256"].as_str().unwrap()
            );
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
                ),
                row["original_sha256"].as_str().unwrap()
            );
            let mut restored = current.to_owned();
            for edit in row["edits"].as_array().unwrap() {
                let new = edit["new"].as_str().unwrap();
                assert_eq!(restored.matches(new).count(), 1);
                restored = restored.replacen(new, edit["before"].as_str().unwrap(), 1);
            }
            assert_eq!(restored, original);
        }
        let live = sources[1].0.split("#[cfg(test)]").next().unwrap();
        for selected in [
            "pay_type(declaration, owner)?",
            "pay_type(entry.declaration, owner)?",
            "pay_signature(original, owner)?",
            "typed_cost::teardown_function(original, true, owner)?",
            "typed_cost::teardown_function(original, false, owner)?",
            "pay_default_teardown(&function.return_type, &type_index, owner)?",
        ] {
            assert!(live.contains(selected));
        }
        assert!(!live.contains("StructuralCost::"));
        let typed = sources[3].0.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "Vec<Diagnostic>",
            "reserve_builder_structure(",
            "ok_or_else(refusal)",
            "checked_usage(",
        ] {
            assert!(!typed.contains(forbidden));
        }
    }
}
