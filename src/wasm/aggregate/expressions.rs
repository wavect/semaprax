//! Small recursive dispatch and block lowering. Keep inactive aggregate-arm
//! temporaries off the host stack while recursively emitting scalar bodies.

use super::*;

impl Emitter<'_> {
    pub(super) fn emit_expr_inner(&mut self, expr: &ResolvedExpr) -> Result<Value, Diagnostic> {
        match &expr.kind {
            ResolvedExprKind::Block { statements, tail } => self.emit_block(expr, statements, tail),
            ResolvedExprKind::Unary { op, value } => self.emit_unary(expr, *op, value),
            ResolvedExprKind::Binary { op, left, right } => {
                self.emit_binary(expr, *op, left, right)
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.emit_if(expr, condition, then_branch, else_branch),
            ResolvedExprKind::Call {
                callee,
                instance,
                type_arguments,
                args,
            } => self.emit_call(expr, callee, instance.as_ref(), type_arguments, args),
            ResolvedExprKind::Match {
                mode: crate::hir::ResolvedMatchMode::Value,
                scrutinee,
                arms,
            } if crate::hir::is_refutable_match_scalar(&scrutinee.ty) => {
                if is_aggregate(self.program, &expr.ty)? {
                    return Err(error("copy match result must be i64 or bool"));
                }
                let scrutinee = self.emit_expr(scrutinee)?;
                self.emit_scalar_refutable_match(expr, &scrutinee, arms)
            }
            _ => self.emit_complex_expr(expr),
        }
    }

    fn emit_block(
        &mut self,
        expr: &ResolvedExpr,
        statements: &[ResolvedStatement],
        tail: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let saved = self.bindings.clone();
        for statement in statements {
            self.emit_block_statement(statement)?;
        }
        let tail = self.emit_expr(tail)?;
        let result = self.materialize(expr, &tail)?;
        self.emit_block_scope_cleanup(expr)?;
        self.bindings = saved;
        Ok(result)
    }

    fn emit_block_statement(&mut self, statement: &ResolvedStatement) -> Result<(), Diagnostic> {
        if self.owned_utf8_literals.is_some()
            && matches!(statement, ResolvedStatement::Unsafe { body, .. } if body.ty == ResolvedType::String)
        {
            return Err(error(
                "discarding an owned string has no admitted WebAssembly lowering",
            ));
        }
        if self.owned_utf8_literals.is_some()
            && matches!(statement, ResolvedStatement::Assign { binding, value, .. }
                if binding.ty == ResolvedType::String
                    && !crate::string_ops::replacement::admitted(binding, value))
        {
            return Err(error(
                "string assignment has no admitted WebAssembly lowering",
            ));
        }
        // Field Mutation v1: the assigned value evaluates fully first, then
        // stores into the direct scalar field of the aggregate frame slot.
        if let ResolvedStatement::Assign {
            binding,
            field: Some(field_id),
            ..
        } = statement
        {
            let value = self.emit_expr(statement.value())?;
            let offset = self
                .plan
                .aggregate_bindings
                .get(&binding.id)
                .copied()
                .ok_or_else(|| error(format!("missing aggregate binding `{}`", binding.id)))?;
            let record_layout = layout(self.program, &binding.ty)?;
            let field = record_layout.field(field_id).cloned().ok_or_else(|| {
                error(format!(
                    "record `{}` has no assignment field `{field_id}`",
                    record_layout.record
                ))
            })?;
            let destination = value_at(
                Pointer {
                    local: self.plan.frame_base,
                    offset: offset
                        .checked_add(field.offset)
                        .ok_or_else(|| error("field pointer overflows u32"))?,
                },
                field.ty,
                self.program,
            )?;
            self.copy_value(&destination, &value, "field assignment")?;
            return Ok(());
        }
        // Lets declare and store; assignments re-store into the same slot.
        // Unsafe boundaries emit their body transparently and bind nothing.
        let (ResolvedStatement::Let { binding, .. } | ResolvedStatement::Assign { binding, .. }) =
            statement
        else {
            if let ResolvedStatement::While {
                condition, body, ..
            } = statement
            {
                self.emit_while(condition, body)?;
            } else {
                self.emit_expr(statement.value())?;
            }
            return Ok(());
        };
        let value = self.emit_expr(statement.value())?;
        let destination = if is_aggregate(self.program, &binding.ty)? {
            let offset = self
                .plan
                .aggregate_bindings
                .get(&binding.id)
                .copied()
                .ok_or_else(|| error(format!("missing aggregate binding `{}`", binding.id)))?;
            Value::Aggregate {
                pointer: Pointer {
                    local: self.plan.frame_base,
                    offset,
                },
                ty: binding.ty.clone(),
            }
        } else {
            let local = self
                .plan
                .scalar_bindings
                .get(&binding.id)
                .copied()
                .ok_or_else(|| error(format!("missing scalar binding `{}`", binding.id)))?;
            Value::Scalar {
                local,
                ty: binding.ty.clone(),
            }
        };
        self.copy_value(&destination, &value, "local binding")?;
        self.bindings.insert(binding.id.clone(), destination);
        Ok(())
    }
}

