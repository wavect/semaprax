//! Aggregate-lane function-value table planning and indirect-call ABI.

use std::collections::{BTreeMap, HashMap};

use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, FunctionExecutionId, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedProgram, ResolvedType,
};

use super::{error, scalar_wasm_type, Signature, I32};

pub(super) struct TablePlan {
    pub(super) targets: Vec<ResolvedFunction>,
    pub(super) bodies: Vec<ResolvedFunction>,
    pub(super) captures: BTreeMap<DeclarationId, Vec<ResolvedType>>,
    pub(super) closure_profile: bool,
    pub(super) signatures: Vec<ResolvedType>,
}

/// The aggregate functions retain their checked `(args..., result-out) -> status`
/// ABI. Function carriers are table indices, and each `call_indirect` uses the
/// corresponding status ABI type rather than the core scalar return ABI.
pub(super) fn table_plan(program: &ResolvedProgram) -> Result<TablePlan, Diagnostic> {
    let mut signatures = BTreeMap::new();
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        crate::hir::function_value::walk(function, |expression| {
            if let ResolvedExprKind::Invoke { callable, .. } = &expression.kind {
                signatures.insert(callable.ty.identity_key(), callable.ty.clone());
            }
        });
    }
    let mut targets = crate::hir::function_value::target_universe(program)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let mut bodies = Vec::new();
    let mut captures = BTreeMap::new();
    for site in crate::hir::closure::inventory(program) {
        let ResolvedExprKind::Closure {
            captures: values, ..
        } = &site.kind
        else {
            unreachable!()
        };
        let body = crate::hir::closure::closure_function(program, site)?;
        captures.insert(
            body.id.clone(),
            values
                .iter()
                .map(|value| value.binding.ty.clone())
                .collect(),
        );
        let mut target = body.clone();
        target.params.drain(..values.len());
        targets.push(target);
        bodies.push(body);
    }
    targets.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(TablePlan {
        closure_profile: !bodies.is_empty(),
        targets,
        bodies,
        captures,
        signatures: signatures.into_values().collect(),
    })
}

pub(super) fn abi_signature(
    program: &ResolvedProgram,
    signature: &ResolvedType,
) -> Result<Signature, Diagnostic> {
    let ResolvedType::Function { parameters, .. } = signature else {
        return Err(error(
            "function invocation has a non-function callable type",
        ));
    };
    let mut params = parameters
        .iter()
        .map(|parameter| scalar_wasm_type(program, parameter))
        .collect::<Result<Vec<_>, _>>()?;
    params.push(I32);
    Ok(Signature {
        params,
        results: vec![I32],
    })
}

pub(super) fn type_indexes(
    program: &ResolvedProgram,
    signatures: &[ResolvedType],
    types: &mut Vec<Signature>,
    indexes: &mut HashMap<Signature, u32>,
    closure_profile: bool,
) -> Result<HashMap<String, u32>, Diagnostic> {
    let mut result = HashMap::new();
    for signature in signatures {
        let mut abi = abi_signature(program, signature)?;
        if closure_profile {
            abi.params.insert(0, I32);
        }
        let index = super::intern_type(abi, types, indexes);
        if result.insert(signature.identity_key(), index).is_some() {
            return Err(error("aggregate function invocation signature repeats"));
        }
    }
    Ok(result)
}

pub(super) fn table_indexes(
    targets: &[ResolvedFunction],
) -> Result<HashMap<DeclarationId, u32>, Diagnostic> {
    let mut indexes = HashMap::new();
    for (ordinal, target) in targets.iter().enumerate() {
        crate::hir::function_value::signature(target)
            .ok_or_else(|| error("aggregate function table contains an ineligible target"))?;
        let index =
            u32::try_from(ordinal).map_err(|_| error("aggregate function table exceeds u32"))?;
        if indexes.insert(target.id.clone(), index).is_some() {
            return Err(error("aggregate function table target repeats"));
        }
    }
    Ok(indexes)
}

