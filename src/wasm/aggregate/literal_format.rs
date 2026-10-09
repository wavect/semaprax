//! Backend-private, source-bound literal formatting for the two owned String
//! Wasm profiles. Dynamic children are evaluated and staged before rendering.
use super::*;

pub(super) fn import_type(
    program: &ResolvedProgram,
    types: &mut Vec<Signature>,
    indexes: &mut HashMap<Signature, u32>,
) -> Option<u32> {
    string_runtime::program_uses_format(program).then(|| {
        intern_type(
            Signature {
                params: vec![I32, I64, I64],
                results: vec![I64],
            },
            types,
            indexes,
        )
    })
}

pub(super) fn emit_import(output: &mut Vec<u8>, ty: Option<u32>) {
    if let Some(ty) = ty {
        function_import(output, "env", "spx_format_step_v1", ty);
    }
}

pub(super) fn insert_index(
    indexes: &mut HashMap<FunctionExecutionId, u32>,
    base: u32,
    program: &ResolvedProgram,
    toolkit_count: usize,
    ty: Option<u32>,
) {
    if ty.is_some() {
        indexes.insert(
            FunctionExecutionId::Monomorphic(DeclarationId::new(string_runtime::FORMAT_STEP_ID)),
            base + string_runtime::IMPORT_COUNT
                + u32::from(string_runtime::program_uses_ordering(program))
                + toolkit_count as u32,
        );
    }
}

