//! Selected checked text imports. Output is published only after status zero.
use super::*;
use crate::string_ops::StringOp;

const OPERATIONS: [StringOp; 7] = [
    StringOp::Slice,
    StringOp::Find,
    StringOp::ToI64,
    StringOp::Trim,
    StringOp::ByteAt,
    StringOp::FromStr,
    StringOp::FileReadText,
];

pub(super) fn admitted(operation: StringOp) -> bool {
    OPERATIONS.contains(&operation)
}

pub(in crate::wasm) fn selected(program: &ResolvedProgram) -> Vec<StringOp> {
    let mut found = BTreeSet::new();
    let mut pending = Vec::new();
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expr) = pending.pop() {
        if let ResolvedExprKind::Call { callee, .. } = &expr.kind {
            if let Some(index) = OPERATIONS.iter().position(|op| op.id() == callee.as_str()) {
                found.insert(index);
            }
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expr, &mut pending);
    }
    found.into_iter().map(|index| OPERATIONS[index]).collect()
}

pub(in crate::wasm) fn program_uses_toolkit(program: &ResolvedProgram) -> bool {
    !selected(program).is_empty()
}

pub(in crate::wasm) fn uses_byte_get(program: &ResolvedProgram) -> bool {
    if selected(program).contains(&StringOp::FileReadText) {
        return true;
    }
    let mut pending = Vec::new();
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if matches!(
            &expression.kind,
            ResolvedExprKind::Call { callee, .. }
                if callee.as_str() == crate::byte_ops::GET_ID
        ) {
            return true;
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}

pub(super) fn import_types(
    program: &ResolvedProgram,
    types: &mut Vec<Signature>,
    indexes: &mut HashMap<Signature, u32>,
) -> Vec<(StringOp, u32)> {
    selected(program)
        .into_iter()
        .map(|op| {
            let mut params = vec![I64; op.arity()];
            params.push(I32);
            let ty = intern_type(
                Signature {
                    params,
                    results: vec![I32],
                },
                types,
                indexes,
            );
            (op, ty)
        })
        .collect()
}

pub(in crate::wasm) fn import_name(operation: StringOp) -> &'static str {
    match operation {
        StringOp::Slice => "spx_string_slice_v2",
        StringOp::Find => "spx_string_find_v2",
        StringOp::ToI64 => "spx_string_to_i64_v2",
        StringOp::Trim => "spx_string_trim_v2",
        StringOp::ByteAt => "spx_string_byte_at_v2",
        StringOp::FromStr => "spx_string_from_str_v2",
        StringOp::FileReadText => "spx_file_read_text_v2",
        _ => unreachable!("selected toolkit operation"),
    }
}

impl Emitter<'_> {
    pub(super) fn emit_checked_text_operation(
        &mut self,
        expression: &ResolvedExpr,
        operation: StringOp,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        if !admitted(operation) || args.len() != operation.arity() {
            return Err(error("checked text operation arity/profile mismatch"));
        }
        require_type(
            &expression.ty,
            &operation.return_type(),
            "checked text result",
        )?;
        let mut values = Vec::new();
        for (argument, ty) in args.iter().zip(operation.param_types()) {
            let value = self.emit_expr(argument)?;
            require_type(value_type(&value), ty, "checked text argument")?;
            values.push(value);
        }
        self.apply_call_commit(&expression.id)?;
        let pointer = if operation == StringOp::ToI64 {
            self.plan.expr_pointer(expression)?
        } else {
            Pointer {
                local: self.plan.frame_base,
                offset: *self
                    .plan
                    .call_out
                    .get(&expression.id)
                    .ok_or_else(|| error("checked text output slot missing"))?,
            }
        };
        let runtime = *self
            .function_indexes
            .get(&FunctionExecutionId::Monomorphic(DeclarationId::new(
                operation.id(),
            )))
            .ok_or_else(|| error("checked text import missing"))?;
        for value in &values {
            self.get_scalar(value);
        }
        self.emit_pointer(pointer);
        self.output.push(0x10);
        write_u32(self.output, runtime);
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        // Authenticate the host status before using it as proof-selected failure.
        let statuses: &[i32] = match operation {
            StringOp::Slice => &[0, 23, 24],
            StringOp::Find | StringOp::ByteAt => &[0, 23],
            StringOp::FromStr => &[0, 25],
            StringOp::FileReadText => &[0, 25, 65, 66, 67, 68, 69, 70, 71],
            _ => &[0],
        };
        for (index, status) in statuses.iter().enumerate() {
            self.output.push(0x20);
            write_u32(self.output, self.plan.status);
            self.output.push(0x41);
            write_i64(self.output, i64::from(*status));
            self.output.push(0x47); // i32.ne
            if index != 0 {
                self.output.push(0x71);
            }
        }
        self.trap_if();
        if statuses.len() > 1 {
            self.output.push(0x20);
            write_u32(self.output, self.plan.status);
            self.output.extend([0x04, 0x40]);
            self.emit_failure_cleanup(&expression.id, StatusLane::OperationFailure)?;
            self.output.push(0x0c);
            write_u32(
                self.output,
                self.control_depth + self.status_exit_extra_depth,
            );
            self.output.push(0x0b);
        }
        if operation == StringOp::ToI64 {
            let layout = variant_layout(self.variant_layouts, &expression.ty)?;
            let some_id = self
                .program
                .declarations
                .case_id(&layout.variant, "Some")
                .ok_or_else(|| error("Option Some identity missing"))?;
            let none_id = self
                .program
                .declarations
                .case_id(&layout.variant, "None")
                .ok_or_else(|| error("Option None identity missing"))?;
            let some = layout
                .cases
                .iter()
                .find(|case| &case.case == some_id)
                .ok_or_else(|| error("Option Some layout missing"))?;
            let none = layout
                .cases
                .iter()
                .find(|case| &case.case == none_id)
                .ok_or_else(|| error("Option None layout missing"))?;
            if layout.size != 16
                || layout.payload_offset != 8
                || some.fields.len() != 1
                || some.fields[0].ty != ResolvedType::I64
                || some.fields[0].offset != 0
                || !none.fields.is_empty()
            {
                return Err(error("text parse Option packet layout mismatch"));
            }
            self.emit_pointer(pointer);
            self.output.extend([0x28, 0x02, 0x00, 0x41, 0x01, 0x4b]);
            self.trap_if();
            self.emit_pointer(pointer);
            self.emit_pointer(pointer);
            self.output.extend([0x28, 0x02, 0x00, 0x04, I32, 0x41]);
            write_i64(self.output, i64::from(some.tag));
            self.output.extend([0x05, 0x41]);
            write_i64(self.output, i64::from(none.tag));
            self.output.extend([0x0b, 0x36, 0x02, 0x00]);
            Ok(Value::Aggregate {
                pointer,
                ty: expression.ty.clone(),
            })
        } else {
            self.emit_pointer(pointer);
            self.load_scalar(&expression.ty);
            let local = self.plan.expr_scalar(expression)?;
            self.output.push(0x21);
            write_u32(self.output, local);
            if self.standalone_strings && expression.ty == ResolvedType::String {
                self.string_capacity_guard(local)?;
            }
            Ok(Value::Scalar {
                local,
                ty: expression.ty.clone(),
            })
        }
    }
}