pub(super) fn execution_target(target: &ResolvedFunction) -> FunctionExecutionId {
    FunctionExecutionId::Monomorphic(target.id.clone())
}

pub(super) fn callable_signature(expr: &ResolvedExpr) -> Result<&ResolvedType, Diagnostic> {
    match &expr.ty {
        ResolvedType::Function { .. } => Ok(&expr.ty),
        _ => Err(error(
            "aggregate function invocation callable is not a function",
        )),
    }
}

impl super::Emitter<'_> {
    pub(super) fn emit_function_reference(
        &mut self,
        expr: &ResolvedExpr,
        target: &DeclarationId,
    ) -> Result<super::Value, Diagnostic> {
        if self.closure_profile() {
            return self.emit_closure_reference(expr, target);
        }
        let local = self.plan.expr_scalar(expr)?;
        let table = *self
            .function_tables
            .get(target)
            .ok_or_else(|| error("function reference has no aggregate WebAssembly table slot"))?;
        self.output.push(0x41);
        super::write_i64(self.output, i64::from(table));
        self.output.push(0x21);
        super::write_u32(self.output, local);
        Ok(super::Value::Scalar {
            local,
            ty: expr.ty.clone(),
        })
    }

    pub(super) fn emit_function_invoke(
        &mut self,
        expr: &ResolvedExpr,
        callable: &ResolvedExpr,
        args: &[ResolvedExpr],
    ) -> Result<super::Value, Diagnostic> {
        crate::hir::function_value::validate_invocation(expr)?;
        let signature = callable_signature(callable)?;
        let ResolvedType::Function { parameters, result } = signature else {
            unreachable!()
        };
        if parameters.len() != args.len() || **result != expr.ty {
            return Err(error(
                "aggregate function invocation disagrees with its signature",
            ));
        }
        let callable_value = self.emit_expr(callable)?;
        if !self.closure_profile() {
            self.require_scalar(&callable_value, signature, "function invocation callable")?;
        }
        let scratch = *self
            .plan
            .function_callables
            .get(&expr.id)
            .ok_or_else(|| error("aggregate function invocation has no callable scratch"))?;
        if self.closure_profile() {
            let super::Value::Aggregate { pointer, .. } = &callable_value else {
                return Err(error("closure invocation carrier is not aggregate"));
            };
            self.emit_pointer(*pointer);
        } else {
            self.get_scalar(&callable_value);
        }
        self.output.push(0x21);
        super::write_u32(self.output, scratch);

        // Each argument snapshots into its dedicated local before the next
        // expression executes, preserving left-to-right value evaluation even
        // when a later argument mutates a binding read by an earlier one.
        let mut stages = Vec::with_capacity(args.len());
        for (argument, parameter) in args.iter().zip(parameters) {
            let value = self.emit_expr(argument)?;
            self.require_scalar(&value, parameter, "function invocation argument")?;
            let stage = *self
                .plan
                .function_arguments
                .get(&argument.id)
                .ok_or_else(|| error("aggregate function invocation has no argument scratch"))?;
            self.get_scalar(&value);
            self.output.push(0x21);
            super::write_u32(self.output, stage);
            stages.push(stage);
        }
        self.apply_call_commit(&expr.id)?;
        if self.closure_profile() {
            self.output.push(0x20);
            super::write_u32(self.output, scratch);
        }
        for stage in stages {
            self.output.push(0x20);
            super::write_u32(self.output, stage);
        }
        let offset = *self
            .plan
            .call_out
            .get(&expr.id)
            .ok_or_else(|| error("aggregate function invocation has no result slot"))?;
        let pointer = super::Pointer {
            local: self.plan.frame_base,
            offset,
        };
        self.emit_pointer(pointer);
        self.output.push(0x20);
        super::write_u32(self.output, scratch);
        if self.closure_profile() {
            self.output.extend([0x28, 0x02, 0x00]);
        }
        self.output.push(0x11);
        super::write_u32(
            self.output,
            *self
                .function_type_indexes
                .get(&signature.identity_key())
                .ok_or_else(|| {
                    error("aggregate function invocation has no WebAssembly signature type")
                })?,
        );
        self.output.push(0x00);
        self.output.push(0x22);
        super::write_u32(self.output, self.plan.status);
        self.output.extend([0x04, 0x40]);
        self.emit_failure_cleanup(&expr.id, crate::cleanup_plan::StatusLane::OperationFailure)?;
        self.output.push(0x0c);
        super::write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        let local = self.plan.expr_scalar(expr)?;
        self.emit_pointer(pointer);
        self.load_scalar(&expr.ty);
        self.output.push(0x21);
        super::write_u32(self.output, local);
        Ok(super::Value::Scalar {
            local,
            ty: expr.ty.clone(),
        })
    }
}