/// Whether an aggregate expression may select the existing try status lane.
pub(super) fn expression_has_try(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => true,
        ResolvedExprKind::Call { args, .. } => args.iter().any(expression_has_try),
        ResolvedExprKind::Invoke { callable, args } => {
            expression_has_try(callable) || args.iter().any(expression_has_try)
        }
        ResolvedExprKind::NativeRustImportCall(call) => call.args.iter().any(expression_has_try),
        ResolvedExprKind::HostCommandCall(call) => call.args.iter().any(expression_has_try),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => expression_has_try(source) || expression_has_try(start) || expression_has_try(end),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => expression_has_try(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            expression_has_try(left) || expression_has_try(right)
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| {
                (0..statement.child_count()).any(|index| {
                    expression_has_try(
                        statement
                            .child(index)
                            .expect("resolved statement child count is canonical"),
                    )
                })
            }) || expression_has_try(tail)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expression_has_try(condition)
                || expression_has_try(then_branch)
                || expression_has_try(else_branch)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.iter().any(|field| expression_has_try(&field.value))
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => expression_has_try(scrutinee) || arms.iter().any(|arm| expression_has_try(&arm.value)),
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            expression_has_try(base) || fields.iter().any(|field| expression_has_try(&field.value))
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. }
        | ResolvedExprKind::FunctionReference { .. } => false,
        ResolvedExprKind::Closure { captures, .. } => captures
            .iter()
            .any(|capture| expression_has_try(&capture.value)),
    }
}

pub(super) fn emit_arithmetic_trap_case(
    body: &mut Vec<u8>,
    status_local: u32,
    expected: i32,
    import: u32,
    left: i64,
    right: i64,
) {
    body.push(0x20);
    write_u32(body, status_local);
    body.push(0x41);
    write_i64(body, i64::from(expected));
    body.push(0x46);
    body.extend([0x04, 0x40, 0x42]);
    write_i64(body, left);
    body.push(0x42);
    write_i64(body, right);
    body.push(0x10);
    write_u32(body, import);
    body.extend([0x1a, 0x00, 0x0b]);
}