/// The worker has independent failure cleanup after the canonical CallCommit.
/// Count every emitted guard's drops in the same bounded emission inventory.
pub(super) fn worker_cleanup_actions(function: &ResolvedFunction) -> Result<usize, Diagnostic> {
    let mut pending = std::iter::once(&function.body)
        .chain(&function.requires)
        .chain(&function.ensures)
        .collect::<Vec<_>>();
    let mut total = 0usize;
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::LiteralFormat { template, args } = &expression.kind {
            let pieces =
                crate::literal_format::scan(template).map_err(|reason| error(reason.message()))?;
            let owned = args
                .iter()
                .filter(|arg| arg.ty == ResolvedType::String)
                .count();
            // One initial-accumulator guard, then a piece and a join guard.
            let actions = pieces
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .and_then(|guards| guards.checked_mul(3 + owned))
                .ok_or_else(|| error("literal format cleanup work overflows"))?;
            total = total
                .checked_add(actions)
                .ok_or_else(|| error("literal format cleanup work overflows"))?;
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    Ok(total)
}

impl FunctionPlan {
    pub(super) fn collect_literal_format(
        &mut self,
        program: &ResolvedProgram,
        variant_layouts: &VariantLayoutCache,
        expression: &ResolvedExpr,
        args: &[ResolvedExpr],
        parameter_count: u32,
        frame: &mut FrameAllocator,
    ) -> Result<(), Diagnostic> {
        self.collect_exprs(program, variant_layouts, args, parameter_count, frame)?;
        let scalar_args = args
            .iter()
            .map(|argument| {
                if argument.ty == ResolvedType::String {
                    Ok(None)
                } else {
                    self.add_local(parameter_count, scalar_wasm_type(program, &argument.ty)?)
                        .map(Some)
                }
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        if self
            .literal_format_args
            .insert(expression.id.clone(), scalar_args)
            .is_some()
        {
            return Err(error("literal-format staged argument locals repeat"));
        }
        let scratch = [
            self.add_local(parameter_count, I64)?,
            self.add_local(parameter_count, I64)?,
            self.add_local(parameter_count, I64)?,
        ];
        if self
            .literal_format_scratch
            .insert(expression.id.clone(), scratch)
            .is_some()
        {
            return Err(error("literal-format scratch repeats"));
        }
        Ok(())
    }
}

impl Emitter<'_> {
    pub(super) fn emit_literal_format(
        &mut self,
        expression: &ResolvedExpr,
        template: &str,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        require_type(
            &expression.ty,
            &ResolvedType::String,
            "literal format result",
        )?;
        let pieces =
            crate::literal_format::scan(template).map_err(|failure| error(failure.message()))?;
        if crate::literal_format::field_count(&pieces) != args.len() {
            return Err(error("literal format field count changed after validation"));
        }
        let mut staged = Vec::with_capacity(args.len());
        let scalar_snapshots = self
            .plan
            .literal_format_args
            .get(&expression.id)
            .ok_or_else(|| error("literal format argument snapshots are missing"))?
            .clone();
        for (index, argument) in args.iter().enumerate() {
            if !crate::literal_format::accepts_hir_type(&argument.ty) {
                return Err(error(
                    "literal format argument type changed after validation",
                ));
            }
            let value = self.emit_expr(argument)?;
            require_type(value_type(&value), &argument.ty, "literal format argument")?;
            let local = if argument.ty == ResolvedType::String {
                let epoch = crate::cleanup_plan::StorageId::CallArgument {
                    call: expression.id.clone(),
                    parameter_index: u32::try_from(index)
                        .map_err(|_| error("literal format argument index overflows u32"))?,
                    value_expression: argument.id.clone(),
                };
                self.plan
                    .cleanup_call_argument_carriers
                    .get(&epoch)
                    .copied()
                    .ok_or_else(|| error("literal format owned argument has no canonical epoch"))?
            } else {
                match value {
                    Value::Scalar { local, .. } => {
                        let snapshot = scalar_snapshots[index]
                            .ok_or_else(|| error("literal format scalar snapshot is missing"))?;
                        self.output.push(0x20);
                        write_u32(self.output, local);
                        self.output.push(0x21);
                        write_u32(self.output, snapshot);
                        snapshot
                    }
                    _ => return Err(error("literal format scalar argument lacks a local")),
                }
            };
            staged.push(local);
        }
        let mut commits = self
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .filter_map(|transition| match transition {
                crate::cleanup_plan::CleanupTransition::CallCommit { call, arguments }
                    if call == &expression.id =>
                {
                    Some(arguments)
                }
                _ => None,
            });
        let committed = commits
            .next()
            .ok_or_else(|| error("literal format lacks canonical CallCommit"))?;
        if commits.next().is_some() {
            return Err(error("literal format has duplicate CallCommit"));
        }
        let expected = args
            .iter()
            .enumerate()
            .filter(|(_, argument)| argument.ty == ResolvedType::String)
            .collect::<Vec<_>>();
        if committed.len() != expected.len() {
            return Err(error("literal format CallCommit ownership disagrees"));
        }
        for (transfer, (index, argument)) in committed.iter().zip(expected) {
            if transfer.parameter_index != index as u32
                || !transfer.source.projections.is_empty()
                || !matches!(&transfer.source.storage,
                    crate::cleanup_plan::StorageId::CallArgument { call, parameter_index, value_expression }
                    if call == &expression.id && *parameter_index == index as u32 && value_expression == &argument.id)
            {
                return Err(error(
                    "literal format CallCommit is not source-authenticated",
                ));
            }
        }
        self.apply_call_commit(&expression.id)?;
        let previous_failure = self.failure_expression.replace(expression.id.clone());
        let scratch = *self
            .plan
            .literal_format_scratch
            .get(&expression.id)
            .ok_or_else(|| error("literal format scratch is missing"))?;
        let [accumulator, part, next] = scratch;
        let destination = self.plan.expr_scalar(expression)?;
        for local in [accumulator, part, next] {
            owned_strings::emit_clear(self.output, local);
        }
        self.format_literal_into("", accumulator)?;
        self.format_guard(accumulator, &staged, args, scratch)?;
        let mut field = 0;
        for piece in pieces {
            match piece {
                crate::literal_format::Piece::Literal(value) => {
                    self.format_literal_into(&value, part)?;
                }
                crate::literal_format::Piece::Field => {
                    let argument = &args[field];
                    let source = staged[field];
                    match argument.ty {
                        ResolvedType::String => {
                            self.output.push(0x20);
                            write_u32(self.output, source);
                            self.output.push(0x21);
                            write_u32(self.output, part);
                            owned_strings::emit_clear(self.output, source);
                        }
                        ResolvedType::I64 | ResolvedType::U8 | ResolvedType::Usize => {
                            if !self.standalone_strings {
                                self.output.push(0x41); // i32.const operation
                                write_i64(
                                    self.output,
                                    if argument.ty == ResolvedType::Usize {
                                        3
                                    } else {
                                        2
                                    },
                                );
                            }
                            self.output.push(0x20);
                            write_u32(self.output, source);
                            if argument.ty == ResolvedType::U8 {
                                self.output.push(0xad); // i64.extend_i32_u
                            }
                            let operation = if argument.ty == ResolvedType::Usize {
                                crate::string_ops::StringOp::FromUsize
                            } else {
                                crate::string_ops::StringOp::FromI64
                            };
                            if !self.standalone_strings {
                                self.output.push(0x42); // i64.const unused operand
                                write_i64(self.output, 0);
                            }
                            let index = if self.standalone_strings {
                                self.function_indexes
                                    .get(&FunctionExecutionId::Monomorphic(DeclarationId::new(
                                        operation.id(),
                                    )))
                                    .copied()
                                    .ok_or_else(|| {
                                        error("literal format numeric runtime is not indexed")
                                    })?
                            } else {
                                self.format_step_index()?
                            };
                            self.output.push(0x10);
                            write_u32(self.output, index);
                            self.output.push(0x21);
                            write_u32(self.output, part);
                        }
                        ResolvedType::Bool => {
                            self.output.push(0x20);
                            write_u32(self.output, source);
                            self.output.extend([0x04, 0x40]); // if
                            self.format_literal_into("true", part)?;
                            self.output.push(0x05); // else
                            self.format_literal_into("false", part)?;
                            self.output.push(0x0b);
                        }
                        _ => {
                            return Err(error(
                                "literal format argument type is outside the closed set",
                            ))
                        }
                    }
                    field += 1;
                }
            }
            self.format_guard(part, &staged, args, scratch)?;
            if !self.standalone_strings {
                self.output.push(0x41); // i32.const join operation
                write_i64(self.output, 1);
            }
            self.output.push(0x20);
            write_u32(self.output, accumulator);
            self.output.push(0x20);
            write_u32(self.output, part);
            let concat = if self.standalone_strings {
                2
            } else {
                self.format_step_index()?
            };
            self.output.push(0x10);
            write_u32(self.output, concat);
            self.output.push(0x21);
            write_u32(self.output, next);
            self.format_guard(next, &staged, args, scratch)?;
            owned_strings::emit_drop(self.output, accumulator);
            owned_strings::emit_drop(self.output, part);
            self.output.push(0x20);
            write_u32(self.output, next);
            self.output.push(0x21);
            write_u32(self.output, accumulator);
            owned_strings::emit_clear(self.output, next);
        }
        self.output.push(0x20);
        write_u32(self.output, accumulator);
        self.output.push(0x21);
        write_u32(self.output, destination);
        owned_strings::emit_clear(self.output, accumulator);
        self.failure_expression = previous_failure;
        Ok(Value::Scalar {
            local: destination,
            ty: ResolvedType::String,
        })
    }

    fn format_literal_into(&mut self, text: &str, destination: u32) -> Result<(), Diagnostic> {
        let literals = self
            .owned_utf8_literals
            .as_deref_mut()
            .ok_or_else(|| error("literal format requires an owned UTF-8 runtime"))?;
        let (offset, length) = literals.intern(text)?;
        if self.standalone_strings {
            self.output.push(0x41);
            write_i64(self.output, i64::from(offset));
            self.output.push(0x41);
            write_i64(self.output, i64::from(length));
            self.output.push(0x10);
            write_u32(self.output, internal_strings::LITERAL_IMPORT);
        } else {
            let carrier = (u64::from(offset) << 32) | u64::from(length);
            self.output.push(0x41); // i32.const copy-literal operation
            write_i64(self.output, 0);
            self.output.push(0x42);
            write_i64(self.output, carrier as i64);
            self.output.push(0x42);
            write_i64(self.output, 0);
            let step = self.format_step_index()?;
            self.output.push(0x10);
            write_u32(self.output, step);
        }
        self.output.push(0x21);
        write_u32(self.output, destination);
        Ok(())
    }

    fn format_step_index(&self) -> Result<u32, Diagnostic> {
        self.function_indexes
            .get(&FunctionExecutionId::Monomorphic(DeclarationId::new(
                string_runtime::FORMAT_STEP_ID,
            )))
            .copied()
            .ok_or_else(|| error("literal format private host step is not indexed"))
    }

    fn format_guard(
        &mut self,
        local: u32,
        staged: &[u32],
        args: &[ResolvedExpr],
        scratch: [u32; 3],
    ) -> Result<(), Diagnostic> {
        self.output.push(0x20);
        write_u32(self.output, local);
        self.output.push(0x50); // i64.eqz
        self.output.extend([0x04, 0x40]); // if
        for owner in scratch {
            owned_strings::emit_drop(self.output, owner);
        }
        for (index, argument) in args.iter().enumerate() {
            if argument.ty == ResolvedType::String {
                owned_strings::emit_drop(self.output, staged[index]);
            }
        }
        self.output.push(0x41);
        write_i64(self.output, 1);
        // The private host String helpers report allocation failure as zero.
        // Cleanup above is complete before the ordinary sticky status exit.
        self.control_depth += 1;
        let failed = self.fail_if(STATUS_STRING_FORMAT_FAILURE);
        self.control_depth -= 1;
        failed?;
        self.output.push(0x0b);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_format_worker_cleanup_census_counts_all_failure_guards() {
        let ast = crate::check(
            r#"module format.cleanup_work;
@id("render") fn render(a:own string,b:own string)->string {string_format("{}-{}",a,b)}
@id("main") fn main()->i64 {0}
"#,
            "format-cleanup-work.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&ast).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "render")
            .unwrap();
        // Three pieces, two guards per piece + initial accumulator; every
        // guard settles three scratch handles and two staged owned arguments.
        assert_eq!(
            worker_cleanup_actions(function).unwrap(),
            (2 * 3 + 1) * (3 + 2)
        );
        assert_eq!(
            worker_cleanup_actions(
                program
                    .functions
                    .iter()
                    .find(|f| f.id.as_str() == "main")
                    .unwrap()
            )
            .unwrap(),
            0
        );
    }
}