pub(super) fn program_uses_byte_range(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| {
            function.cleanup_plan.status_sources.iter().any(|source| {
                matches!(
                    &source.producer,
                    crate::cleanup_plan::StatusProducer::PropagatedCall { callee }
                        if callee.as_str() == crate::byte_ops::RANGE_ID
                )
            })
        })
}

pub(in crate::wasm) fn program_uses_owned_buffer(program: &ResolvedProgram) -> bool {
    super::executable_functions(program)
        .iter()
        .any(|(function, _)| {
            function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .any(|expression| {
                    let mut found = false;
                    crate::hir::visit_resolved_calls(expression, &mut |callee, instance, _| {
                        found |= instance.is_none()
                            && crate::byte_ops::by_id(callee.as_str())
                                .is_some_and(crate::byte_ops::ByteOp::is_owned_buffer_chain);
                    });
                    found
                })
        })
}

pub(super) fn hex_identity(id: &DeclarationId) -> String {
    let mut output = String::new();
    for byte in id.as_str().bytes() {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

pub(super) fn hex_execution_identity(id: &FunctionExecutionId) -> String {
    if let FunctionExecutionId::Monomorphic(declaration) = id {
        return hex_identity(declaration);
    }
    let mut output = String::new();
    for byte in id.identity_key().bytes() {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

pub(in crate::wasm) fn vec_import_base(program: &ResolvedProgram) -> u32 {
    super::SCALAR_IMPORT_COUNT
        + if super::super::program_uses_byte_data(program) {
            super::BYTE_IMPORT_COUNT
        } else {
            0
        }
        + if program_uses_owned_buffer(program) {
            super::OWNED_BUFFER_IMPORT_COUNT
        } else {
            0
        }
}

pub(in crate::wasm) fn box_import_base(program: &ResolvedProgram) -> u32 {
    vec_import_base(program)
        + if super::super::program_uses_vec(program) {
            super::VEC_IMPORT_COUNT
        } else {
            0
        }
        + if super::super::vec_ops::program_uses_extended_vec(program) {
            super::EXTENDED_VEC_IMPORT_COUNT
        } else {
            0
        }
        + if super::super::vec_ops::program_uses_record_vec(program) {
            super::RECORD_VEC_IMPORT_COUNT
        } else {
            0
        }
        + if crate::iterator_ops::resolved_program_uses_owned_iterator(program) {
            super::OWNED_ITER_IMPORT_COUNT
        } else {
            0
        }
}

pub(super) fn executable_functions(
    program: &ResolvedProgram,
) -> Vec<(&ResolvedFunction, FunctionExecutionId)> {
    program
        .functions
        .iter()
        .map(|function| {
            (
                function,
                FunctionExecutionId::Monomorphic(function.id.clone()),
            )
        })
        .chain(program.function_instances.iter().map(|instance| {
            (
                &instance.function,
                FunctionExecutionId::Generic(instance.id.clone()),
            )
        }))
        .collect()
}