impl Emitter<'_> {
    /// Bounded While-Loops v1 lowers to a core `block`/`loop` pair: the
    /// condition re-evaluates at the top, a false condition branches out of
    /// the enclosing block, and the discarded body value falls through to the
    /// back-edge branch. Checked-arithmetic failures inside the loop keep the
    /// same sticky host-status contract as straight-line code.
    pub(super) fn emit_while(
        &mut self,
        condition: &ResolvedExpr,
        body: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        if self.owned_utf8_literals.is_some() && body.ty == ResolvedType::String {
            return Err(error(
                "discarding an owned string has no admitted WebAssembly lowering",
            ));
        }
        self.output.extend([0x02, 0x40]); // block (empty) $exit
        self.output.extend([0x03, 0x40]); // loop (empty) $top
        self.control_depth += 2;
        let condition_value = self.emit_expr(condition)?;
        self.require_scalar(&condition_value, &ResolvedType::Bool, "while condition")?;
        self.emit_scalar_match_guard_cleanup(condition, condition)?;
        self.get_scalar(&condition_value);
        self.output.push(0x45); // i32.eqz
        self.output.extend([0x0d, 0x01]); // br_if 1 -> $exit on false
        self.semantic_charge()?; // stage semantic work v1: metered builds only
        let _body_value = self.emit_expr(body)?;
        self.control_depth -= 2;
        self.output.extend([0x0c, 0x00]); // br 0 -> $top
        self.output.push(0x0b); // end loop
        self.output.push(0x0b); // end block
        Ok(())
    }
}

/// A block's direct storage: its `let` bindings and statement values. These
/// always live in the block's own region and break ties when nested operand
/// anchors also touch other regions.
pub(super) fn block_direct_anchors(
    slots: &HashMap<crate::cleanup_plan::StorageId, ResolvedType>,
    expression: &ResolvedExpr,
) -> std::collections::BTreeSet<crate::cleanup_plan::StorageId> {
    use crate::cleanup_plan::StorageId;
    let ResolvedExprKind::Block { statements, tail } = &expression.kind else {
        return std::collections::BTreeSet::new();
    };
    let mut anchors = std::collections::BTreeSet::new();
    // The block's own result slot belongs to its region too.
    let result = StorageId::Temporary(tail.id.clone());
    if slots.contains_key(&result) {
        anchors.insert(result);
    }
    for statement in statements {
        if let ResolvedStatement::Let { binding, .. } = statement {
            let storage = StorageId::Value(binding.id.clone());
            if slots.contains_key(&storage) {
                anchors.insert(storage);
            }
        }
        if let ResolvedStatement::Let { value, .. } | ResolvedStatement::Assign { value, .. } =
            statement
        {
            let storage = StorageId::Temporary(value.id.clone());
            if slots.contains_key(&storage) {
                anchors.insert(storage);
            }
        }
    }
    anchors
}

/// Slots of this exact lexical Block region, including scalar operand owners.
/// Nested Blocks own their children; direct HIR If arms share their parent.
pub(super) fn block_scope_anchors(
    slots: &HashMap<crate::cleanup_plan::StorageId, ResolvedType>,
    expression: &ResolvedExpr,
) -> std::collections::BTreeSet<crate::cleanup_plan::StorageId> {
    use crate::cleanup_plan::StorageId;
    let ResolvedExprKind::Block { statements, tail } = &expression.kind else {
        return std::collections::BTreeSet::new();
    };
    let mut anchors = std::collections::BTreeSet::new();
    let mut pending = vec![tail.as_ref()];
    for statement in statements {
        if let ResolvedStatement::Let { binding, .. } = statement {
            let storage = StorageId::Value(binding.id.clone());
            if slots.contains_key(&storage) {
                anchors.insert(storage);
            }
        }
        match statement {
            ResolvedStatement::Let { value, .. } | ResolvedStatement::Assign { value, .. } => {
                pending.push(value)
            }
            ResolvedStatement::Unsafe { body, .. } => pending.push(body),
            ResolvedStatement::While { .. } => {}
        }
    }
    while let Some(expression) = pending.pop() {
        let storage = StorageId::Temporary(expression.id.clone());
        if slots.contains_key(&storage) {
            anchors.insert(storage);
        }
        match &expression.kind {
            ResolvedExprKind::Block { .. } | ResolvedExprKind::Closure { .. } => {}
            ResolvedExprKind::Match { scrutinee, .. } => pending.push(scrutinee),
            _ => pending.extend(crate::interpreter::trace_child_expressions(expression)),
        }
    }
    anchors
}
