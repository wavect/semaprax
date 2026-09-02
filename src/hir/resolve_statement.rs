//! Assignment-target lowering and bounded-loop admission.
//!
//! Also owns the `resolve_expr` entry point that drives the iterative
//! expression resolver.

use std::collections::BTreeMap;

use crate::ast::{Expr, ExprKind, ParamMode, Span, Statement, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::is_scalar_source_type;

use super::expr_nodes::ResolvedExpr;
use super::ids::{DeclarationId, FunctionExecutionId};
use super::monomorphize::substitute_type;
use super::nodes::{is_scalar_resolved_type, DeclarationKind, ResolvedBinding, ResolvedType};
use super::{Binding, Resolver};

impl Resolver<'_> {
    pub(super) fn resolve_expr(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
    ) -> Result<ResolvedExpr, Diagnostic> {
        self.resolve_expr_iterative(function, expr, bindings, path)
    }

    /// Resolve one assignment target against the enclosing scope. The target
    /// must name an existing `let mut` local; parameters, contracts bindings,
    /// and immutable locals are rejected before the assigned value resolves.
    /// Resolve one assignment target against the enclosing scope. The target
    /// must name an existing `let mut` local; parameters, contracts bindings,
    /// and immutable locals are rejected before the assigned value resolves.
    /// Simple assignments report `SPX-U101`; Field Mutation v1 targets report
    /// `SPX-U107`.
    pub(super) fn resolve_assign_target(
        &self,
        name: &str,
        name_span: Span,
        bindings: &BTreeMap<String, Binding>,
        immutable_code: &'static str,
    ) -> Result<ResolvedBinding, Diagnostic> {
        let binding = bindings.get(name).ok_or_else(|| {
            self.error("SPX-H002", format!("unresolved value `{name}`"), name_span)
        })?;
        if !binding.mutable {
            return Err(self.error(
                immutable_code,
                format!("cannot assign to immutable binding `{name}`; declare it with `let mut`"),
                name_span,
            ));
        }
        Ok(ResolvedBinding {
            id: binding.id.clone(),
            name: name.to_owned(),
            ownership: binding.ownership,
            ty: binding.ty.clone(),
            span: name_span,
        })
    }

    /// Field Mutation v1: resolve the one direct `<binding>.<field>` level.
    /// The base must be a record/class-typed mutable local and the field must
    /// be a checked Copy scalar; everything else fails closed before the
    /// assigned value resolves.
    pub(super) fn resolve_assign_field_target(
        &self,
        binding: &ResolvedBinding,
        field: &crate::ast::FieldTarget,
    ) -> Result<(DeclarationId, ResolvedType), Diagnostic> {
        let ResolvedType::Nominal {
            declaration: owner,
            arguments,
        } = &binding.ty
        else {
            return Err(self.error(
                "SPX-U112",
                format!(
                    "cannot mutate a field of non-record value `{}`",
                    binding.ty.identity_key()
                ),
                field.span,
            ));
        };
        if self.declarations.declaration(owner).is_none_or(|item| {
            !matches!(item.kind, DeclarationKind::Record | DeclarationKind::Class)
        }) {
            return Err(self.error(
                "SPX-U112",
                format!(
                    "cannot mutate a field of non-record value `{}`",
                    binding.ty.identity_key()
                ),
                field.span,
            ));
        }
        let field_id = self
            .declarations
            .field_id(owner, &field.name)
            .cloned()
            .ok_or_else(|| {
                self.error(
                    "SPX-U108",
                    format!("record `{owner}` has no field `{}`", field.name),
                    field.span,
                )
            })?;
        let declared = self
            .declarations
            .record_fields(owner)
            .and_then(|fields| fields.iter().find(|item| item.id == field_id))
            .map(|item| item.ty.clone())
            .ok_or_else(|| {
                self.error(
                    "SPX-H001",
                    format!("field `{field_id}` has no resolved type"),
                    field.span,
                )
            })?;
        let field_ty = substitute_type(&declared, owner, arguments)?;
        if !is_scalar_resolved_type(&field_ty) {
            return Err(self.error(
                "SPX-U109",
                "field mutation v1 supports only direct scalar Copy record fields",
                field.span,
            ));
        }
        Ok((field_id, field_ty))
    }

    /// Bounded While-Loops v1 plus Indexed Byte Loop v2 admission profile: a
    /// loop condition or body may contain Copy-scalar operations — scalar
    /// literals, names, checked
    /// scalar arithmetic and comparisons, nested `if`s over scalars, blocks
    /// with scalar statements, scalar `let`/assignment statements, nested
    /// while loops, monomorphic calls to scalar-value functions, exact
    /// read-only `byte_len`/`byte_get`, and one guard-free direct
    /// `byte_get`/`Option<u8>` match. Every other construct (records, variants,
    /// general matches, `?`, projections, method
    /// calls, strings, unsafe boundaries, generic calls, non-scalar calls)
    /// is rejected fail-closed so loop cleanup stays edge-free.
    pub(super) fn reject_while_disallowed(&self, expression: &Expr) -> Result<(), Diagnostic> {
        enum Item<'a> {
            Expression(&'a Expr),
            Statement(&'a Statement),
        }

        let mut pending = vec![Item::Expression(expression)];
        while let Some(item) = pending.pop() {
            let expression = match item {
                Item::Statement(statement) => match statement {
                    Statement::Let { value, .. } | Statement::Assign { value, .. } => value,
                    Statement::Unsafe { span, .. } => {
                        return Err(self.error(
                            "SPX-T252",
                            "unsafe boundary statements are not yet admitted in while bodies",
                            *span,
                        ));
                    }
                    Statement::While {
                        condition, body, ..
                    } => {
                        pending.push(Item::Expression(body));
                        pending.push(Item::Expression(condition));
                        continue;
                    }
                },
                Item::Expression(expression) => expression,
            };

            match &expression.kind {
                ExprKind::Int(_)
                | ExprKind::Int32(_)
                | ExprKind::Char(_)
                | ExprKind::Uint8(_)
                | ExprKind::Usize(_)
                | ExprKind::Float32(_)
                | ExprKind::Float64(_)
                | ExprKind::Bool(_)
                | ExprKind::Var(_) => {}
                ExprKind::String(_) => {
                    return Err(self.error(
                        "SPX-T252",
                        "string literals are not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::ArrayU8(_) | ExprKind::RepeatArrayU8 { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "fixed-array literals are not admitted in bounded while bodies",
                        expression.span,
                    ));
                }
                ExprKind::Unary { value, .. } => pending.push(Item::Expression(value)),
                ExprKind::Binary { left, right, .. } => {
                    pending.push(Item::Expression(right));
                    pending.push(Item::Expression(left));
                }
                ExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    pending.push(Item::Expression(else_branch));
                    pending.push(Item::Expression(then_branch));
                    pending.push(Item::Expression(condition));
                }
                ExprKind::Block { statements, tail } => {
                    pending.push(Item::Expression(tail));
                    pending.extend(statements.iter().rev().map(Item::Statement));
                }
                ExprKind::Call {
                    type_arguments,
                    args,
                    name,
                    ..
                } => {
                    if !type_arguments.is_empty() {
                        return Err(self.error(
                            "SPX-T252",
                            "generic calls are not yet admitted in while bodies",
                            expression.span,
                        ));
                    }
                    if let Some(operation) = crate::byte_ops::by_name(name) {
                        if !matches!(
                            operation,
                            crate::byte_ops::ByteOp::Len
                                | crate::byte_ops::ByteOp::Get
                                | crate::byte_ops::ByteOp::Range
                        ) || args.len() != operation.arity()
                        {
                            return Err(self.error(
                                "SPX-T252",
                                format!(
                                    "byte operation `{name}` is not admitted in while bodies; only exact byte_len and byte_get reads qualify"
                                ),
                                expression.span,
                            ));
                        }
                    }
                    // Only calls that resolve to a monomorphic function with
                    // by-value scalar parameters and a scalar result keep the
                    // loop cleanup-edge-free; everything else is rejected
                    // before any argument in the same order as the recursive
                    // admission scan.
                    let declared = self
                        .program
                        .functions
                        .iter()
                        .find(|function| function.name == *name);
                    if let Some(declared) = declared {
                        let scalar_signature = declared.effects.is_empty()
                            && is_scalar_source_type(&declared.return_type)
                            && declared.params.iter().all(|param| {
                                (param.mode == ParamMode::Value && is_scalar_source_type(&param.ty))
                                    || (param.mode == ParamMode::Borrow
                                        && param.ty == Type::SliceU8)
                            });
                        if !scalar_signature {
                            return Err(self.error(
                                "SPX-T252",
                                format!(
                                    "call `{name}` is not admitted in while bodies; only scalar functions qualify"
                                ),
                                expression.span,
                            ));
                        }
                    }
                    pending.extend(args.iter().rev().map(Item::Expression));
                }
                ExprKind::MethodCall { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "method calls are not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::SuperMethod { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "super method calls are not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::Project { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "record field projection is not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::ConstructRecord { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "record construction is not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::ConstructVariant { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "variant construction is not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::UpdateRecord { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "record updates are not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::Match {
                    scrutinee, arms, ..
                } if crate::byte_ops::is_indexed_byte_option_match_source(expression) => {
                    pending.extend(arms.iter().rev().map(|arm| Item::Expression(&arm.value)));
                    pending.push(Item::Expression(scrutinee));
                }
                ExprKind::Match { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "match expressions are not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::Try { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "postfix `?` propagation is not yet admitted in while bodies",
                        expression.span,
                    ));
                }
            }
        }
        Ok(())
    }
}
