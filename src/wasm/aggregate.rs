//! Deterministic Wasm32 lowering for executable aggregate records v1.
//!
//! This is deliberately isolated from the scalar encoder so existing scalar,
//! owned-resource, callable, and Component byte contracts remain unchanged.

use std::collections::HashMap;

use crate::aggregate_layout::{AggregateLayout, AggregateLayoutCache, AggregateTarget};
use crate::ast::{BinaryOp, UnaryOp};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ExpressionId, FunctionExecutionId, PlaceProjection, ResolvedExpr,
    ResolvedExprKind, ResolvedFunction, ResolvedProgram, ResolvedStatement, ResolvedType,
    ResolvedTypeDeclarationKind, ValueId,
};
use crate::variant_layout::{VariantLayout, VariantLayoutCache, VariantTarget};

use super::{
    function_import, intern_type, section, write_bytes, write_i64, write_name, write_u32,
    Signature, F32, F64, I32, I64, SCALAR_IMPORT_COUNT,
};

const BYTE_IMPORT_COUNT: u32 = 4;
const BYTE_COPY_IMPORT: u32 = SCALAR_IMPORT_COUNT;
const BYTE_GET_IMPORT: u32 = SCALAR_IMPORT_COUNT + 1;
const BYTE_DROP_IMPORT: u32 = SCALAR_IMPORT_COUNT + 2;
const BYTE_AS_SLICE_IMPORT: u32 = SCALAR_IMPORT_COUNT + 3;

pub(super) const SHADOW_STACK_TOP: u32 = 65_536;
pub(super) const STATUS_SUCCESS: i32 = 0;
pub(super) const STATUS_ADD_OVERFLOW: i32 = 1;
pub(super) const STATUS_SUB_OVERFLOW: i32 = 2;
pub(super) const STATUS_MUL_OVERFLOW: i32 = 3;
pub(super) const STATUS_DIV_ZERO: i32 = 4;
pub(super) const STATUS_DIV_OVERFLOW: i32 = 5;
pub(super) const STATUS_REM_ZERO: i32 = 6;
pub(super) const STATUS_REM_OVERFLOW: i32 = 7;
pub(super) const STATUS_NEG_OVERFLOW: i32 = 8;
pub(super) const STATUS_REQUIRES_FALSE: i32 = 9;
pub(super) const STATUS_ENSURES_FALSE: i32 = 10;
pub(super) const STATUS_INTERNAL_INVALID_TAG: i32 = -1;

#[cfg(any(test, feature = "unstable-wit-component-harness"))]
pub(super) struct SelectedAggregateLowering {
    pub(super) types: Vec<Signature>,
    pub(super) function_type_indexes: Vec<u32>,
    pub(super) bodies: Vec<Vec<u8>>,
    pub(super) selected_index: u32,
}

#[derive(Clone, Copy)]
struct Pointer {
    local: u32,
    offset: u32,
}

#[derive(Clone)]
enum Value {
    Scalar { local: u32, ty: ResolvedType },
    ScalarMemory { pointer: Pointer, ty: ResolvedType },
    Aggregate { pointer: Pointer, ty: ResolvedType },
}

#[derive(Default)]
struct FrameAllocator {
    size: u32,
    align: u32,
}

impl FrameAllocator {
    fn allocate(&mut self, size: u32, align: u32) -> Result<u32, Diagnostic> {
        if !align.is_power_of_two() {
            return Err(error("aggregate frame alignment is not a power of two"));
        }
        let mask = align - 1;
        let offset = self
            .size
            .checked_add(mask)
            .map(|value| value & !mask)
            .ok_or_else(|| error("aggregate frame alignment overflows u32"))?;
        self.size = offset
            .checked_add(size)
            .ok_or_else(|| error("aggregate frame size overflows u32"))?;
        self.align = self.align.max(align);
        Ok(offset)
    }

    fn finish(&self) -> Result<u32, Diagnostic> {
        let align = self.align.max(8);
        let mask = align - 1;
        self.size
            .checked_add(mask)
            .map(|value| value & !mask)
            .ok_or_else(|| error("aggregate frame rounding overflows u32"))
    }
}

struct FunctionPlan {
    local_types: Vec<u8>,
    old_stack: u32,
    frame_base: u32,
    status: u32,
    command_byte: Option<u32>,
    external_root_bytes: Option<u32>,
    result_staged: Option<u32>,
    has_try: bool,
    result_out: u32,
    result_stage_scalar: Option<u32>,
    result_stage_aggregate: Option<u32>,
    scalar_expressions: HashMap<ExpressionId, u32>,
    scalar_bindings: HashMap<ValueId, u32>,
    aggregate_expressions: HashMap<ExpressionId, u32>,
    aggregate_bindings: HashMap<ValueId, u32>,
    call_out: HashMap<ExpressionId, u32>,
    cleanup_flags: std::collections::BTreeMap<crate::cleanup::LivenessFlagId, u32>,
    cleanup_storage_flags: HashMap<crate::cleanup_plan::StorageId, u32>,
    cleanup_call_argument_carriers: HashMap<crate::cleanup_plan::StorageId, u32>,
    frame_size: u32,
}

impl FunctionPlan {
    fn build(
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        variant_layouts: &VariantLayoutCache,
    ) -> Result<Self, Diagnostic> {
        let parameter_count = u32::try_from(function.params.len())
            .map_err(|_| error("aggregate function has too many parameters"))?;
        let result_out = parameter_count;
        let mut local_types = Vec::new();
        let mut add_local = |ty: u8| -> Result<u32, Diagnostic> {
            let relative = u32::try_from(local_types.len())
                .map_err(|_| error("aggregate function has too many locals"))?;
            local_types.push(ty);
            parameter_count
                .checked_add(1)
                .and_then(|base| base.checked_add(relative))
                .ok_or_else(|| error("aggregate local index overflows u32"))
        };
        let old_stack = add_local(I32)?;
        let frame_base = add_local(I32)?;
        let status = add_local(I32)?;
        let command_byte = program
            .permits
            .iter()
            .any(|effect| effect == crate::command_io_ops::ARGS_READ_EFFECT)
            .then(|| add_local(I32))
            .transpose()?;
        let external_root_bytes = function
            .params
            .iter()
            .any(|param| matches!(param.ty, ResolvedType::SliceU8 | ResolvedType::Str))
            .then(|| add_local(I64))
            .transpose()?;
        let has_try = expression_has_try(&function.body);
        let result_staged = if has_try { Some(add_local(I32)?) } else { None };
        let mut cleanup_flags = std::collections::BTreeMap::new();
        let mut cleanup_storage_flags = HashMap::new();
        let mut cleanup_call_argument_carriers = HashMap::new();
        for slot in &function.cleanup_plan.slots {
            if slot.ty != ResolvedType::Bytes {
                continue;
            }
            let crate::cleanup::FieldLivenessShape::Leaf { flag, lifecycle } =
                &slot.field_liveness_shape
            else {
                return Err(error("Bytes CleanupPlan slot is not one direct leaf"));
            };
            if lifecycle.as_str() != crate::cleanup::BYTES_DROP_LIFECYCLE_ID {
                return Err(error("Bytes CleanupPlan slot has the wrong lifecycle"));
            }
            let local = add_local(I32)?;
            if cleanup_flags.insert(*flag, local).is_some()
                || cleanup_storage_flags
                    .insert(slot.storage.clone(), local)
                    .is_some()
            {
                return Err(error("Bytes CleanupPlan repeats a liveness identity"));
            }
            if matches!(
                slot.storage,
                crate::cleanup_plan::StorageId::CallArgument { .. }
            ) {
                let carrier = add_local(I64)?;
                if cleanup_call_argument_carriers
                    .insert(slot.storage.clone(), carrier)
                    .is_some()
                {
                    return Err(error("Bytes CleanupPlan repeats a call-argument epoch"));
                }
            }
        }

        let mut frame = FrameAllocator::default();
        let (result_stage_scalar, result_stage_aggregate) =
            if is_aggregate(program, &function.return_type)? {
                let (size, align) =
                    aggregate_size_align(program, variant_layouts, &function.return_type)?;
                (None, Some(frame.allocate(size, align)?))
            } else {
                (
                    Some(add_local(scalar_wasm_type(&function.return_type)?)?),
                    None,
                )
            };
        let mut plan = Self {
            local_types,
            old_stack,
            frame_base,
            status,
            command_byte,
            external_root_bytes,
            result_staged,
            has_try,
            result_out,
            result_stage_scalar,
            result_stage_aggregate,
            scalar_expressions: HashMap::new(),
            scalar_bindings: HashMap::new(),
            aggregate_expressions: HashMap::new(),
            aggregate_bindings: HashMap::new(),
            call_out: HashMap::new(),
            cleanup_flags,
            cleanup_storage_flags,
            cleanup_call_argument_carriers,
            frame_size: 0,
        };
        for contract in &function.requires {
            plan.collect_expr(
                program,
                variant_layouts,
                contract,
                parameter_count,
                &mut frame,
            )?;
        }
        plan.collect_expr(
            program,
            variant_layouts,
            &function.body,
            parameter_count,
            &mut frame,
        )?;
        for contract in &function.ensures {
            plan.collect_expr(
                program,
                variant_layouts,
                contract,
                parameter_count,
                &mut frame,
            )?;
        }
        plan.frame_size = frame.finish()?;
        Ok(plan)
    }

    fn add_local(&mut self, parameter_count: u32, ty: u8) -> Result<u32, Diagnostic> {
        let relative = u32::try_from(self.local_types.len())
            .map_err(|_| error("aggregate function has too many locals"))?;
        self.local_types.push(ty);
        parameter_count
            .checked_add(1)
            .and_then(|base| base.checked_add(relative))
            .ok_or_else(|| error("aggregate local index overflows u32"))
    }

    fn collect_expr(
        &mut self,
        program: &ResolvedProgram,
        variant_layouts: &VariantLayoutCache,
        expr: &ResolvedExpr,
        parameter_count: u32,
        frame: &mut FrameAllocator,
    ) -> Result<(), Diagnostic> {
        if is_aggregate(program, &expr.ty)? {
            let (size, align) = aggregate_size_align(program, variant_layouts, &expr.ty)?;
            let offset = frame.allocate(size, align)?;
            if self
                .aggregate_expressions
                .insert(expr.id.clone(), offset)
                .is_some()
            {
                return Err(error(format!(
                    "duplicate aggregate expression identity `{}`",
                    expr.id
                )));
            }
        } else {
            let ty = scalar_wasm_type(&expr.ty)?;
            let local = self.add_local(parameter_count, ty)?;
            if self
                .scalar_expressions
                .insert(expr.id.clone(), local)
                .is_some()
            {
                return Err(error(format!(
                    "duplicate scalar expression identity `{}`",
                    expr.id
                )));
            }
            if matches!(
                expr.kind,
                ResolvedExprKind::Call { .. } | ResolvedExprKind::HostCommandCall(_)
            ) {
                let (size, align) = scalar_size_align(&expr.ty)?;
                self.call_out
                    .insert(expr.id.clone(), frame.allocate(size, align)?);
            }
        }

        match &expr.kind {
            ResolvedExprKind::Call { args, .. } => {
                for arg in args {
                    self.collect_expr(program, variant_layouts, arg, parameter_count, frame)?;
                }
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                for arg in &call.args {
                    self.collect_expr(program, variant_layouts, arg, parameter_count, frame)?;
                }
            }
            ResolvedExprKind::HostCommandCall(call) => {
                for arg in &call.args {
                    self.collect_expr(program, variant_layouts, arg, parameter_count, frame)?;
                }
            }
            ResolvedExprKind::Unary { value, .. } => {
                self.collect_expr(program, variant_layouts, value, parameter_count, frame)?;
            }
            ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
                self.collect_expr(program, variant_layouts, operand, parameter_count, frame)?;
            }
            ResolvedExprKind::Binary { left, right, .. } => {
                self.collect_expr(program, variant_layouts, left, parameter_count, frame)?;
                self.collect_expr(program, variant_layouts, right, parameter_count, frame)?;
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    let ResolvedStatement::Let { binding, value, .. } = statement else {
                        // Assignment targets reuse their `let` slot and while
                        // statements contribute their condition and body; only
                        // evaluated expressions join the local walk.
                        for index in 0..statement.child_count() {
                            if let Some(child) = statement.child(index) {
                                self.collect_expr(
                                    program,
                                    variant_layouts,
                                    child,
                                    parameter_count,
                                    frame,
                                )?;
                            }
                        }
                        continue;
                    };
                    self.collect_expr(program, variant_layouts, value, parameter_count, frame)?;
                    if is_aggregate(program, &binding.ty)? {
                        let (size, align) =
                            aggregate_size_align(program, variant_layouts, &binding.ty)?;
                        let offset = frame.allocate(size, align)?;
                        if self
                            .aggregate_bindings
                            .insert(binding.id.clone(), offset)
                            .is_some()
                        {
                            return Err(error(format!(
                                "duplicate aggregate binding identity `{}`",
                                binding.id
                            )));
                        }
                    } else {
                        let local =
                            self.add_local(parameter_count, scalar_wasm_type(&binding.ty)?)?;
                        if self
                            .scalar_bindings
                            .insert(binding.id.clone(), local)
                            .is_some()
                        {
                            return Err(error(format!(
                                "duplicate scalar binding identity `{}`",
                                binding.id
                            )));
                        }
                    }
                }
                self.collect_expr(program, variant_layouts, tail, parameter_count, frame)?;
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_expr(program, variant_layouts, condition, parameter_count, frame)?;
                self.collect_expr(
                    program,
                    variant_layouts,
                    then_branch,
                    parameter_count,
                    frame,
                )?;
                self.collect_expr(
                    program,
                    variant_layouts,
                    else_branch,
                    parameter_count,
                    frame,
                )?;
            }
            ResolvedExprKind::ConstructRecord { fields, .. } => {
                for field in fields {
                    self.collect_expr(
                        program,
                        variant_layouts,
                        &field.value,
                        parameter_count,
                        frame,
                    )?;
                }
            }
            ResolvedExprKind::ConstructVariant { fields, .. } => {
                for field in fields {
                    self.collect_expr(
                        program,
                        variant_layouts,
                        &field.value,
                        parameter_count,
                        frame,
                    )?;
                }
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                self.collect_expr(program, variant_layouts, scrutinee, parameter_count, frame)?;
                for arm in arms {
                    match &arm.pattern {
                        crate::hir::ResolvedMatchPattern::Variant { fields, .. } => {
                            for field in fields {
                                let local = self.add_local(
                                    parameter_count,
                                    scalar_wasm_type(&field.binding.ty)?,
                                )?;
                                if self
                                    .scalar_bindings
                                    .insert(field.binding.id.clone(), local)
                                    .is_some()
                                {
                                    return Err(error(format!(
                                        "duplicate match binding identity `{}`",
                                        field.binding.id
                                    )));
                                }
                            }
                        }
                        crate::hir::ResolvedMatchPattern::Record { fields, .. } => self
                            .collect_record_match_bindings(
                                program,
                                variant_layouts,
                                fields,
                                parameter_count,
                                frame,
                            )?,
                        crate::hir::ResolvedMatchPattern::Wildcard => {}
                        // Refutable Match v1: a binding arm owns one scalar
                        // local; literals and or-patterns own nothing.
                        crate::hir::ResolvedMatchPattern::Binding(binding) => {
                            let local =
                                self.add_local(parameter_count, scalar_wasm_type(&binding.ty)?)?;
                            if self
                                .scalar_bindings
                                .insert(binding.id.clone(), local)
                                .is_some()
                            {
                                return Err(error(format!(
                                    "duplicate match binding identity `{}`",
                                    binding.id
                                )));
                            }
                        }
                        crate::hir::ResolvedMatchPattern::Literal(_)
                        | crate::hir::ResolvedMatchPattern::Or(_) => {}
                    }
                    if let Some(guard) = &arm.guard {
                        self.collect_expr(program, variant_layouts, guard, parameter_count, frame)?;
                    }
                    self.collect_expr(
                        program,
                        variant_layouts,
                        &arm.value,
                        parameter_count,
                        frame,
                    )?;
                }
            }
            ResolvedExprKind::Project { base, .. } => {
                self.collect_expr(program, variant_layouts, base, parameter_count, frame)?;
            }
            ResolvedExprKind::Upcast { source } => {
                self.collect_expr(program, variant_layouts, source, parameter_count, frame)?;
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                self.collect_expr(program, variant_layouts, base, parameter_count, frame)?;
                for field in fields {
                    self.collect_expr(
                        program,
                        variant_layouts,
                        &field.value,
                        parameter_count,
                        frame,
                    )?;
                }
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
            | ResolvedExprKind::BorrowPlace { .. } => {}
        }
        Ok(())
    }

    fn collect_record_match_bindings(
        &mut self,
        program: &ResolvedProgram,
        variant_layouts: &VariantLayoutCache,
        fields: &[crate::hir::ResolvedRecordMatchPatternField],
        parameter_count: u32,
        frame: &mut FrameAllocator,
    ) -> Result<(), Diagnostic> {
        for field in fields {
            match &field.pattern {
                crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                    let duplicate = if is_aggregate(program, &binding.ty)? {
                        let (size, align) =
                            aggregate_size_align(program, variant_layouts, &binding.ty)?;
                        self.aggregate_bindings
                            .insert(binding.id.clone(), frame.allocate(size, align)?)
                            .is_some()
                    } else {
                        let local =
                            self.add_local(parameter_count, scalar_wasm_type(&binding.ty)?)?;
                        self.scalar_bindings
                            .insert(binding.id.clone(), local)
                            .is_some()
                    };
                    if duplicate {
                        return Err(error(format!(
                            "duplicate record match binding identity `{}`",
                            binding.id
                        )));
                    }
                }
                crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
                crate::hir::ResolvedRecordMatchFieldPattern::Record { fields, .. } => self
                    .collect_record_match_bindings(
                        program,
                        variant_layouts,
                        fields,
                        parameter_count,
                        frame,
                    )?,
            }
        }
        Ok(())
    }

    fn expr_scalar(&self, expr: &ResolvedExpr) -> Result<u32, Diagnostic> {
        self.scalar_expressions
            .get(&expr.id)
            .copied()
            .ok_or_else(|| error(format!("missing scalar local for `{}`", expr.id)))
    }

    fn expr_pointer(&self, expr: &ResolvedExpr) -> Result<Pointer, Diagnostic> {
        self.aggregate_expressions
            .get(&expr.id)
            .copied()
            .map(|offset| Pointer {
                local: self.frame_base,
                offset,
            })
            .ok_or_else(|| error(format!("missing aggregate slot for `{}`", expr.id)))
    }
}

fn expression_has_try(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => true,
        ResolvedExprKind::Call { args, .. } => args.iter().any(expression_has_try),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.iter().any(expression_has_try),
        ResolvedExprKind::HostCommandCall(call) => call.args.iter().any(expression_has_try),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => expression_has_try(value),
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
        ResolvedExprKind::Match { scrutinee, arms } => {
            expression_has_try(scrutinee) || arms.iter().any(|arm| expression_has_try(&arm.value))
        }
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
        | ResolvedExprKind::BorrowPlace { .. } => false,
    }
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W110", message)
}

fn resource_gate() -> Diagnostic {
    Diagnostic::io(
        "SPX-W111",
        "WebAssembly aggregate resource execution remains private and is not admitted by the public backend",
    )
}

fn is_record(program: &ResolvedProgram, ty: &ResolvedType) -> Result<bool, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    let item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| error(format!("unknown aggregate type `{declaration}`")))?;
    if !matches!(
        item.kind,
        ResolvedTypeDeclarationKind::Record { .. } | ResolvedTypeDeclarationKind::Class { .. }
    ) {
        return Ok(false);
    }
    if arguments.len() != item.type_parameters.len()
        || arguments
            .iter()
            .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
    {
        return Err(error(format!(
            "Wasm record representation requires exact concrete i64/bool arguments for `{}`",
            ty.identity_key()
        )));
    }
    Ok(true)
}

fn is_variant(program: &ResolvedProgram, ty: &ResolvedType) -> Result<bool, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    let item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| error(format!("unknown aggregate type `{declaration}`")))?;
    if !matches!(item.kind, ResolvedTypeDeclarationKind::Variant { .. }) {
        return Ok(false);
    }
    if arguments.len() != item.type_parameters.len()
        || arguments.iter().any(|argument| {
            !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                && !(declaration.as_str() == crate::prelude::OPTION_ID
                    && *argument == ResolvedType::U8)
        })
    {
        return Err(error(format!(
            "Wasm variant representation requires admitted exact concrete arguments for `{}`",
            ty.identity_key()
        )));
    }
    Ok(true)
}

fn is_aggregate(program: &ResolvedProgram, ty: &ResolvedType) -> Result<bool, Diagnostic> {
    Ok(matches!(ty, ResolvedType::ArrayU8(_))
        || is_record(program, ty)?
        || is_variant(program, ty)?)
}

fn layout(program: &ResolvedProgram, ty: &ResolvedType) -> Result<AggregateLayout, Diagnostic> {
    let layout = AggregateLayout::for_type(program, AggregateTarget::Wasm32, ty)?;
    layout.validate(program)?;
    Ok(layout)
}

fn variant_layout(
    variant_layouts: &VariantLayoutCache,
    ty: &ResolvedType,
) -> Result<VariantLayout, Diagnostic> {
    Ok(variant_layouts.layout(ty)?.clone())
}

fn aggregate_size_align(
    program: &ResolvedProgram,
    variant_layouts: &VariantLayoutCache,
    ty: &ResolvedType,
) -> Result<(u32, u32), Diagnostic> {
    if is_record(program, ty)? {
        let layout = layout(program, ty)?;
        Ok((layout.size, layout.align))
    } else if is_variant(program, ty)? {
        let layout = variant_layout(variant_layouts, ty)?;
        Ok((layout.size, layout.align))
    } else if let ResolvedType::ArrayU8(length) = ty {
        Ok((*length, 1))
    } else {
        Err(error(format!(
            "aggregate layout requested for scalar `{}`",
            ty.identity_key()
        )))
    }
}

fn scalar_wasm_type(ty: &ResolvedType) -> Result<u8, Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok(I64),
        ResolvedType::I32 => Ok(I32),
        ResolvedType::Char => Ok(I32),
        ResolvedType::U8 => Ok(I32),
        ResolvedType::Usize => Ok(I64),
        ResolvedType::SliceU8 | ResolvedType::Str => Ok(I64),
        ResolvedType::Bytes => Ok(I64),
        ResolvedType::F32 => Ok(F32),
        ResolvedType::F64 => Ok(F64),
        ResolvedType::Bool => Ok(I32),
        _ => Err(error(format!(
            "non-scalar type `{}` reached scalar aggregate lowering",
            ty.identity_key()
        ))),
    }
}

fn scalar_size_align(ty: &ResolvedType) -> Result<(u32, u32), Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok((8, 8)),
        ResolvedType::I32 => Ok((4, 4)),
        ResolvedType::Char => Ok((4, 4)),
        ResolvedType::U8 => Ok((4, 4)),
        ResolvedType::Usize => Ok((8, 8)),
        ResolvedType::SliceU8 | ResolvedType::Str => Ok((8, 8)),
        ResolvedType::Bytes => Ok((8, 8)),
        ResolvedType::F32 => Ok((4, 4)),
        ResolvedType::F64 => Ok((8, 8)),
        ResolvedType::Bool => Ok((4, 4)),
        _ => Err(error(format!(
            "non-scalar type `{}` has no Wasm32 scalar layout",
            ty.identity_key()
        ))),
    }
}

#[cfg(any(test, feature = "unstable-wit-component-harness"))]
pub(super) fn lower_selected_functions(
    program: &ResolvedProgram,
    ordered_function_ids: &[DeclarationId],
    selected: &DeclarationId,
) -> Result<SelectedAggregateLowering, Diagnostic> {
    if ordered_function_ids.is_empty() {
        return Err(error(
            "selected aggregate lowering requires a function closure",
        ));
    }
    if program
        .types
        .iter()
        .any(|item| matches!(item.kind, ResolvedTypeDeclarationKind::Resource { .. }))
    {
        return Err(resource_gate());
    }
    let variant_layouts = VariantLayoutCache::build(program, VariantTarget::Wasm32)?;
    let mut types = Vec::<Signature>::new();
    let mut type_indexes = HashMap::<Signature, u32>::new();
    let mut function_type_indexes = Vec::with_capacity(ordered_function_ids.len());
    let mut function_indexes = HashMap::with_capacity(ordered_function_ids.len());
    for (index, id) in ordered_function_ids.iter().enumerate() {
        let function = program
            .functions
            .iter()
            .find(|function| function.id == *id)
            .ok_or_else(|| error(format!("selected aggregate function `{id}` is missing")))?;
        let mut params = Vec::with_capacity(function.params.len() + 1);
        for param in &function.params {
            params.push(if is_aggregate(program, &param.ty)? {
                I32
            } else {
                scalar_wasm_type(&param.ty)?
            });
        }
        params.push(I32);
        function_type_indexes.push(intern_type(
            Signature {
                params,
                results: vec![I32],
            },
            &mut types,
            &mut type_indexes,
        ));
        let index = u32::try_from(index)
            .map_err(|_| error("selected aggregate function index overflows u32"))?;
        if function_indexes
            .insert(FunctionExecutionId::Monomorphic(id.clone()), index)
            .is_some()
        {
            return Err(error(format!(
                "selected aggregate closure repeats function `{id}`"
            )));
        }
    }
    let selected_index = *function_indexes
        .get(&FunctionExecutionId::Monomorphic(selected.clone()))
        .ok_or_else(|| error(format!("selected aggregate closure omits `{selected}`")))?;
    let mut bodies = Vec::with_capacity(ordered_function_ids.len());
    for id in ordered_function_ids {
        let function = program
            .functions
            .iter()
            .find(|function| function.id == *id)
            .ok_or_else(|| error(format!("selected aggregate function `{id}` is missing")))?;
        bodies.push(emit_function(
            program,
            function,
            &function_indexes,
            &variant_layouts,
            None,
        )?);
    }
    Ok(SelectedAggregateLowering {
        types,
        function_type_indexes,
        bodies,
        selected_index,
    })
}

#[cfg(any(test, feature = "unstable-wit-component-harness"))]
pub(super) fn lower_selected_function_instances(
    program: &ResolvedProgram,
    ordered_instance_ids: &[crate::hir::FunctionInstanceId],
    selected: &crate::hir::FunctionInstanceId,
) -> Result<SelectedAggregateLowering, Diagnostic> {
    crate::hir::validate(program)?;
    if ordered_instance_ids.is_empty() {
        return Err(error(
            "selected generic aggregate lowering requires a function-instance closure",
        ));
    }
    if program
        .types
        .iter()
        .any(|item| matches!(item.kind, ResolvedTypeDeclarationKind::Resource { .. }))
    {
        return Err(resource_gate());
    }
    if ordered_instance_ids.len() != program.function_instances.len()
        || ordered_instance_ids
            .iter()
            .zip(&program.function_instances)
            .any(|(expected, instance)| expected != &instance.id)
    {
        return Err(error(
            "selected generic aggregate closure is not the exact reachable instance sequence",
        ));
    }
    for instance in &program.function_instances {
        if crate::hir::FunctionInstanceId::derive(&instance.template, &instance.type_arguments)
            != instance.id
        {
            return Err(error(
                "selected generic aggregate instance identity is inconsistent",
            ));
        }
    }

    let variant_layouts = VariantLayoutCache::build(program, VariantTarget::Wasm32)?;
    let mut types = Vec::<Signature>::new();
    let mut type_indexes = HashMap::<Signature, u32>::new();
    let mut function_type_indexes = Vec::with_capacity(program.function_instances.len());
    let mut function_indexes = HashMap::with_capacity(program.function_instances.len());
    for (index, instance) in program.function_instances.iter().enumerate() {
        let function = &instance.function;
        let mut params = Vec::with_capacity(function.params.len() + 1);
        for param in &function.params {
            params.push(if is_aggregate(program, &param.ty)? {
                I32
            } else {
                scalar_wasm_type(&param.ty)?
            });
        }
        params.push(I32);
        function_type_indexes.push(intern_type(
            Signature {
                params,
                results: vec![I32],
            },
            &mut types,
            &mut type_indexes,
        ));
        let index = u32::try_from(index)
            .map_err(|_| error("selected generic aggregate function index overflows u32"))?;
        if function_indexes
            .insert(FunctionExecutionId::Generic(instance.id.clone()), index)
            .is_some()
        {
            return Err(error(format!(
                "selected generic aggregate closure repeats function instance `{}`",
                instance.id
            )));
        }
    }
    let selected_index = *function_indexes
        .get(&FunctionExecutionId::Generic(selected.clone()))
        .ok_or_else(|| {
            error(format!(
                "selected generic aggregate closure omits function instance `{selected}`"
            ))
        })?;
    let bodies = program
        .function_instances
        .iter()
        .map(|instance| {
            emit_function(
                program,
                &instance.function,
                &function_indexes,
                &variant_layouts,
                None,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SelectedAggregateLowering {
        types,
        function_type_indexes,
        bodies,
        selected_index,
    })
}

pub(super) fn emit(program: &ResolvedProgram) -> Result<Vec<u8>, Diagnostic> {
    emit_profile(program, false, false)
}

#[cfg(test)]
pub(super) fn emit_stdout_transcript(program: &ResolvedProgram) -> Result<Vec<u8>, Diagnostic> {
    emit_profile(program, false, true)
}

/// Emit Public Useful Data Export v1 without widening the legacy aggregate
/// module. The complete internal function inventory is compiled, but only the
/// selected raw wrappers and fixed scratch metadata are exported.
pub(super) fn emit_byte_exports(
    program: &ResolvedProgram,
    plans: &[super::data_exports::DataExportPlan],
) -> Result<Vec<u8>, Diagnostic> {
    emit_byte_exports_profile(program, plans, false, false, None)
}

pub(super) fn emit_byte_exports_with_stdout_transcript(
    program: &ResolvedProgram,
    plans: &[super::data_exports::DataExportPlan],
) -> Result<Vec<u8>, Diagnostic> {
    emit_byte_exports_profile(program, plans, true, false, None)
}

pub(super) fn emit_useful_data_command_v2(
    program: &ResolvedProgram,
    plans: &[super::data_exports::DataExportPlan],
) -> Result<Vec<u8>, Diagnostic> {
    emit_byte_exports_profile(program, plans, true, true, None)
}

pub(super) fn emit_language_command_io(
    program: &ResolvedProgram,
    plan: &super::command_io::CommandPlan,
) -> Result<Vec<u8>, Diagnostic> {
    emit_byte_exports_profile(program, &[], true, false, Some(plan))
}

fn emit_byte_exports_profile(
    program: &ResolvedProgram,
    plans: &[super::data_exports::DataExportPlan],
    host_output: bool,
    publish_only_truthy: bool,
    command_io: Option<&super::command_io::CommandPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    if (plans.is_empty() && command_io.is_none()) || !super::program_uses_byte_data(program) {
        return Err(error(
            "Public Useful Data Export v1 requires selected byte-data exports",
        ));
    }
    if program
        .types
        .iter()
        .any(|item| matches!(item.kind, ResolvedTypeDeclarationKind::Resource { .. }))
    {
        return Err(resource_gate());
    }
    let variant_layouts = VariantLayoutCache::build(program, VariantTarget::Wasm32)?;
    let record_layouts = AggregateLayoutCache::build(program, AggregateTarget::Wasm32)?;
    for record_layout in record_layouts.layouts() {
        record_layout.validate(program)?;
    }

    let executable_functions = program
        .functions
        .iter()
        .map(|function| {
            (
                function,
                FunctionExecutionId::Monomorphic(function.id.clone()),
            )
        })
        .collect::<Vec<_>>();
    let mut types = Vec::<Signature>::new();
    let mut type_indexes = HashMap::<Signature, u32>::new();
    let binary_checked = intern_type(
        Signature {
            params: vec![I64, I64],
            results: vec![I64],
        },
        &mut types,
        &mut type_indexes,
    );
    let unary_checked = intern_type(
        Signature {
            params: vec![I64],
            results: vec![I64],
        },
        &mut types,
        &mut type_indexes,
    );
    let contract_fail = intern_type(
        Signature {
            params: Vec::new(),
            results: Vec::new(),
        },
        &mut types,
        &mut type_indexes,
    );
    let byte_unary = intern_type(
        Signature {
            params: vec![I64],
            results: vec![I64],
        },
        &mut types,
        &mut type_indexes,
    );
    let byte_get = intern_type(
        Signature {
            params: vec![I64, I64],
            results: vec![I32],
        },
        &mut types,
        &mut type_indexes,
    );
    let byte_drop = intern_type(
        Signature {
            params: vec![I64],
            results: Vec::new(),
        },
        &mut types,
        &mut type_indexes,
    );

    let mut function_types = Vec::with_capacity(executable_functions.len());
    for (function, _) in &executable_functions {
        let mut params = Vec::with_capacity(function.params.len() + 1);
        for parameter in &function.params {
            params.push(if is_aggregate(program, &parameter.ty)? {
                I32
            } else {
                scalar_wasm_type(&parameter.ty)?
            });
        }
        params.push(I32); // exact caller-owned result slot
        function_types.push(intern_type(
            Signature {
                params,
                results: vec![I32], // sticky internal status
            },
            &mut types,
            &mut type_indexes,
        ));
    }
    let mut wrapper_types = plans
        .iter()
        .map(|plan| {
            intern_type(
                Signature {
                    params: plan.raw_params(),
                    results: vec![plan.result.raw_wasm_type()],
                },
                &mut types,
                &mut type_indexes,
            )
        })
        .collect::<Vec<_>>();
    if command_io.is_some() {
        wrapper_types.push(intern_type(
            Signature {
                params: Vec::new(),
                results: vec![I32],
            },
            &mut types,
            &mut type_indexes,
        ));
    }

    let command_import_count = if command_io.is_some() {
        super::command_io::IMPORT_COUNT
    } else {
        0
    };
    let command_import_types = command_io.map(|_| {
        (
            intern_type(
                Signature {
                    params: Vec::new(),
                    results: vec![I64],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I64, I32],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I64],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
        )
    });

    let function_indexes = executable_functions
        .iter()
        .enumerate()
        .map(|(index, (_, execution))| {
            (
                execution.clone(),
                SCALAR_IMPORT_COUNT
                    + BYTE_IMPORT_COUNT
                    + command_import_count
                    + u32::try_from(index).unwrap_or(u32::MAX),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut module = b"\0asm\x01\0\0\0".to_vec();
    let mut type_section = Vec::new();
    write_u32(&mut type_section, types.len() as u32);
    for signature in &types {
        type_section.push(0x60);
        write_bytes(&mut type_section, &signature.params);
        write_bytes(&mut type_section, &signature.results);
    }
    section(&mut module, 1, type_section);

    let mut imports = Vec::new();
    write_u32(
        &mut imports,
        SCALAR_IMPORT_COUNT + BYTE_IMPORT_COUNT + command_import_count,
    );
    for name in ["spx_add", "spx_sub", "spx_mul", "spx_div", "spx_rem"] {
        function_import(&mut imports, "env", name, binary_checked);
    }
    function_import(&mut imports, "env", "spx_neg", unary_checked);
    function_import(&mut imports, "env", "spx_contract_fail", contract_fail);
    function_import(&mut imports, "env", "spx_bytes_copy", byte_unary);
    function_import(&mut imports, "env", "spx_bytes_get", byte_get);
    function_import(&mut imports, "env", "spx_bytes_drop", byte_drop);
    function_import(&mut imports, "env", "spx_bytes_as_slice", byte_unary);
    if let Some((args_len, arg_utf8, stdin_read, owned_validate)) = command_import_types {
        function_import(&mut imports, "env", "spx_command_args_len_v1", args_len);
        function_import(&mut imports, "env", "spx_command_arg_utf8_v1", arg_utf8);
        function_import(&mut imports, "env", "spx_command_stdin_read_v1", stdin_read);
        function_import(
            &mut imports,
            "env",
            "spx_command_owned_bytes_validate_v1",
            owned_validate,
        );
    }
    section(&mut module, 2, imports);

    let mut functions = Vec::new();
    write_u32(
        &mut functions,
        u32::try_from(function_types.len() + wrapper_types.len())
            .map_err(|_| error("too many Public Useful Data functions"))?,
    );
    for type_index in function_types.into_iter().chain(wrapper_types) {
        write_u32(&mut functions, type_index);
    }
    section(&mut module, 3, functions);

    let mut memory = Vec::new();
    write_u32(&mut memory, 1);
    let memory_pages = if command_io.is_some() {
        6
    } else if host_output {
        super::host_output::MEMORY_PAGES
    } else {
        super::data_exports::FIXED_MEMORY_PAGES
    };
    memory.extend([0x01, memory_pages, memory_pages]);
    section(&mut module, 5, memory);

    // Global 0 is the private shadow-stack top. The public globals follow in
    // exact status/base/capacity order and are the only exported globals.
    let mut globals = Vec::new();
    write_u32(
        &mut globals,
        if command_io.is_some() {
            15
        } else if host_output {
            9
        } else {
            4
        },
    );
    globals.extend([I32, 0x01, 0x41]);
    write_i64(&mut globals, 131_072);
    globals.push(0x0b);
    globals.extend([I32, 0x01, 0x41, 0x00, 0x0b]);
    globals.extend([I32, 0x00, 0x41]);
    write_i64(&mut globals, i64::from(super::data_exports::SCRATCH_BASE));
    globals.push(0x0b);
    globals.extend([I32, 0x00, 0x41]);
    write_i64(
        &mut globals,
        i64::from(super::data_exports::SCRATCH_CAPACITY),
    );
    globals.push(0x0b);
    if host_output {
        super::host_output::append_data_globals(&mut globals);
    }
    if command_io.is_some() {
        super::host_output::append_stderr_data_globals(&mut globals);
        // Mutable marker for the authenticated command-input sub-domain.
        // Generic language failures continue to use only the ordinary status
        // global and must never be attributed to this domain.
        globals.extend([I32, 0x01, 0x41, 0x00, 0x0b]);
    }
    section(&mut module, 6, globals);

    let mut exports = Vec::new();
    write_u32(
        &mut exports,
        (if command_io.is_some() {
            11_u32
        } else if host_output {
            7_u32
        } else {
            4_u32
        })
        .checked_add(
            u32::try_from(plans.len() + usize::from(command_io.is_some()))
                .map_err(|_| error("too many data exports"))?,
        )
        .ok_or_else(|| error("Public Useful Data export count overflows u32"))?,
    );
    write_name(&mut exports, super::data_exports::MEMORY_EXPORT);
    exports.push(0x02);
    write_u32(&mut exports, 0);
    for (name, index) in [
        (super::data_exports::STATUS_GLOBAL_EXPORT, 1_u32),
        (super::data_exports::SCRATCH_BASE_EXPORT, 2_u32),
        (super::data_exports::SCRATCH_CAPACITY_EXPORT, 3_u32),
    ] {
        write_name(&mut exports, name);
        exports.push(0x03);
        write_u32(&mut exports, index);
    }
    if host_output {
        super::host_output::append_exports(&mut exports, super::host_output::DATA_GLOBALS, false);
    }
    if command_io.is_some() {
        super::host_output::append_stderr_exports(&mut exports);
        write_name(&mut exports, super::command_io::INPUT_STATUS_EXPORT);
        exports.push(0x03);
        write_u32(&mut exports, super::command_io::INPUT_STATUS_GLOBAL);
    }
    let wrapper_base = SCALAR_IMPORT_COUNT
        .checked_add(BYTE_IMPORT_COUNT)
        .and_then(|value| value.checked_add(command_import_count))
        .and_then(|value| value.checked_add(u32::try_from(executable_functions.len()).ok()?))
        .ok_or_else(|| error("Public Useful Data wrapper index overflows u32"))?;
    for (ordinal, plan) in plans.iter().enumerate() {
        write_name(&mut exports, &plan.wasm_export);
        exports.push(0x00);
        write_u32(
            &mut exports,
            wrapper_base
                .checked_add(u32::try_from(ordinal).map_err(|_| error("too many wrappers"))?)
                .ok_or_else(|| error("Public Useful Data wrapper index overflows u32"))?,
        );
    }
    if let Some(plan) = command_io {
        write_name(&mut exports, &plan.wasm_export);
        exports.push(0x00);
        write_u32(
            &mut exports,
            wrapper_base
                .checked_add(u32::try_from(plans.len()).map_err(|_| error("too many wrappers"))?)
                .ok_or_else(|| error("Language Command wrapper index overflows u32"))?,
        );
    }
    section(&mut module, 7, exports);

    let mut code = Vec::new();
    write_u32(
        &mut code,
        u32::try_from(executable_functions.len() + plans.len() + usize::from(command_io.is_some()))
            .map_err(|_| error("too many Public Useful Data bodies"))?,
    );
    for (function, _) in &executable_functions {
        let body = emit_function(
            program,
            function,
            &function_indexes,
            &variant_layouts,
            host_output.then_some(super::host_output::DATA_GLOBALS),
        )?;
        write_u32(&mut code, body.len() as u32);
        code.extend(body);
    }
    for plan in plans {
        let target = function_indexes
            .get(&FunctionExecutionId::Monomorphic(plan.function_id.clone()))
            .copied()
            .ok_or_else(|| error("selected data export target is not indexed"))?;
        let body = if publish_only_truthy {
            plan.emit_command_v2_wrapper_body(target, 0, 1, super::host_output::DATA_GLOBALS)?
        } else if host_output {
            plan.emit_wrapper_body_with_stdout_transcript(
                target,
                0,
                1,
                super::host_output::DATA_GLOBALS,
            )?
        } else {
            plan.emit_wrapper_body(target, 0, 1)?
        };
        write_u32(&mut code, body.len() as u32);
        code.extend(body);
    }
    if let Some(plan) = command_io {
        let target = function_indexes
            .get(&FunctionExecutionId::Monomorphic(plan.function_id.clone()))
            .copied()
            .ok_or_else(|| error("selected Language Command target is not indexed"))?;
        let body = super::command_io::emit_wrapper_body(target);
        write_u32(&mut code, body.len() as u32);
        code.extend(body);
    }
    section(&mut module, 10, code);
    Ok(module)
}

fn emit_profile(
    program: &ResolvedProgram,
    test_exports: bool,
    host_output: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let uses_byte_data = super::program_uses_byte_data(program);
    if program
        .types
        .iter()
        .any(|item| matches!(item.kind, ResolvedTypeDeclarationKind::Resource { .. }))
    {
        return Err(resource_gate());
    }
    let variant_layouts = VariantLayoutCache::build(program, VariantTarget::Wasm32)?;
    let record_layouts = AggregateLayoutCache::build(program, AggregateTarget::Wasm32)?;
    for record_layout in record_layouts.layouts() {
        record_layout.validate(program)?;
    }

    let main_index = program
        .functions
        .iter()
        .position(|function| function.id == program.entrypoint)
        .ok_or_else(|| error("web target requires a main function"))?;
    let main = &program.functions[main_index];
    if !main.params.is_empty() || main.return_type != ResolvedType::I64 {
        return Err(error(
            "resolved web entry point must have type `fn main() -> i64`",
        ));
    }

    let mut types = Vec::<Signature>::new();
    let mut type_indexes = HashMap::<Signature, u32>::new();
    let binary_checked = intern_type(
        Signature {
            params: vec![I64, I64],
            results: vec![I64],
        },
        &mut types,
        &mut type_indexes,
    );
    let unary_checked = intern_type(
        Signature {
            params: vec![I64],
            results: vec![I64],
        },
        &mut types,
        &mut type_indexes,
    );
    let contract_fail = intern_type(
        Signature {
            params: Vec::new(),
            results: Vec::new(),
        },
        &mut types,
        &mut type_indexes,
    );
    let byte_unary = uses_byte_data.then(|| {
        intern_type(
            Signature {
                params: vec![I64],
                results: vec![I64],
            },
            &mut types,
            &mut type_indexes,
        )
    });
    let byte_get = uses_byte_data.then(|| {
        intern_type(
            Signature {
                params: vec![I64, I64],
                results: vec![I32],
            },
            &mut types,
            &mut type_indexes,
        )
    });
    let byte_drop = uses_byte_data.then(|| {
        intern_type(
            Signature {
                params: vec![I64],
                results: Vec::new(),
            },
            &mut types,
            &mut type_indexes,
        )
    });

    let executable_functions = program
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
        .collect::<Vec<_>>();
    let mut function_types = Vec::with_capacity(executable_functions.len());
    for (function, _) in &executable_functions {
        let mut params = Vec::with_capacity(function.params.len() + 1);
        for param in &function.params {
            params.push(if is_aggregate(program, &param.ty)? {
                I32
            } else {
                scalar_wasm_type(&param.ty)?
            });
        }
        params.push(I32);
        function_types.push(intern_type(
            Signature {
                params,
                results: vec![I32],
            },
            &mut types,
            &mut type_indexes,
        ));
    }
    let wrapper_type = intern_type(
        Signature {
            params: Vec::new(),
            results: vec![I64],
        },
        &mut types,
        &mut type_indexes,
    );
    let function_indexes = executable_functions
        .iter()
        .enumerate()
        .map(|(index, (_, execution))| {
            (
                execution.clone(),
                SCALAR_IMPORT_COUNT
                    + if uses_byte_data { BYTE_IMPORT_COUNT } else { 0 }
                    + u32::try_from(index).unwrap_or(u32::MAX),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut module = b"\0asm\x01\0\0\0".to_vec();
    let mut type_section = Vec::new();
    write_u32(&mut type_section, types.len() as u32);
    for signature in &types {
        type_section.push(0x60);
        write_bytes(&mut type_section, &signature.params);
        write_bytes(&mut type_section, &signature.results);
    }
    section(&mut module, 1, type_section);

    let mut imports = Vec::new();
    write_u32(
        &mut imports,
        SCALAR_IMPORT_COUNT + if uses_byte_data { BYTE_IMPORT_COUNT } else { 0 },
    );
    for name in ["spx_add", "spx_sub", "spx_mul", "spx_div", "spx_rem"] {
        function_import(&mut imports, "env", name, binary_checked);
    }
    function_import(&mut imports, "env", "spx_neg", unary_checked);
    function_import(&mut imports, "env", "spx_contract_fail", contract_fail);
    if uses_byte_data {
        function_import(&mut imports, "env", "spx_bytes_copy", byte_unary.unwrap());
        function_import(&mut imports, "env", "spx_bytes_get", byte_get.unwrap());
        function_import(&mut imports, "env", "spx_bytes_drop", byte_drop.unwrap());
        function_import(
            &mut imports,
            "env",
            "spx_bytes_as_slice",
            byte_unary.unwrap(),
        );
    }
    section(&mut module, 2, imports);

    let mut function_section = Vec::new();
    write_u32(
        &mut function_section,
        u32::try_from(function_types.len() + 1)
            .map_err(|_| error("too many aggregate functions"))?,
    );
    for ty in function_types {
        write_u32(&mut function_section, ty);
    }
    write_u32(&mut function_section, wrapper_type);
    section(&mut module, 3, function_section);

    let mut memory = Vec::new();
    write_u32(&mut memory, 1);
    if uses_byte_data {
        if host_output {
            memory.extend([
                0x01,
                super::host_output::MEMORY_PAGES,
                super::host_output::MEMORY_PAGES,
            ]);
        } else {
            memory.extend([0x01, 0x02, 0x02]);
        }
    } else {
        memory.extend([0x00, 0x01]);
    }
    section(&mut module, 5, memory);

    let mut globals = Vec::new();
    write_u32(&mut globals, if host_output { 5 } else { 1 });
    globals.extend([I32, 0x01, 0x41]);
    write_i64(
        &mut globals,
        i64::from(if uses_byte_data {
            131_072
        } else {
            SHADOW_STACK_TOP
        }),
    );
    globals.push(0x0b);
    if host_output {
        super::host_output::append_globals(&mut globals);
    }
    section(&mut module, 6, globals);

    let mut exports = Vec::new();
    let extra_exports = if test_exports {
        u32::try_from(executable_functions.len())
            .map_err(|_| error("too many aggregate test exports"))?
            .checked_add(2)
            .ok_or_else(|| error("aggregate test export count overflows u32"))?
    } else {
        0
    };
    write_u32(
        &mut exports,
        1 + extra_exports + u32::from(uses_byte_data) + if host_output { 4 } else { 0 },
    );
    write_name(&mut exports, "semaprax_main");
    exports.push(0x00);
    let wrapper_index = SCALAR_IMPORT_COUNT
        .checked_add(if uses_byte_data { BYTE_IMPORT_COUNT } else { 0 })
        .ok_or_else(|| error("aggregate wrapper import count overflows u32"))?
        .checked_add(
            u32::try_from(executable_functions.len()).map_err(|_| error("too many functions"))?,
        )
        .ok_or_else(|| error("aggregate wrapper index overflows u32"))?;
    write_u32(&mut exports, wrapper_index);
    if uses_byte_data {
        write_name(&mut exports, "__spx_byte_memory");
        exports.push(0x02);
        write_u32(&mut exports, 0);
    }
    if host_output {
        super::host_output::append_exports(&mut exports, super::host_output::ROOT_GLOBALS, true);
    }
    if test_exports {
        write_name(&mut exports, "__spx_test_memory");
        exports.push(0x02);
        write_u32(&mut exports, 0);
        write_name(&mut exports, "__spx_test_shadow_stack");
        exports.push(0x03);
        write_u32(&mut exports, 0);
        for (_function, execution) in &executable_functions {
            write_name(
                &mut exports,
                &format!("__spx_test_{}", hex_execution_identity(execution)),
            );
            exports.push(0x00);
            write_u32(
                &mut exports,
                *function_indexes
                    .get(execution)
                    .ok_or_else(|| error("aggregate test function is not indexed"))?,
            );
        }
    }
    section(&mut module, 7, exports);

    let mut code = Vec::new();
    write_u32(
        &mut code,
        u32::try_from(executable_functions.len() + 1)
            .map_err(|_| error("too many aggregate function bodies"))?,
    );
    for (function, _) in &executable_functions {
        let body = emit_function(
            program,
            function,
            &function_indexes,
            &variant_layouts,
            host_output.then_some(super::host_output::ROOT_GLOBALS),
        )?;
        write_u32(&mut code, body.len() as u32);
        code.extend(body);
    }
    let wrapper = emit_wrapper(
        *function_indexes
            .get(&FunctionExecutionId::Monomorphic(main.id.clone()))
            .ok_or_else(|| error("aggregate main function is not indexed"))?,
        host_output,
    );
    write_u32(&mut code, wrapper.len() as u32);
    code.extend(wrapper);
    section(&mut module, 10, code);
    Ok(module)
}

fn hex_identity(id: &DeclarationId) -> String {
    let mut output = String::new();
    for byte in id.as_str().bytes() {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn hex_execution_identity(id: &FunctionExecutionId) -> String {
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

fn emit_function(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    function_indexes: &HashMap<FunctionExecutionId, u32>,
    variant_layouts: &VariantLayoutCache,
    host_output: Option<super::host_output::Globals>,
) -> Result<Vec<u8>, Diagnostic> {
    let plan = FunctionPlan::build(program, function, variant_layouts)?;
    let mut body = Vec::new();
    write_u32(&mut body, plan.local_types.len() as u32);
    for ty in &plan.local_types {
        write_u32(&mut body, 1);
        body.push(*ty);
    }

    body.push(0x23);
    write_u32(&mut body, 0);
    body.push(0x21);
    write_u32(&mut body, plan.old_stack);
    if let Some(external_root_bytes) = plan.external_root_bytes {
        emit_external_byte_root_admission(
            &mut body,
            program,
            function,
            plan.old_stack,
            external_root_bytes,
        )?;
    }
    if plan.frame_size != 0 {
        body.push(0x20);
        write_u32(&mut body, plan.old_stack);
        body.push(0x41);
        write_i64(&mut body, i64::from(plan.frame_size));
        body.push(0x49);
        body.extend([0x04, 0x40, 0x00, 0x0b]);
    }
    body.push(0x20);
    write_u32(&mut body, plan.old_stack);
    body.push(0x41);
    write_i64(&mut body, i64::from(plan.frame_size));
    body.push(0x6b);
    body.push(0x22);
    write_u32(&mut body, plan.frame_base);
    body.push(0x24);
    write_u32(&mut body, 0);
    body.push(0x41);
    write_i64(&mut body, i64::from(STATUS_SUCCESS));
    body.push(0x21);
    write_u32(&mut body, plan.status);
    if let Some(result_staged) = plan.result_staged {
        body.extend([0x41, 0x00, 0x21]);
        write_u32(&mut body, result_staged);
    }
    for place in &function.cleanup_plan.entry_state.live_owned_parameters {
        if !place.projections.is_empty() {
            return Err(error("Bytes entry liveness is not a direct storage root"));
        }
        let flag = plan
            .cleanup_storage_flags
            .get(&place.storage)
            .copied()
            .ok_or_else(|| error("Bytes entry liveness has no exact flag local"))?;
        body.extend([0x41, 0x01, 0x21]);
        write_u32(&mut body, flag);
    }

    let mut bindings = HashMap::new();
    for (index, param) in function.params.iter().enumerate() {
        let local = u32::try_from(index).map_err(|_| error("parameter index overflows u32"))?;
        let value = if is_aggregate(program, &param.ty)? {
            Value::Aggregate {
                pointer: Pointer { local, offset: 0 },
                ty: param.ty.clone(),
            }
        } else {
            Value::Scalar {
                local,
                ty: param.ty.clone(),
            }
        };
        bindings.insert(param.id.clone(), value);
    }
    body.extend([0x02, 0x40]);
    let mut emitter = Emitter {
        output: &mut body,
        program,
        variant_layouts,
        function_indexes,
        plan: &plan,
        return_type: &function.return_type,
        cleanup_plan: &function.cleanup_plan,
        bindings,
        control_depth: 0,
        status_exit_extra_depth: 1,
        try_target_enabled: false,
        failure_expression: None,
        host_output,
    };
    for contract in &function.requires {
        let condition = emitter.emit_expr(contract)?;
        emitter.require_scalar(&condition, &ResolvedType::Bool, "precondition")?;
        emitter.get_scalar(&condition);
        emitter.output.push(0x45);
        emitter.failure_expression = Some(contract.id.clone());
        emitter.fail_if(STATUS_REQUIRES_FALSE)?;
        emitter.failure_expression = None;
    }
    if plan.has_try {
        emitter.output.extend([0x02, 0x40]);
        emitter.status_exit_extra_depth = 2;
        emitter.try_target_enabled = true;
    }
    let result = emitter.emit_expr(&function.body)?;
    emitter.try_target_enabled = false;
    let staged = if let Some(local) = plan.result_stage_scalar {
        emitter.require_scalar(&result, &function.return_type, "function result")?;
        emitter.get_scalar(&result);
        emitter.output.push(0x21);
        write_u32(emitter.output, local);
        Value::Scalar {
            local,
            ty: function.return_type.clone(),
        }
    } else {
        let offset = plan
            .result_stage_aggregate
            .ok_or_else(|| error("missing aggregate result staging slot"))?;
        let staged = Value::Aggregate {
            pointer: Pointer {
                local: plan.frame_base,
                offset,
            },
            ty: function.return_type.clone(),
        };
        emitter.copy_value(&staged, &result, "function result")?;
        staged
    };
    if let Some(result_staged) = plan.result_staged {
        emitter.output.extend([0x41, 0x01, 0x21]);
        write_u32(emitter.output, result_staged);
        emitter.output.push(0x0b);
        emitter.status_exit_extra_depth = 1;
        emitter.output.push(0x20);
        write_u32(emitter.output, result_staged);
        emitter.output.extend([0x45, 0x04, 0x40, 0x00, 0x0b]);
    }
    emitter
        .bindings
        .insert(function.result_id.clone(), staged.clone());
    for contract in &function.ensures {
        let condition = emitter.emit_expr(contract)?;
        emitter.require_scalar(&condition, &ResolvedType::Bool, "postcondition")?;
        emitter.get_scalar(&condition);
        emitter.output.push(0x45);
        emitter.failure_expression = Some(contract.id.clone());
        emitter.fail_if(STATUS_ENSURES_FALSE)?;
        emitter.failure_expression = None;
    }
    emitter.emit_success_cleanup(&function.cleanup_plan)?;
    let caller = if is_aggregate(program, &function.return_type)? {
        Value::Aggregate {
            pointer: Pointer {
                local: plan.result_out,
                offset: 0,
            },
            ty: function.return_type.clone(),
        }
    } else {
        Value::Scalar {
            local: plan.result_out,
            ty: function.return_type.clone(),
        }
    };
    if matches!(caller, Value::Aggregate { .. }) {
        emitter.copy_value(&caller, &staged, "caller result publication")?;
    } else {
        let Value::Scalar { local: source, ty } = staged else {
            return Err(error(
                "scalar caller publication received aggregate staging",
            ));
        };
        emitter.emit_pointer(Pointer {
            local: plan.result_out,
            offset: 0,
        });
        emitter.output.push(0x20);
        write_u32(emitter.output, source);
        emitter.store_scalar(&ty);
    }
    drop(emitter);
    body.push(0x0b);
    body.push(0x20);
    write_u32(&mut body, plan.old_stack);
    body.push(0x24);
    write_u32(&mut body, 0);
    body.push(0x20);
    write_u32(&mut body, plan.status);
    body.push(0x0b);
    Ok(body)
}

fn emit_external_byte_root_admission(
    body: &mut Vec<u8>,
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    old_stack: u32,
    charged: u32,
) -> Result<(), Diagnostic> {
    let roots = function
        .params
        .iter()
        .enumerate()
        .filter(|(_, param)| matches!(param.ty, ResolvedType::SliceU8 | ResolvedType::Str))
        .map(|(index, param)| {
            if param.ty == ResolvedType::SliceU8 {
                let provenance = program
                    .declarations
                    .byte_slice_provenance(&param.id)
                    .ok_or_else(|| error("external Slice<u8> parameter lacks provenance"))?;
                if provenance.root != param.id
                    || provenance.root_kind != crate::hir::ByteSliceRootKind::FunctionParameter
                    || provenance.root_length != crate::hir::ByteSliceExtent::ParameterLength
                    || provenance.offset != crate::hir::ByteSliceExtent::Constant(0)
                    || provenance.length != crate::hir::ByteSliceExtent::ParameterLength
                {
                    return Err(error(
                        "external Slice<u8> parameter provenance is not an exact full root",
                    ));
                }
            }
            u32::try_from(index).map_err(|_| error("byte root parameter index overflows u32"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if roots.is_empty() {
        return Ok(());
    }

    // A direct test/callable export observes the untouched data-profile stack
    // top. Internal forwarding runs below it and must not recharge roots.
    body.push(0x20);
    write_u32(body, old_stack);
    body.push(0x41);
    write_i64(body, 131_072);
    body.extend([0x46, 0x04, 0x40]);
    body.extend([0x42, 0x00, 0x21]);
    write_u32(body, charged);
    for parameter in roots {
        // len <= 65536, root tag is zero, and offset <= 65536 - len.
        body.push(0x20);
        write_u32(body, parameter);
        body.extend([0xa7, 0xad, 0x42]);
        write_i64(body, crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES as i64);
        body.push(0x58);
        body.push(0x20);
        write_u32(body, parameter);
        body.extend([0x42, 0x20, 0x88, 0x42]);
        write_i64(body, 0x8000_0000u32 as i64);
        body.extend([0x83, 0x50]);
        body.push(0x20);
        write_u32(body, parameter);
        body.extend([0x42, 0x20, 0x88, 0xa7, 0xad, 0x42]);
        write_i64(body, crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES as i64);
        body.push(0x20);
        write_u32(body, parameter);
        body.extend([
            0xa7, 0xad, 0x7d, 0x58, 0x71, 0x71, 0x45, 0x04, 0x40, 0x00, 0x0b,
        ]);

        // charged <= 65536 - len; only then add the distinct parameter root.
        body.push(0x20);
        write_u32(body, charged);
        body.push(0x42);
        write_i64(body, crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES as i64);
        body.push(0x20);
        write_u32(body, parameter);
        body.extend([0xa7, 0xad, 0x7d, 0x58, 0x45, 0x04, 0x40, 0x00, 0x0b]);
        body.push(0x20);
        write_u32(body, charged);
        body.push(0x20);
        write_u32(body, parameter);
        body.extend([0xa7, 0xad, 0x7c, 0x21]);
        write_u32(body, charged);
    }
    body.push(0x0b);
    Ok(())
}

struct Emitter<'a> {
    output: &'a mut Vec<u8>,
    program: &'a ResolvedProgram,
    variant_layouts: &'a VariantLayoutCache,
    function_indexes: &'a HashMap<FunctionExecutionId, u32>,
    plan: &'a FunctionPlan,
    return_type: &'a ResolvedType,
    cleanup_plan: &'a crate::cleanup_plan::CleanupPlan,
    bindings: HashMap<ValueId, Value>,
    control_depth: u32,
    status_exit_extra_depth: u32,
    try_target_enabled: bool,
    failure_expression: Option<ExpressionId>,
    host_output: Option<super::host_output::Globals>,
}

impl Emitter<'_> {
    fn emit_expr(&mut self, expr: &ResolvedExpr) -> Result<Value, Diagnostic> {
        let value = self.emit_expr_inner(expr)?;
        self.apply_post_transitions(&expr.id, &value)?;
        Ok(value)
    }

    fn emit_success_cleanup(
        &mut self,
        plan: &crate::cleanup_plan::CleanupPlan,
    ) -> Result<(), Diagnostic> {
        let mut commits = plan.exits.iter().filter(|exit| {
            matches!(
                exit.continuation,
                crate::cleanup_plan::ExitContinuation::CommitResult { .. }
            )
        });
        let commit = commits
            .next()
            .ok_or_else(|| error("CleanupPlan has no successful result commit"))?;
        if commits.next().is_some() {
            return Err(error("CleanupPlan has multiple successful result commits"));
        }
        self.emit_cleanup_actions(&commit.finalize_in_order)
    }

    fn emit_failure_cleanup(&mut self, expression: &ExpressionId) -> Result<(), Diagnostic> {
        let mut selected = None;
        for exit in &self.cleanup_plan.exits {
            if let crate::cleanup_plan::ExitContinuation::ReturnFailure { source } =
                &exit.continuation
            {
                if &source.expression == expression {
                    if selected.is_some() {
                        return Err(error("CleanupPlan repeats one failure exit source"));
                    }
                    selected = Some(exit.finalize_in_order.clone());
                }
            }
        }
        let actions = selected.ok_or_else(|| {
            error(format!(
                "CleanupPlan has no exact failure exit for `{expression}`"
            ))
        })?;
        self.emit_cleanup_actions(&actions)
    }

    fn emit_cleanup_actions(
        &mut self,
        actions: &[crate::cleanup_plan::FinalizeAction],
    ) -> Result<(), Diagnostic> {
        for action in actions {
            if action.lifecycle_id.as_str() != crate::cleanup::BYTES_DROP_LIFECYCLE_ID
                || !action.source.projections.is_empty()
            {
                return Err(error(
                    "byte-data WebAssembly cleanup requires direct compiler-owned Bytes leaves",
                ));
            }
            let local = match &action.source.storage {
                crate::cleanup_plan::StorageId::Value(value) => {
                    self.plan.scalar_bindings.get(value).copied().or_else(|| {
                        self.bindings.get(value).and_then(|value| match value {
                            Value::Scalar { local, ty } if *ty == ResolvedType::Bytes => {
                                Some(*local)
                            }
                            _ => None,
                        })
                    })
                }
                crate::cleanup_plan::StorageId::Temporary(expression) => {
                    self.plan.scalar_expressions.get(expression).copied()
                }
                crate::cleanup_plan::StorageId::ProvisionalResult => self.plan.result_stage_scalar,
                crate::cleanup_plan::StorageId::CallArgument { .. } => self
                    .plan
                    .cleanup_call_argument_carriers
                    .get(&action.source.storage)
                    .copied(),
            }
            .ok_or_else(|| error("CleanupPlan Bytes finalizer has no exact Wasm carrier slot"))?;
            let flag = self
                .plan
                .cleanup_flags
                .get(&action.guard_flag)
                .copied()
                .ok_or_else(|| error("CleanupPlan finalizer guard has no exact Wasm local"))?;
            self.output.push(0x20);
            write_u32(self.output, flag);
            self.output.extend([0x04, 0x40]);
            self.output.push(0x20);
            write_u32(self.output, local);
            self.output.push(0x10);
            write_u32(self.output, BYTE_DROP_IMPORT);
            // Poison the moved/dropped carrier locally. Any backend mistake
            // that reads it later reaches the host's malformed-token trap.
            self.output.extend([0x42, 0x00, 0x21]);
            write_u32(self.output, local);
            self.output.extend([0x41, 0x00, 0x21]);
            write_u32(self.output, flag);
            self.output.push(0x0b);
        }
        Ok(())
    }

    fn emit_block_scope_cleanup(
        &mut self,
        statements: &[ResolvedStatement],
    ) -> Result<(), Diagnostic> {
        let anchors = statements
            .iter()
            .flat_map(|statement| {
                let mut anchors = Vec::with_capacity(2);
                if let ResolvedStatement::Let { binding, .. } = statement {
                    if binding.ty == ResolvedType::Bytes {
                        anchors.push(crate::cleanup_plan::StorageId::Value(binding.id.clone()));
                    }
                }
                let value = match statement {
                    ResolvedStatement::Let { value, .. }
                    | ResolvedStatement::Assign { value, .. } => Some(value),
                    ResolvedStatement::Unsafe { body, .. } => Some(body.as_ref()),
                    ResolvedStatement::While { .. } => None,
                };
                if let Some(value) = value.filter(|value| value.ty == ResolvedType::Bytes) {
                    anchors.push(crate::cleanup_plan::StorageId::Temporary(value.id.clone()));
                }
                anchors
            })
            .collect::<std::collections::BTreeSet<_>>();
        if anchors.is_empty() {
            return Ok(());
        }
        let mut regions = self
            .cleanup_plan
            .regions
            .iter()
            .filter(|region| region.slots.iter().any(|slot| anchors.contains(slot)));
        let region = regions
            .next()
            .ok_or_else(|| error("Bytes block has no authenticated CleanupPlan region"))?;
        if regions.next().is_some() {
            return Err(error("Bytes block maps to multiple CleanupPlan regions"));
        }
        // The function body occupies the root region. Its owned values remain
        // live through postconditions and are finalized by CommitResult or an
        // exact failure exit, never at the syntactic end of the body block.
        if region.parent.is_none() {
            return Ok(());
        }
        let exit = self
            .cleanup_plan
            .exits
            .get(region.normal_scope_end.0 as usize)
            .filter(|exit| exit.id == region.normal_scope_end)
            .ok_or_else(|| error("Bytes block region has no exact normal-scope exit"))?;
        if !matches!(
            exit.continuation,
            crate::cleanup_plan::ExitContinuation::Continue(_)
        ) || exit.leaves_regions.as_slice() != [region.id]
        {
            return Err(error(
                "Bytes block region normal-scope exit is not canonical",
            ));
        }
        self.emit_cleanup_actions(&exit.finalize_in_order)
    }

    fn set_storage_flag(
        &mut self,
        place: &crate::cleanup_plan::CleanupPlace,
        live: bool,
    ) -> Result<(), Diagnostic> {
        if !place.projections.is_empty() {
            return Err(error(
                "Bytes transition addresses a projected cleanup place",
            ));
        }
        let local = self
            .plan
            .cleanup_storage_flags
            .get(&place.storage)
            .copied()
            .ok_or_else(|| error("Bytes transition storage has no exact liveness local"))?;
        self.output.extend([0x41, u8::from(live), 0x21]);
        write_u32(self.output, local);
        Ok(())
    }

    fn apply_call_commit(&mut self, expression: &ExpressionId) -> Result<(), Diagnostic> {
        let transitions = self
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .filter_map(|transition| match transition {
                crate::cleanup_plan::CleanupTransition::CallCommit { call, arguments }
                    if call == expression =>
                {
                    Some(arguments.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for arguments in transitions {
            for argument in arguments {
                self.set_storage_flag(&argument.source, false)?;
            }
        }
        Ok(())
    }

    fn apply_post_transitions(
        &mut self,
        expression: &ExpressionId,
        value: &Value,
    ) -> Result<(), Diagnostic> {
        let transitions = self
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .filter(|transition| match transition {
                crate::cleanup_plan::CleanupTransition::Initialize { at, .. }
                | crate::cleanup_plan::CleanupTransition::Transfer { at, .. } => at == expression,
                _ => false,
            })
            .cloned()
            .collect::<Vec<_>>();
        for transition in transitions {
            match transition {
                crate::cleanup_plan::CleanupTransition::Initialize { destination, .. } => {
                    self.set_storage_flag(&destination, true)?;
                }
                crate::cleanup_plan::CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                } => {
                    if let Some(local) = self
                        .plan
                        .cleanup_call_argument_carriers
                        .get(&destination.storage)
                        .copied()
                    {
                        require_type(value_type(value), &ResolvedType::Bytes, "owned call epoch")?;
                        self.get_scalar(value);
                        self.output.push(0x21);
                        write_u32(self.output, local);
                    }
                    self.set_storage_flag(&source, false)?;
                    self.set_storage_flag(&destination, true)?;
                }
                _ => unreachable!("filtered byte cleanup transition"),
            }
        }
        Ok(())
    }

    fn emit_expr_inner(&mut self, expr: &ResolvedExpr) -> Result<Value, Diagnostic> {
        match &expr.kind {
            ResolvedExprKind::Int(value) => {
                let destination = self.plan.expr_scalar(expr)?;
                self.output.push(0x42);
                write_i64(self.output, *value);
                self.output.push(0x21);
                write_u32(self.output, destination);
                Ok(Value::Scalar {
                    local: destination,
                    ty: ResolvedType::I64,
                })
            }
            ResolvedExprKind::Int32(value) => {
                let destination = self.plan.expr_scalar(expr)?;
                self.output.push(0x41);
                write_i64(self.output, i64::from(*value));
                self.output.push(0x21);
                write_u32(self.output, destination);
                Ok(Value::Scalar {
                    local: destination,
                    ty: ResolvedType::I32,
                })
            }
            ResolvedExprKind::Char(value) => {
                let destination = self.plan.expr_scalar(expr)?;
                self.output.push(0x41);
                write_i64(self.output, i64::from(*value));
                self.output.push(0x21);
                write_u32(self.output, destination);
                Ok(Value::Scalar {
                    local: destination,
                    ty: ResolvedType::Char,
                })
            }
            ResolvedExprKind::Uint8(value) => {
                let destination = self.plan.expr_scalar(expr)?;
                self.output.push(0x41);
                write_i64(self.output, i64::from(*value));
                self.output.push(0x21);
                write_u32(self.output, destination);
                Ok(Value::Scalar {
                    local: destination,
                    ty: ResolvedType::U8,
                })
            }
            ResolvedExprKind::Usize(value) => {
                let destination = self.plan.expr_scalar(expr)?;
                self.output.push(0x42);
                write_i64(self.output, *value as i64);
                self.output.push(0x21);
                write_u32(self.output, destination);
                Ok(Value::Scalar {
                    local: destination,
                    ty: ResolvedType::Usize,
                })
            }
            ResolvedExprKind::ArrayU8(values) => self.emit_array_literal(expr, values, None),
            ResolvedExprKind::RepeatArrayU8 { value, count } => {
                self.emit_array_literal(expr, &[], Some((*value, *count)))
            }
            ResolvedExprKind::Float32(bits) => {
                let destination = self.plan.expr_scalar(expr)?;
                self.output.push(0x43);
                self.output.extend_from_slice(&bits.to_le_bytes());
                self.output.push(0x21);
                write_u32(self.output, destination);
                Ok(Value::Scalar {
                    local: destination,
                    ty: ResolvedType::F32,
                })
            }
            ResolvedExprKind::Float64(bits) => {
                let destination = self.plan.expr_scalar(expr)?;
                self.output.push(0x44);
                self.output.extend_from_slice(&bits.to_le_bytes());
                self.output.push(0x21);
                write_u32(self.output, destination);
                Ok(Value::Scalar {
                    local: destination,
                    ty: ResolvedType::F64,
                })
            }
            ResolvedExprKind::Bool(value) => {
                let destination = self.plan.expr_scalar(expr)?;
                self.output.push(0x41);
                write_i64(self.output, i64::from(*value));
                self.output.push(0x21);
                write_u32(self.output, destination);
                Ok(Value::Scalar {
                    local: destination,
                    ty: ResolvedType::Bool,
                })
            }
            ResolvedExprKind::String(value) => Err(error(format!(
                "string literal `{value}` is outside aggregate WebAssembly lowering"
            ))),
            ResolvedExprKind::Place(place) => {
                let value = self.place_value(place)?;
                self.materialize(expr, &value)
            }
            ResolvedExprKind::BorrowPlace { operation, place } => {
                self.emit_borrow_place(expr, operation, place)
            }
            ResolvedExprKind::Call {
                callee,
                instance,
                args,
                ..
            } => self.emit_call(expr, callee, instance.as_ref(), args),
            ResolvedExprKind::NativeRustImportCall(_) => Err(Diagnostic::io(
                "SPX-W114",
                "Native Rust imports are unavailable for WebAssembly targets",
            )),
            ResolvedExprKind::HostCommandCall(call) => self.emit_host_command_call(expr, call),
            ResolvedExprKind::Unary { op, value } => self.emit_unary(expr, *op, value),
            ResolvedExprKind::Binary { op, left, right } => {
                self.emit_binary(expr, *op, left, right)
            }
            ResolvedExprKind::Block { statements, tail } => {
                let saved = self.bindings.clone();
                for statement in statements {
                    // Field Mutation v1: the assigned value evaluates fully
                    // first, then stores into the direct scalar field of the
                    // aggregate binding's frame slot.
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
                            .ok_or_else(|| {
                                error(format!("missing aggregate binding `{}`", binding.id))
                            })?;
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
                        continue;
                    }
                    // Lets declare and store; assignments re-store into the
                    // same scalar or aggregate slot. Unsafe boundaries emit
                    // their ordinary body transparently and bind nothing.
                    let (ResolvedStatement::Let { binding, .. }
                    | ResolvedStatement::Assign { binding, .. }) = statement
                    else {
                        if let ResolvedStatement::While {
                            condition, body, ..
                        } = statement
                        {
                            self.emit_while(condition, body)?;
                        } else {
                            self.emit_expr(statement.value())?;
                        }
                        continue;
                    };
                    let value = self.emit_expr(statement.value())?;
                    let destination = if is_aggregate(self.program, &binding.ty)? {
                        let offset = self
                            .plan
                            .aggregate_bindings
                            .get(&binding.id)
                            .copied()
                            .ok_or_else(|| {
                                error(format!("missing aggregate binding `{}`", binding.id))
                            })?;
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
                            .ok_or_else(|| {
                                error(format!("missing scalar binding `{}`", binding.id))
                            })?;
                        Value::Scalar {
                            local,
                            ty: binding.ty.clone(),
                        }
                    };
                    self.copy_value(&destination, &value, "local binding")?;
                    self.bindings.insert(binding.id.clone(), destination);
                }
                let tail = self.emit_expr(tail)?;
                let result = self.materialize(expr, &tail)?;
                self.emit_block_scope_cleanup(statements)?;
                self.bindings = saved;
                Ok(result)
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.emit_if(expr, condition, then_branch, else_branch),
            ResolvedExprKind::ConstructRecord { record, fields } => {
                let destination = Value::Aggregate {
                    pointer: self.plan.expr_pointer(expr)?,
                    ty: expr.ty.clone(),
                };
                let record_layout = layout(self.program, &expr.ty)?;
                if record_layout.record != *record {
                    return Err(error(format!(
                        "constructor `{record}` has result type `{}`",
                        expr.ty.identity_key()
                    )));
                }
                let Value::Aggregate { pointer, .. } = &destination else {
                    unreachable!();
                };
                for initializer in fields {
                    let field = record_layout
                        .field(&initializer.field)
                        .cloned()
                        .ok_or_else(|| {
                            error(format!("unknown constructor field `{}`", initializer.field))
                        })?;
                    let value = self.emit_expr(&initializer.value)?;
                    let field_destination = value_at(
                        Pointer {
                            local: pointer.local,
                            offset: pointer
                                .offset
                                .checked_add(field.offset)
                                .ok_or_else(|| error("field pointer overflows u32"))?,
                        },
                        field.ty,
                        self.program,
                    )?;
                    self.copy_value(&field_destination, &value, "record field initializer")?;
                }
                Ok(destination)
            }
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                let layout = variant_layout(self.variant_layouts, &expr.ty)?;
                if layout.variant != *variant {
                    return Err(error(format!(
                        "variant constructor `{variant}` has result type `{}`",
                        expr.ty.identity_key()
                    )));
                }
                let case_layout = layout
                    .case(case)
                    .cloned()
                    .ok_or_else(|| error(format!("variant `{variant}` has no case `{case}`")))?;
                let mut values = Vec::with_capacity(fields.len());
                for initializer in fields {
                    let field =
                        case_layout
                            .field(&initializer.field)
                            .cloned()
                            .ok_or_else(|| {
                                error(format!(
                                    "variant case `{case}` has no field `{}`",
                                    initializer.field
                                ))
                            })?;
                    let value = self.emit_expr(&initializer.value)?;
                    require_type(value_type(&value), &field.ty, "variant field initializer")?;
                    values.push((field, value));
                }
                let destination = Value::Aggregate {
                    pointer: self.plan.expr_pointer(expr)?,
                    ty: expr.ty.clone(),
                };
                let Value::Aggregate { pointer, .. } = &destination else {
                    unreachable!();
                };
                self.emit_pointer(*pointer);
                self.output.extend([0x41, 0x00, 0x41]);
                write_i64(self.output, i64::from(layout.size));
                self.output.extend([0xfc, 0x0b, 0x00]);
                for (field, value) in values {
                    let destination = value_at(
                        Pointer {
                            local: pointer.local,
                            offset: pointer
                                .offset
                                .checked_add(layout.payload_offset)
                                .and_then(|offset| offset.checked_add(field.offset))
                                .ok_or_else(|| error("variant field pointer overflows u32"))?,
                        },
                        field.ty,
                        self.program,
                    )?;
                    self.copy_value(&destination, &value, "variant field initializer")?;
                }
                self.emit_pointer(*pointer);
                self.output.push(0x41);
                write_i64(self.output, i64::from(case_layout.tag));
                self.output.extend([0x36, 0x02, 0x00]);
                Ok(destination)
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                if is_aggregate(self.program, &expr.ty)? {
                    return Err(error("copy match result must be i64 or bool"));
                }
                let scrutinee = self.emit_expr(scrutinee)?;
                // Refutable Match v1: Copy-scalar scrutinees lower to the
                // literal/guard decision chain even on the aggregate lane;
                // aggregate storage keeps the pre-feature lowering below.
                if matches!(
                    value_type(&scrutinee),
                    ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::U8
                        | ResolvedType::Usize
                        | ResolvedType::Char
                        | ResolvedType::Bool
                ) {
                    return self.emit_scalar_refutable_match(expr, &scrutinee, arms);
                }
                let Value::Aggregate { pointer, ty } = &scrutinee else {
                    return Err(error("match scrutinee is not aggregate storage"));
                };
                if is_record(self.program, ty)? {
                    let [arm] = arms.as_slice() else {
                        return Err(error("irrefutable record match must have exactly one arm"));
                    };
                    let destination = Value::Scalar {
                        local: self.plan.expr_scalar(expr)?,
                        ty: expr.ty.clone(),
                    };
                    let saved = self.bindings.clone();
                    match &arm.pattern {
                        crate::hir::ResolvedMatchPattern::Wildcard => {}
                        crate::hir::ResolvedMatchPattern::Record {
                            record,
                            instance,
                            fields,
                        } => {
                            self.bind_record_match_pattern(&scrutinee, record, instance, fields)?
                        }
                        crate::hir::ResolvedMatchPattern::Variant { .. } => {
                            return Err(error("variant pattern has a record match scrutinee"));
                        }
                        crate::hir::ResolvedMatchPattern::Literal(_)
                        | crate::hir::ResolvedMatchPattern::Or(_)
                        | crate::hir::ResolvedMatchPattern::Binding(_) => {
                            return Err(error(
                                "refutable pattern has an aggregate record match scrutinee",
                            ));
                        }
                    }
                    let value = self.emit_expr(&arm.value)?;
                    self.copy_value(&destination, &value, "record match arm result")?;
                    self.bindings = saved;
                    return Ok(destination);
                }
                let layout = variant_layout(self.variant_layouts, ty)?;
                self.emit_pointer(*pointer);
                self.output.extend([0x28, 0x02, 0x00, 0x41]);
                write_i64(
                    self.output,
                    i64::try_from(layout.cases.len())
                        .map_err(|_| error("variant case count overflows i64"))?,
                );
                self.output.push(0x4f);
                self.fail_if(STATUS_INTERNAL_INVALID_TAG)?;
                let destination = Value::Scalar {
                    local: self.plan.expr_scalar(expr)?,
                    ty: expr.ty.clone(),
                };
                self.emit_match_arms(&destination, *pointer, &layout, arms, 0)?;
                Ok(destination)
            }
            ResolvedExprKind::Try {
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
            } => {
                if !self.try_target_enabled {
                    return Err(error(
                        "copy-result propagation is allowed only in a function body",
                    ));
                }
                require_type(
                    residual_type,
                    self.return_type,
                    "copy-result residual target",
                )?;
                let operand_layout = variant_layout(self.variant_layouts, &operand.ty)?;
                let residual_layout = variant_layout(self.variant_layouts, residual_type)?;
                if operand_layout.variant != *result || residual_layout.variant != *result {
                    return Err(error(
                        "copy-result propagation does not reference its resolved Result declaration",
                    ));
                }
                let operand_ok = operand_layout
                    .case(ok_case)
                    .and_then(|case| case.field(ok_field).map(|field| (case, field)))
                    .ok_or_else(|| error("copy-result propagation has no resolved Ok payload"))?;
                let operand_err = operand_layout
                    .case(err_case)
                    .and_then(|case| case.field(err_field).map(|field| (case, field)))
                    .ok_or_else(|| error("copy-result propagation has no resolved Err payload"))?;
                let residual_err = residual_layout
                    .case(err_case)
                    .and_then(|case| case.field(err_field).map(|field| (case, field)))
                    .ok_or_else(|| error("copy-result residual has no resolved Err payload"))?;
                require_type(&operand_ok.1.ty, &expr.ty, "copy-result Ok payload")?;
                require_type(
                    &operand_err.1.ty,
                    &residual_err.1.ty,
                    "copy-result Err payload",
                )?;

                let operand_value = self.emit_expr(operand)?;
                let Value::Aggregate {
                    pointer: operand_pointer,
                    ty: operand_type,
                } = operand_value
                else {
                    return Err(error("copy-result operand is not aggregate storage"));
                };
                require_type(&operand_type, &operand.ty, "copy-result operand")?;
                self.emit_pointer(operand_pointer);
                self.output.extend([0x28, 0x02, 0x00, 0x41]);
                write_i64(
                    self.output,
                    i64::try_from(operand_layout.cases.len())
                        .map_err(|_| error("Result case count overflows i64"))?,
                );
                self.output.push(0x4f);
                self.fail_if(STATUS_INTERNAL_INVALID_TAG)?;

                self.emit_pointer(operand_pointer);
                self.output.extend([0x28, 0x02, 0x00, 0x41]);
                write_i64(self.output, i64::from(operand_err.0.tag));
                self.output.extend([0x46, 0x04, 0x40]);
                let residual_offset = self
                    .plan
                    .result_stage_aggregate
                    .ok_or_else(|| error("copy-result residual has no result staging slot"))?;
                let residual_pointer = Pointer {
                    local: self.plan.frame_base,
                    offset: residual_offset,
                };
                self.emit_pointer(residual_pointer);
                self.output.extend([0x41, 0x00, 0x41]);
                write_i64(self.output, i64::from(residual_layout.size));
                self.output.extend([0xfc, 0x0b, 0x00]);
                let source = Value::ScalarMemory {
                    pointer: Pointer {
                        local: operand_pointer.local,
                        offset: operand_pointer
                            .offset
                            .checked_add(operand_layout.payload_offset)
                            .and_then(|offset| offset.checked_add(operand_err.1.offset))
                            .ok_or_else(|| error("Result Err source pointer overflows u32"))?,
                    },
                    ty: operand_err.1.ty.clone(),
                };
                let destination = Value::ScalarMemory {
                    pointer: Pointer {
                        local: residual_pointer.local,
                        offset: residual_pointer
                            .offset
                            .checked_add(residual_layout.payload_offset)
                            .and_then(|offset| offset.checked_add(residual_err.1.offset))
                            .ok_or_else(|| error("Result Err destination pointer overflows u32"))?,
                    },
                    ty: residual_err.1.ty.clone(),
                };
                self.copy_value(&destination, &source, "copy-result Err reconstruction")?;
                self.emit_pointer(residual_pointer);
                self.output.push(0x41);
                write_i64(self.output, i64::from(residual_err.0.tag));
                self.output.extend([0x36, 0x02, 0x00]);
                let result_staged = self
                    .plan
                    .result_staged
                    .ok_or_else(|| error("copy-result propagation has no result-state local"))?;
                self.output.extend([0x41, 0x01, 0x21]);
                write_u32(self.output, result_staged);
                self.output.push(0x0c);
                write_u32(self.output, self.control_depth + 1);
                self.output.push(0x0b);

                self.emit_pointer(operand_pointer);
                self.output.extend([0x28, 0x02, 0x00, 0x41]);
                write_i64(self.output, i64::from(operand_ok.0.tag));
                self.output.push(0x47);
                self.fail_if(STATUS_INTERNAL_INVALID_TAG)?;
                let destination = Value::Scalar {
                    local: self.plan.expr_scalar(expr)?,
                    ty: expr.ty.clone(),
                };
                let source = Value::ScalarMemory {
                    pointer: Pointer {
                        local: operand_pointer.local,
                        offset: operand_pointer
                            .offset
                            .checked_add(operand_layout.payload_offset)
                            .and_then(|offset| offset.checked_add(operand_ok.1.offset))
                            .ok_or_else(|| error("Result Ok pointer overflows u32"))?,
                    },
                    ty: operand_ok.1.ty.clone(),
                };
                self.copy_value(&destination, &source, "copy-result Ok extraction")?;
                Ok(destination)
            }
            ResolvedExprKind::TryOption {
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
            } => {
                if !self.try_target_enabled {
                    return Err(error(
                        "copy-Option propagation is allowed only in a function body",
                    ));
                }
                require_type(
                    residual_type,
                    self.return_type,
                    "copy-Option residual target",
                )?;
                let operand_layout = variant_layout(self.variant_layouts, &operand.ty)?;
                let residual_layout = variant_layout(self.variant_layouts, residual_type)?;
                if operand_layout.variant != *option || residual_layout.variant != *option {
                    return Err(error(
                        "copy-Option propagation does not reference its resolved Option declaration",
                    ));
                }
                let operand_some = operand_layout
                    .case(some_case)
                    .and_then(|case| case.field(some_field).map(|field| (case, field)))
                    .ok_or_else(|| error("copy-Option propagation has no resolved Some payload"))?;
                let operand_none = operand_layout
                    .case(none_case)
                    .ok_or_else(|| error("copy-Option propagation has no resolved None case"))?;
                let residual_none = residual_layout
                    .case(none_case)
                    .ok_or_else(|| error("copy-Option residual has no resolved None case"))?;
                if !operand_none.fields.is_empty() || !residual_none.fields.is_empty() {
                    return Err(error("copy-Option None case unexpectedly has a payload"));
                }
                require_type(&operand_some.1.ty, &expr.ty, "copy-Option Some payload")?;

                let operand_value = self.emit_expr(operand)?;
                let Value::Aggregate {
                    pointer: operand_pointer,
                    ty: operand_type,
                } = operand_value
                else {
                    return Err(error("copy-Option operand is not aggregate storage"));
                };
                require_type(&operand_type, &operand.ty, "copy-Option operand")?;
                self.emit_pointer(operand_pointer);
                self.output.extend([0x28, 0x02, 0x00, 0x41]);
                write_i64(
                    self.output,
                    i64::try_from(operand_layout.cases.len())
                        .map_err(|_| error("Option case count overflows i64"))?,
                );
                self.output.push(0x4f);
                self.fail_if(STATUS_INTERNAL_INVALID_TAG)?;

                self.emit_pointer(operand_pointer);
                self.output.extend([0x28, 0x02, 0x00, 0x41]);
                write_i64(self.output, i64::from(operand_none.tag));
                self.output.extend([0x46, 0x04, 0x40]);
                let residual_offset = self
                    .plan
                    .result_stage_aggregate
                    .ok_or_else(|| error("copy-Option residual has no result staging slot"))?;
                let residual_pointer = Pointer {
                    local: self.plan.frame_base,
                    offset: residual_offset,
                };
                self.emit_pointer(residual_pointer);
                self.output.extend([0x41, 0x00, 0x41]);
                write_i64(self.output, i64::from(residual_layout.size));
                self.output.extend([0xfc, 0x0b, 0x00]);
                self.emit_pointer(residual_pointer);
                self.output.push(0x41);
                write_i64(self.output, i64::from(residual_none.tag));
                self.output.extend([0x36, 0x02, 0x00]);
                let result_staged = self
                    .plan
                    .result_staged
                    .ok_or_else(|| error("copy-Option propagation has no result-state local"))?;
                self.output.extend([0x41, 0x01, 0x21]);
                write_u32(self.output, result_staged);
                self.output.push(0x0c);
                write_u32(self.output, self.control_depth + 1);
                self.output.push(0x0b);

                self.emit_pointer(operand_pointer);
                self.output.extend([0x28, 0x02, 0x00, 0x41]);
                write_i64(self.output, i64::from(operand_some.0.tag));
                self.output.push(0x47);
                self.fail_if(STATUS_INTERNAL_INVALID_TAG)?;
                let destination = Value::Scalar {
                    local: self.plan.expr_scalar(expr)?,
                    ty: expr.ty.clone(),
                };
                let source = Value::ScalarMemory {
                    pointer: Pointer {
                        local: operand_pointer.local,
                        offset: operand_pointer
                            .offset
                            .checked_add(operand_layout.payload_offset)
                            .and_then(|offset| offset.checked_add(operand_some.1.offset))
                            .ok_or_else(|| error("Option Some pointer overflows u32"))?,
                    },
                    ty: operand_some.1.ty.clone(),
                };
                self.copy_value(&destination, &source, "copy-Option Some extraction")?;
                Ok(destination)
            }
            ResolvedExprKind::Project { base, field } => {
                let base = self.emit_expr(base)?;
                let projected = self.project_value(&base, field)?;
                self.materialize(expr, &projected)
            }
            ResolvedExprKind::Upcast { source } => {
                // Class Inheritance v1: copy the ancestor prefix fields from
                // the consumed descendant value; canonical layouts guarantee
                // identical offsets on both sides.
                let value = self.emit_expr(source)?;
                let destination = Value::Aggregate {
                    pointer: self.plan.expr_pointer(expr)?,
                    ty: expr.ty.clone(),
                };
                let target_layout = layout(self.program, &expr.ty)?;
                let source_layout = layout(self.program, value_type(&value))?;
                if target_layout.record == source_layout.record {
                    return Err(error("upcast requires a descendant source layout"));
                }
                let Value::Aggregate { pointer, .. } = &destination else {
                    unreachable!();
                };
                for field in &target_layout.fields {
                    if source_layout.field(&field.field).map(|candidate| {
                        (candidate.offset, candidate.size, candidate.align)
                            == (field.offset, field.size, field.align)
                    }) != Some(true)
                    {
                        return Err(error(
                            "upcast source prefix disagrees with the ancestor layout",
                        ));
                    }
                    let field_destination = value_at(
                        Pointer {
                            local: pointer.local,
                            offset: pointer
                                .offset
                                .checked_add(field.offset)
                                .ok_or_else(|| error("field pointer overflows u32"))?,
                        },
                        field.ty.clone(),
                        self.program,
                    )?;
                    let field_source = self.project_value(&value, &field.field)?;
                    self.copy_value(&field_destination, &field_source, "upcast prefix field")?;
                }
                Ok(destination)
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                let base = self.emit_expr(base)?;
                let destination = Value::Aggregate {
                    pointer: self.plan.expr_pointer(expr)?,
                    ty: expr.ty.clone(),
                };
                self.copy_value(&destination, &base, "record update base")?;
                let record_layout = layout(self.program, &expr.ty)?;
                if record_layout.record != *record {
                    return Err(error(format!(
                        "record update `{record}` has result type `{}`",
                        expr.ty.identity_key()
                    )));
                }
                let Value::Aggregate { pointer, .. } = &destination else {
                    unreachable!();
                };
                for replacement in fields {
                    let field = record_layout
                        .field(&replacement.field)
                        .cloned()
                        .ok_or_else(|| {
                            error(format!("unknown update field `{}`", replacement.field))
                        })?;
                    let value = self.emit_expr(&replacement.value)?;
                    let field_destination = value_at(
                        Pointer {
                            local: pointer.local,
                            offset: pointer
                                .offset
                                .checked_add(field.offset)
                                .ok_or_else(|| error("field pointer overflows u32"))?,
                        },
                        field.ty,
                        self.program,
                    )?;
                    self.copy_value(&field_destination, &value, "record update field")?;
                }
                Ok(destination)
            }
        }
    }

    fn materialize(&mut self, expr: &ResolvedExpr, source: &Value) -> Result<Value, Diagnostic> {
        let destination = if is_aggregate(self.program, &expr.ty)? {
            Value::Aggregate {
                pointer: self.plan.expr_pointer(expr)?,
                ty: expr.ty.clone(),
            }
        } else {
            Value::Scalar {
                local: self.plan.expr_scalar(expr)?,
                ty: expr.ty.clone(),
            }
        };
        self.copy_value(&destination, source, "expression materialization")?;
        Ok(destination)
    }

    fn emit_match_arms(
        &mut self,
        destination: &Value,
        scrutinee: Pointer,
        layout: &VariantLayout,
        arms: &[crate::hir::ResolvedMatchArm],
        index: usize,
    ) -> Result<(), Diagnostic> {
        let Some(arm) = arms.get(index) else {
            self.output.push(0x00);
            return Ok(());
        };
        let saved = self.bindings.clone();
        if arm.guard.is_some() {
            return Err(error("guards are outside aggregate match lowering"));
        }
        match &arm.pattern {
            crate::hir::ResolvedMatchPattern::Variant {
                variant,
                case,
                fields,
            } => {
                if *variant != layout.variant {
                    return Err(error(format!(
                        "match arm variant `{variant}` disagrees with `{}`",
                        layout.variant
                    )));
                }
                let case_layout = layout
                    .case(case)
                    .cloned()
                    .ok_or_else(|| error(format!("match arm references unknown case `{case}`")))?;
                self.emit_pointer(scrutinee);
                self.output.extend([0x28, 0x02, 0x00, 0x41]);
                write_i64(self.output, i64::from(case_layout.tag));
                self.output.extend([0x46, 0x04, 0x40]);
                self.control_depth += 1;
                for pattern_field in fields {
                    let field = case_layout
                        .field(&pattern_field.field)
                        .cloned()
                        .ok_or_else(|| {
                            error(format!(
                                "match case `{case}` has no field `{}`",
                                pattern_field.field
                            ))
                        })?;
                    require_type(
                        &pattern_field.binding.ty,
                        &field.ty,
                        "match payload binding",
                    )?;
                    let pointer = Pointer {
                        local: scrutinee.local,
                        offset: scrutinee
                            .offset
                            .checked_add(layout.payload_offset)
                            .and_then(|offset| offset.checked_add(field.offset))
                            .ok_or_else(|| error("match payload pointer overflows u32"))?,
                    };
                    let local = self
                        .plan
                        .scalar_bindings
                        .get(&pattern_field.binding.id)
                        .copied()
                        .ok_or_else(|| {
                            error(format!(
                                "missing match binding `{}`",
                                pattern_field.binding.id
                            ))
                        })?;
                    self.emit_pointer(pointer);
                    self.load_scalar(&field.ty);
                    self.output.push(0x21);
                    write_u32(self.output, local);
                    self.bindings.insert(
                        pattern_field.binding.id.clone(),
                        Value::Scalar {
                            local,
                            ty: field.ty,
                        },
                    );
                }
                let value = self.emit_expr(&arm.value)?;
                self.copy_value(destination, &value, "match arm result")?;
                self.bindings = saved.clone();
                self.output.push(0x05);
                self.emit_match_arms(destination, scrutinee, layout, arms, index + 1)?;
                self.control_depth -= 1;
                self.output.push(0x0b);
            }
            crate::hir::ResolvedMatchPattern::Wildcard => {
                let value = self.emit_expr(&arm.value)?;
                self.copy_value(destination, &value, "wildcard match arm result")?;
            }
            crate::hir::ResolvedMatchPattern::Record { .. } => {
                return Err(error("record pattern has a variant match scrutinee"));
            }
            crate::hir::ResolvedMatchPattern::Literal(_)
            | crate::hir::ResolvedMatchPattern::Or(_)
            | crate::hir::ResolvedMatchPattern::Binding(_) => {
                return Err(error(
                    "refutable pattern has an aggregate variant match scrutinee",
                ));
            }
        }
        self.bindings = saved;
        Ok(())
    }

    /// Refutable Match v1 aggregate-lane lowering. The scrutinee is a scalar
    /// already staged in its own planned local, so every arm test re-reads it
    /// with `local.get` — one evaluation, many reads. Each arm nests one
    /// reject block whose `br_if 0` falls through to the following arms and
    /// one outer `br` that exits the whole chain after selection; a guard is
    /// an ordinary emitted bool expression short-circuited after the pattern
    /// test so it evaluates exactly once per reached matching arm.
    fn emit_scalar_refutable_match(
        &mut self,
        expr: &ResolvedExpr,
        scrutinee: &Value,
        arms: &[crate::hir::ResolvedMatchArm],
    ) -> Result<Value, Diagnostic> {
        let Value::Scalar {
            local: scrutinee_local,
            ty: scrutinee_ty,
        } = scrutinee
        else {
            return Err(error("scalar match scrutinee is not scalar storage"));
        };
        let destination = Value::Scalar {
            local: self.plan.expr_scalar(expr)?,
            ty: expr.ty.clone(),
        };
        // block $done (void): selecting any arm jumps here after storing.
        self.output.extend([0x02, 0x40]);
        self.control_depth += 1;
        for (index, arm) in arms.iter().enumerate() {
            let final_arm = index + 1 == arms.len();
            if !final_arm {
                // block $reject: `br_if 0` exits this arm only.
                self.output.extend([0x02, 0x40]);
                self.control_depth += 1;
                self.emit_scalar_pattern_test(scrutinee_local, scrutinee_ty, &arm.pattern)?;
                self.output.push(0x45); // i32.eqz
                self.output.extend([0x0d, 0x00]); // br_if 0 -> next arm
            }
            let saved = self.bindings.clone();
            if let crate::hir::ResolvedMatchPattern::Binding(binding) = &arm.pattern {
                let local = self
                    .plan
                    .scalar_bindings
                    .get(&binding.id)
                    .copied()
                    .ok_or_else(|| error(format!("missing match binding `{}`", binding.id)))?;
                require_type(&binding.ty, scrutinee_ty, "scalar match binding")?;
                self.output.push(0x20);
                write_u32(self.output, *scrutinee_local);
                self.output.push(0x21);
                write_u32(self.output, local);
                self.bindings.insert(
                    binding.id.clone(),
                    Value::Scalar {
                        local,
                        ty: binding.ty.clone(),
                    },
                );
            }
            if let Some(guard) = &arm.guard {
                // Guard false must fall through to the following arms: emit
                // the guard, invert, and branch to the reject label of this
                // same arm. The reject block is still the innermost label at
                // depth zero because selection has not happened yet.
                let flag = self.emit_expr(guard)?;
                require_type(value_type(&flag), &ResolvedType::Bool, "match guard")?;
                self.output.push(0x45); // i32.eqz
                self.output.extend([0x0d, 0x00]); // br_if 0 -> next arm
            }
            let value = self.emit_expr(&arm.value)?;
            self.copy_value(&destination, &value, "refutable match arm result")?;
            self.bindings = saved;
            if !final_arm {
                // Selecting this arm exits the whole chain. The only labels
                // enclosing this point are this arm's own reject block (0)
                // and `$done` (1); every earlier arm's block was already
                // closed. The trailing catch-all arm simply falls out of
                // `$done` without a branch.
                self.output.extend([0x0c, 0x01]); // br 1 -> $done
                self.output.push(0x0b); // end reject block
                self.control_depth -= 1;
            }
        }
        self.output.push(0x0b); // end $done
        self.control_depth -= 1;
        Ok(destination)
    }

    /// Pushes an i32 truth value: does the staged scalar equal any literal
    /// alternative of this pattern?
    fn emit_scalar_pattern_test(
        &mut self,
        scrutinee_local: &u32,
        scrutinee_ty: &ResolvedType,
        pattern: &crate::hir::ResolvedMatchPattern,
    ) -> Result<(), Diagnostic> {
        let alternatives: Vec<crate::hir::PatternValue> = match pattern {
            crate::hir::ResolvedMatchPattern::Literal(value) => vec![*value],
            crate::hir::ResolvedMatchPattern::Or(alternatives) => alternatives
                .iter()
                .map(|alternative| match alternative {
                    crate::hir::ResolvedMatchPattern::Literal(value) => *value,
                    _ => unreachable!("or-pattern alternatives are literals"),
                })
                .collect(),
            crate::hir::ResolvedMatchPattern::Wildcard
            | crate::hir::ResolvedMatchPattern::Binding(_) => {
                // Irrefutable pattern: constant true; a guard decides. An
                // unguarded irrefutable arm is the trailing catch-all, which
                // never reaches test emission.
                self.output.extend([0x41, 0x01]); // i32.const 1
                return Ok(());
            }
            crate::hir::ResolvedMatchPattern::Variant { .. }
            | crate::hir::ResolvedMatchPattern::Record { .. } => {
                return Err(error("aggregate pattern has a Copy-scalar match scrutinee"));
            }
        };
        for (position, value) in alternatives.iter().enumerate() {
            require_type(&value.ty(), scrutinee_ty, "literal pattern type")?;
            self.output.push(0x20);
            write_u32(self.output, *scrutinee_local);
            match (scrutinee_ty, value) {
                (ResolvedType::I64, crate::hir::PatternValue::Int(inner)) => {
                    self.output.push(0x42);
                    write_i64(self.output, *inner);
                    self.output.push(0x51); // i64.eq -> i32
                }
                (ResolvedType::I32, crate::hir::PatternValue::Int32(inner)) => {
                    self.output.push(0x41);
                    write_i64(self.output, i64::from(*inner));
                    self.output.push(0x46); // i32.eq
                }
                (ResolvedType::U8, crate::hir::PatternValue::Uint8(inner)) => {
                    self.output.push(0x41);
                    write_i64(self.output, i64::from(*inner));
                    self.output.push(0x46);
                }
                (ResolvedType::Usize, crate::hir::PatternValue::Usize(inner)) => {
                    self.output.push(0x42);
                    write_i64(self.output, *inner as i64);
                    self.output.push(0x51);
                }
                (ResolvedType::Char, crate::hir::PatternValue::Char(inner)) => {
                    self.output.push(0x41);
                    write_i64(self.output, i64::from(*inner));
                    self.output.push(0x46);
                }
                (ResolvedType::Bool, crate::hir::PatternValue::Bool(inner)) => {
                    self.output.push(0x41);
                    write_i64(self.output, i64::from(*inner));
                    self.output.push(0x46);
                }
                _ => return Err(error("literal pattern disagrees with its scrutinee type")),
            }
            if position != 0 {
                self.output.push(0x72); // i32.or combines equality flags
            }
        }
        Ok(())
    }

    fn bind_record_match_pattern(
        &mut self,
        base: &Value,
        record: &DeclarationId,
        instance: &ResolvedType,
        fields: &[crate::hir::ResolvedRecordMatchPatternField],
    ) -> Result<(), Diagnostic> {
        require_type(value_type(base), instance, "record pattern instance")?;
        let record_layout = layout(self.program, instance)?;
        if record_layout.record != *record || record_layout.fields.len() != fields.len() {
            return Err(error(
                "record pattern disagrees with its exact aggregate layout",
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for field in fields {
            if !seen.insert(field.field.clone()) {
                return Err(error(format!(
                    "record pattern `{record}` repeats field `{}`",
                    field.field
                )));
            }
            let projected = self.project_value(base, &field.field)?;
            match &field.pattern {
                crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                    require_type(
                        &binding.ty,
                        value_type(&projected),
                        "record pattern binding",
                    )?;
                    let destination = if is_aggregate(self.program, &binding.ty)? {
                        let offset = self
                            .plan
                            .aggregate_bindings
                            .get(&binding.id)
                            .copied()
                            .ok_or_else(|| {
                                error(format!(
                                    "missing aggregate record match binding `{}`",
                                    binding.id
                                ))
                            })?;
                        Value::Aggregate {
                            pointer: Pointer {
                                local: self.plan.frame_base,
                                offset,
                            },
                            ty: binding.ty.clone(),
                        }
                    } else {
                        Value::Scalar {
                            local: self
                                .plan
                                .scalar_bindings
                                .get(&binding.id)
                                .copied()
                                .ok_or_else(|| {
                                    error(format!(
                                        "missing scalar record match binding `{}`",
                                        binding.id
                                    ))
                                })?,
                            ty: binding.ty.clone(),
                        }
                    };
                    self.copy_value(&destination, &projected, "record pattern binding")?;
                    if self
                        .bindings
                        .insert(binding.id.clone(), destination)
                        .is_some()
                    {
                        return Err(error("record match binding is not fresh"));
                    }
                }
                crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
                crate::hir::ResolvedRecordMatchFieldPattern::Record {
                    record,
                    instance,
                    fields,
                } => self.bind_record_match_pattern(&projected, record, instance, fields)?,
            }
        }
        Ok(())
    }

    fn emit_command_transcript_write(
        &mut self,
        expression: &ExpressionId,
        carrier_local: u32,
        channel: super::host_output::Globals,
        other: super::host_output::Globals,
    ) -> Result<(), Diagnostic> {
        // len > capacity - other.len => sticky bounded-output failure.
        self.output.push(0x20);
        write_u32(self.output, carrier_local);
        self.output.push(0xa7);
        self.output.push(0x41);
        write_i64(
            self.output,
            i64::from(super::host_output::TRANSCRIPT_CAPACITY),
        );
        self.output.push(0x23);
        write_u32(self.output, other.staged_length);
        self.output.extend([0x6b, 0x4b]);
        self.emit_command_failure_if(expression, channel, other)?;

        // Copy while the source carrier is live. `spx_bytes_get` authenticates
        // both fixed-memory roots and tagged arena tokens; this avoids ever
        // reinterpreting token bits as a guest pointer.
        self.output.extend([0x41, 0x00, 0x21]);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x02, 0x40, 0x03, 0x40, 0x20]);
        write_u32(self.output, self.plan.status);
        self.output.push(0x20);
        write_u32(self.output, carrier_local);
        self.output.extend([0xa7, 0x4f, 0x0d, 0x01, 0x41]);
        write_i64(self.output, i64::from(channel.range_base));
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x6a, 0x20]);
        write_u32(self.output, carrier_local);
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.extend([0xad, 0x10]);
        write_u32(self.output, BYTE_GET_IMPORT);
        self.output.push(0x22);
        write_u32(
            self.output,
            self.plan
                .command_byte
                .ok_or_else(|| error("command transcript byte local is absent"))?,
        );
        self.output.extend([0x41]);
        write_i64(self.output, 255);
        self.output.extend([0x4b, 0x04, 0x40]);
        self.output.push(0x41);
        write_i64(self.output, i64::from(STATUS_INTERNAL_INVALID_TAG));
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x0c, 0x02, 0x0b, 0x20]);
        write_u32(
            self.output,
            self.plan
                .command_byte
                .ok_or_else(|| error("command transcript byte local is absent"))?,
        );
        self.output.extend([0x3a, 0x00, 0x00, 0x20]);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x41, 0x01, 0x6a, 0x21]);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x0c, 0x00, 0x0b, 0x0b]);
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.push(0x41);
        write_i64(self.output, i64::from(STATUS_INTERNAL_INVALID_TAG));
        self.output.push(0x46);
        self.emit_command_failure_if(expression, channel, other)?;
        self.output.push(0x20);
        write_u32(self.output, carrier_local);
        self.output.extend([0xa7, 0x24]);
        write_u32(self.output, channel.staged_length);
        self.output.push(0x20);
        write_u32(self.output, carrier_local);
        self.output.extend([0xa7, 0xad, 0x21]);
        write_u32(self.output, carrier_local);
        self.output.extend([0x41, 0x00, 0x21]);
        write_u32(self.output, self.plan.status);
        Ok(())
    }

    /// Consume a command-boundary invariant failure through the ordinary
    /// expression failure exit. This is deliberately not a Wasm trap: an
    /// owned stdin carrier may still be live, and the cleanup plan is the
    /// only authority that may settle that token exactly once.
    fn emit_command_failure_if(
        &mut self,
        _expression: &ExpressionId,
        channel: super::host_output::Globals,
        other: super::host_output::Globals,
    ) -> Result<(), Diagnostic> {
        self.output.extend([0x04, 0x40, 0x41]);
        write_i64(self.output, i64::from(STATUS_INTERNAL_INVALID_TAG));
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        super::host_output::emit_discard(self.output, channel);
        super::host_output::emit_discard(self.output, other);
        // Command provider invariant failures are backend fail-stop edges, not
        // source-level fallible expressions, so CleanupPlan intentionally has
        // no ReturnFailure exit keyed by this expression. Execute the plan's
        // canonical finalization inventory under its live flags instead: only
        // storage that is live at this exact point is settled, exactly once.
        let cleanup_plan = self.cleanup_plan.clone();
        self.emit_success_cleanup(&cleanup_plan)?;
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        Ok(())
    }

    fn emit_host_command_call(
        &mut self,
        expr: &ResolvedExpr,
        call: &crate::hir::ResolvedHostCommandCall,
    ) -> Result<Value, Diagnostic> {
        use crate::hir::ResolvedHostCommandOperation as Op;

        if call.args.len() != crate::command_io_ops::arity(call.operation) {
            return Err(error(
                "host command operation arity disagrees with resolved HIR",
            ));
        }
        let mut arguments = Vec::with_capacity(call.args.len());
        for argument in &call.args {
            arguments.push(self.emit_expr(argument)?);
        }
        self.apply_call_commit(&expr.id)?;
        let local = self.plan.expr_scalar(expr)?;
        if call.operation == Op::ArgsLen {
            self.output.push(0x10);
            write_u32(self.output, super::command_io::ARGS_LEN_IMPORT);
            self.output.push(0x21);
            write_u32(self.output, local);
            self.output.push(0x20);
            write_u32(self.output, local);
            self.output.extend([0x42]);
            write_i64(self.output, crate::command_io_ops::MAX_ARGUMENTS as i64);
            self.output.push(0x56); // i64.gt_u
            self.emit_command_failure_if(
                &expr.id,
                super::host_output::COMMAND_STDOUT_GLOBALS,
                super::host_output::COMMAND_STDERR_GLOBALS,
            )?;
            return Ok(Value::Scalar {
                local,
                ty: ResolvedType::Usize,
            });
        }

        let offset = self
            .plan
            .call_out
            .get(&expr.id)
            .copied()
            .ok_or_else(|| error("host command result has no exact out slot"))?;
        let pointer = Pointer {
            local: self.plan.frame_base,
            offset,
        };
        // Poison the provider out-slot before entry. A conforming provider
        // writes it only on status zero.
        self.emit_pointer(pointer);
        self.output.extend([0x42, 0x00, 0x37, 0x03, 0x00]);
        match call.operation {
            Op::ArgUtf8 => {
                self.require_scalar(&arguments[0], &ResolvedType::Usize, "arg_utf8 index")?;
                self.get_scalar(&arguments[0]);
                self.emit_pointer(pointer);
                self.output.push(0x10);
                write_u32(self.output, super::command_io::ARG_UTF8_IMPORT);
            }
            Op::StdinRead => {
                self.emit_pointer(pointer);
                self.output.push(0x10);
                write_u32(self.output, super::command_io::STDIN_READ_IMPORT);
            }
            Op::StderrWrite => {
                self.require_scalar(
                    &arguments[0],
                    &ResolvedType::SliceU8,
                    "stderr_write argument",
                )?;
                self.get_scalar(&arguments[0]);
                self.output.push(0x21);
                write_u32(self.output, local);
                let staged = Value::Scalar {
                    local,
                    ty: ResolvedType::SliceU8,
                };
                self.validate_byte_slice(&staged);
                self.emit_command_transcript_write(
                    &expr.id,
                    local,
                    super::host_output::COMMAND_STDERR_GLOBALS,
                    super::host_output::COMMAND_STDOUT_GLOBALS,
                )?;
                return Ok(Value::Scalar {
                    local,
                    ty: ResolvedType::Usize,
                });
            }
            Op::ArgsLen => unreachable!("handled above"),
        }
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);

        // Fail closed if an independently supplied provider returns a code
        // outside the operation's exact normalized sub-domain.
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        match call.operation {
            Op::ArgUtf8 => self.output.extend([0x41, 0x02, 0x4b]), // status > 2
            Op::StdinRead => {
                self.output.extend([0x41, 0x03, 0x49, 0x20]); // status < 3 ||
                write_u32(self.output, self.plan.status);
                self.output.extend([0x41, 0x04, 0x4b, 0x72]);
                self.output.push(0x20);
                write_u32(self.output, self.plan.status);
                self.output.extend([0x45, 0x45, 0x71]); // and status != 0
            }
            _ => unreachable!("fallible operation checked above"),
        }
        self.output.extend([0x04, 0x40, 0x41]);
        write_i64(self.output, i64::from(STATUS_INTERNAL_INVALID_TAG));
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        self.output.push(0x0b);
        if call.operation == Op::StdinRead {
            // Status zero is not enough: stdin must return one tagged,
            // nonzero owned-arena token within the invocation capacity.
            self.output.push(0x20);
            write_u32(self.output, self.plan.status);
            self.output.extend([0x45, 0x04, 0x40]);
            self.emit_pointer(pointer);
            self.load_scalar(&ResolvedType::Bytes);
            self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x41]);
            write_i64(self.output, i64::from(i32::MIN));
            self.output.extend([0x71, 0x45]);
            self.emit_pointer(pointer);
            self.load_scalar(&ResolvedType::Bytes);
            self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x41]);
            write_i64(self.output, i64::from(0x7fff_ffff_u32));
            self.output.extend([0x71, 0x45, 0x72]);
            self.emit_pointer(pointer);
            self.load_scalar(&ResolvedType::Bytes);
            self.output.extend([0xa7, 0x41]);
            write_i64(self.output, crate::command_io_ops::MAX_INPUT_BYTES as i64);
            self.output.extend([0x4b, 0x72, 0x04, 0x40, 0x41]);
            write_i64(self.output, i64::from(STATUS_INTERNAL_INVALID_TAG));
            self.output.push(0x21);
            write_u32(self.output, self.plan.status);
            self.output.extend([0x0b]);

            // Structural tagging is insufficient: authenticate exact arena
            // membership and recorded length through a closed recoverable
            // 0=member / 1=not-member provider contract before CleanupPlan is
            // allowed to initialize the owned result slot. This also checks
            // zero-length carriers instead of treating length zero as proof.
            self.output.push(0x20);
            write_u32(self.output, self.plan.status);
            self.output.extend([0x45, 0x04, 0x40]);
            self.emit_pointer(pointer);
            self.load_scalar(&ResolvedType::Bytes);
            self.output.push(0x10);
            write_u32(self.output, super::command_io::OWNED_BYTES_VALIDATE_IMPORT);
            self.output.push(0x22);
            write_u32(
                self.output,
                self.plan
                    .command_byte
                    .ok_or_else(|| error("command provider validation local is absent"))?,
            );
            self.output.extend([0x41, 0x01, 0x4b, 0x20]); // status > 1 || status == 1
            write_u32(
                self.output,
                self.plan
                    .command_byte
                    .ok_or_else(|| error("command provider validation local is absent"))?,
            );
            self.output
                .extend([0x41, 0x01, 0x46, 0x72, 0x04, 0x40, 0x41]);
            write_i64(self.output, i64::from(STATUS_INTERNAL_INVALID_TAG));
            self.output.push(0x21);
            write_u32(self.output, self.plan.status);
            self.output.extend([0x0b, 0x0b, 0x0b]);
        }
        // Authenticate the operation-specific command-input domain separately
        // from the shared language status code. Arithmetic, contract, and
        // internal fail-stop statuses leave this marker at zero.
        let (first_code, second_code) = match call.operation {
            Op::ArgUtf8 => (1, 2),
            Op::StdinRead => (3, 4),
            _ => unreachable!("only fallible command operations reach the marker"),
        };
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x41]);
        write_i64(self.output, first_code);
        self.output.extend([0x46, 0x20]);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x41]);
        write_i64(self.output, second_code);
        self.output.extend([0x46, 0x72, 0x04, 0x40, 0x20]);
        write_u32(self.output, self.plan.status);
        self.output.push(0x24);
        write_u32(self.output, super::command_io::INPUT_STATUS_GLOBAL);
        self.output.push(0x0b);
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x04, 0x40]);
        self.emit_failure_cleanup(&expr.id)?;
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        self.emit_pointer(pointer);
        self.load_scalar(&expr.ty);
        self.output.push(0x21);
        write_u32(self.output, local);
        Ok(Value::Scalar {
            local,
            ty: expr.ty.clone(),
        })
    }

    fn emit_call(
        &mut self,
        expr: &ResolvedExpr,
        callee: &DeclarationId,
        instance: Option<&crate::hir::FunctionInstanceId>,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        if instance.is_none() {
            if crate::host_io_ops::by_id(callee.as_str()).is_some() {
                if self.host_output.is_none() {
                    return Err(error(
                        "host stdout write requires the Wasm stdout-transcript profile",
                    ));
                }
                if args.len() != 1 {
                    return Err(error("host stdout write arity disagrees with resolved HIR"));
                }
                let value = self.emit_expr(&args[0])?;
                self.require_scalar(&value, &ResolvedType::SliceU8, "host stdout write argument")?;
                require_type(&expr.ty, &ResolvedType::Usize, "host stdout write result")?;
                self.apply_call_commit(&expr.id)?;
                let local = self.plan.expr_scalar(expr)?;
                self.get_scalar(&value);
                self.output.push(0x21);
                write_u32(self.output, local);
                let staged = Value::Scalar {
                    local,
                    ty: ResolvedType::SliceU8,
                };
                self.validate_byte_slice(&staged);
                let stdout = self.host_output.expect("checked stdout transcript profile");
                if self.program.permits
                    == [
                        crate::command_io_ops::ARGS_READ_EFFECT,
                        crate::command_io_ops::STDERR_WRITE_EFFECT,
                        crate::command_io_ops::STDIN_READ_EFFECT,
                        crate::host_io_ops::STDOUT_WRITE_EFFECT,
                    ]
                {
                    self.emit_command_transcript_write(
                        &expr.id,
                        local,
                        super::host_output::COMMAND_STDOUT_GLOBALS,
                        super::host_output::COMMAND_STDERR_GLOBALS,
                    )?;
                } else {
                    super::host_output::emit_write(self.output, local, local, stdout);
                }
                return Ok(Value::Scalar {
                    local,
                    ty: ResolvedType::Usize,
                });
            }
            if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                return self.emit_byte_op(expr, op, args);
            }
        }
        let target = self
            .program
            .resolve_call_target(callee, instance)
            .ok_or_else(|| error(format!("unknown aggregate callee `{callee}`")))?;
        if target.params.len() != args.len() {
            return Err(error(format!(
                "aggregate call `{callee}` has {} arguments; expected {}",
                args.len(),
                target.params.len()
            )));
        }
        let mut values = Vec::with_capacity(args.len());
        for (index, (argument, parameter)) in args.iter().zip(&target.params).enumerate() {
            let value = self.emit_expr(argument)?;
            require_type(value_type(&value), &parameter.ty, "call argument")?;
            let parameter_index = u32::try_from(index)
                .map_err(|_| error("aggregate call argument index overflows u32"))?;
            let epoch = crate::cleanup_plan::StorageId::CallArgument {
                call: expr.id.clone(),
                parameter_index,
                value_expression: argument.id.clone(),
            };
            values.push(
                self.plan
                    .cleanup_call_argument_carriers
                    .get(&epoch)
                    .copied()
                    .map_or(value, |local| Value::Scalar {
                        local,
                        ty: parameter.ty.clone(),
                    }),
            );
        }
        self.apply_call_commit(&expr.id)?;
        for value in &values {
            match value {
                Value::Scalar { local, .. } => {
                    self.output.push(0x20);
                    write_u32(self.output, *local);
                }
                Value::ScalarMemory { .. } => self.get_scalar(value),
                Value::Aggregate { pointer, .. } => self.emit_pointer(*pointer),
            }
        }
        let (result, result_pointer) =
            if is_aggregate(self.program, &expr.ty)? {
                let pointer = self.plan.expr_pointer(expr)?;
                (
                    Value::Aggregate {
                        pointer,
                        ty: expr.ty.clone(),
                    },
                    pointer,
                )
            } else {
                let offset =
                    self.plan.call_out.get(&expr.id).copied().ok_or_else(|| {
                        error(format!("missing scalar call out slot `{}`", expr.id))
                    })?;
                (
                    Value::Scalar {
                        local: self.plan.expr_scalar(expr)?,
                        ty: expr.ty.clone(),
                    },
                    Pointer {
                        local: self.plan.frame_base,
                        offset,
                    },
                )
            };
        self.emit_pointer(result_pointer);
        self.output.push(0x10);
        let execution = instance.map_or_else(
            || FunctionExecutionId::Monomorphic(callee.clone()),
            |instance| FunctionExecutionId::Generic(instance.clone()),
        );
        write_u32(
            self.output,
            *self
                .function_indexes
                .get(&execution)
                .ok_or_else(|| error(format!("callee `{callee}` is not indexed")))?,
        );
        self.output.push(0x22);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x04, 0x40]);
        self.emit_failure_cleanup(&expr.id)?;
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        if let Value::Scalar { local, ty } = &result {
            let offset = self.plan.call_out[&expr.id];
            self.emit_pointer(Pointer {
                local: self.plan.frame_base,
                offset,
            });
            self.load_scalar(ty);
            self.output.push(0x21);
            write_u32(self.output, *local);
        }
        Ok(result)
    }

    fn emit_byte_op(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::byte_ops::ByteOp,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        if args.len() != op.arity() {
            return Err(error("byte operation arity disagrees with resolved HIR"));
        }
        let mut values = Vec::with_capacity(args.len());
        for (argument, expected) in args.iter().zip(op.param_types()) {
            let value = self.emit_expr(argument)?;
            require_type(value_type(&value), expected, "byte operation argument")?;
            values.push(value);
        }
        self.apply_call_commit(&expr.id)?;
        self.validate_byte_slice(&values[0]);
        require_type(&expr.ty, &op.return_type(), "byte operation result")?;
        match op {
            crate::byte_ops::ByteOp::Len => {
                let local = self.plan.expr_scalar(expr)?;
                self.get_scalar(&values[0]);
                self.output.push(0x10);
                write_u32(self.output, BYTE_AS_SLICE_IMPORT);
                self.output.extend([0xa7, 0xad, 0x21]);
                write_u32(self.output, local);
                Ok(Value::Scalar {
                    local,
                    ty: ResolvedType::Usize,
                })
            }
            crate::byte_ops::ByteOp::Get => {
                let pointer = self.plan.expr_pointer(expr)?;
                let layout = variant_layout(self.variant_layouts, &expr.ty)?;
                let none = layout
                    .case(&crate::hir::DeclarationId::new(
                        crate::prelude::OPTION_NONE_ID,
                    ))
                    .ok_or_else(|| error("Option<u8> layout has no None case"))?;
                let some_id = crate::hir::DeclarationId::new(crate::prelude::OPTION_SOME_ID);
                let some = layout
                    .case(&some_id)
                    .ok_or_else(|| error("Option<u8> layout has no Some case"))?;
                let field = some
                    .field(&crate::hir::DeclarationId::new(
                        crate::prelude::OPTION_SOME_VALUE_ID,
                    ))
                    .ok_or_else(|| error("Option<u8> layout has no Some payload"))?;
                self.emit_pointer(pointer);
                self.output.extend([0x41, 0x00, 0x41]);
                write_i64(self.output, i64::from(layout.size));
                self.output.extend([0xfc, 0x0b, 0x00]);
                self.get_scalar(&values[0]);
                self.get_scalar(&values[1]);
                self.output.push(0x10);
                write_u32(self.output, BYTE_GET_IMPORT);
                self.output.extend([0x22]);
                write_u32(self.output, self.plan.status);
                self.output.extend([0x41, 0x00, 0x4e, 0x04, 0x40]);
                self.emit_pointer(Pointer {
                    local: pointer.local,
                    offset: pointer.offset + layout.payload_offset + field.offset,
                });
                self.output.push(0x20);
                write_u32(self.output, self.plan.status);
                self.output.extend([0x3a, 0x00, 0x00]);
                self.emit_pointer(pointer);
                self.output.push(0x41);
                write_i64(self.output, i64::from(some.tag));
                self.output.extend([0x36, 0x02, 0x00, 0x05]);
                self.emit_pointer(pointer);
                self.output.push(0x41);
                write_i64(self.output, i64::from(none.tag));
                self.output.extend([0x36, 0x02, 0x00, 0x0b]);
                self.output.extend([0x41, 0x00, 0x21]);
                write_u32(self.output, self.plan.status);
                Ok(Value::Aggregate {
                    pointer,
                    ty: expr.ty.clone(),
                })
            }
            crate::byte_ops::ByteOp::Copy => {
                let local = self.plan.expr_scalar(expr)?;
                self.get_scalar(&values[0]);
                self.output.push(0x10);
                write_u32(self.output, BYTE_COPY_IMPORT);
                self.output.push(0x21);
                write_u32(self.output, local);
                Ok(Value::Scalar {
                    local,
                    ty: ResolvedType::Bytes,
                })
            }
            crate::byte_ops::ByteOp::BytesAsSlice => {
                let local = self.plan.expr_scalar(expr)?;
                self.get_scalar(&values[0]);
                self.output.push(0x10);
                write_u32(self.output, BYTE_AS_SLICE_IMPORT);
                self.output.push(0x21);
                write_u32(self.output, local);
                Ok(Value::Scalar {
                    local,
                    ty: ResolvedType::SliceU8,
                })
            }
            crate::byte_ops::ByteOp::ArrayAsSlice | crate::byte_ops::ByteOp::StrAsBytes => Err(
                error("byte view operation must lower from authenticated BorrowPlace HIR"),
            ),
        }
    }

    fn emit_array_literal(
        &mut self,
        expr: &ResolvedExpr,
        values: &[u8],
        repeated: Option<(u8, u32)>,
    ) -> Result<Value, Diagnostic> {
        let ResolvedType::ArrayU8(length) = &expr.ty else {
            return Err(error("byte-array literal lacks an exact array type"));
        };
        if values.len() as u32 != *length && repeated.is_none() {
            return Err(error("byte-array literal length disagrees with its type"));
        }
        if repeated.is_some_and(|(_, count)| count != *length) {
            return Err(error("repeated byte-array length disagrees with its type"));
        }
        let destination = Value::Aggregate {
            pointer: self.plan.expr_pointer(expr)?,
            ty: expr.ty.clone(),
        };
        let Value::Aggregate { pointer, .. } = destination.clone() else {
            unreachable!();
        };
        for index in 0..*length {
            self.emit_pointer(Pointer {
                local: pointer.local,
                offset: pointer
                    .offset
                    .checked_add(index)
                    .ok_or_else(|| error("byte-array pointer overflows u32"))?,
            });
            let byte = match repeated {
                Some((value, _)) => value,
                None => values[index as usize],
            };
            self.output.extend([0x41]);
            write_i64(self.output, i64::from(byte));
            self.output.extend([0x3a, 0x00, 0x00]);
        }
        Ok(destination)
    }

    fn emit_borrow_place(
        &mut self,
        expr: &ResolvedExpr,
        operation: &DeclarationId,
        place: &crate::hir::Place,
    ) -> Result<Value, Diagnostic> {
        let source = self.place_value(place)?;
        let local = self.plan.expr_scalar(expr)?;
        match (operation.as_str(), &source) {
            (
                crate::byte_ops::ARRAY_AS_SLICE_ID,
                Value::Aggregate {
                    pointer,
                    ty: ResolvedType::ArrayU8(length),
                },
            ) => {
                if *length == 0 {
                    self.output.extend([0x42, 0x00]);
                } else {
                    self.emit_pointer(*pointer);
                    self.output.extend([0xad, 0x42, 0x20, 0x86]);
                    self.output.push(0x42);
                    write_i64(self.output, i64::from(*length));
                    self.output.push(0x84);
                }
            }
            (
                crate::byte_ops::BYTES_AS_SLICE_ID,
                Value::Scalar { .. } | Value::ScalarMemory { .. },
            ) => {
                require_type(value_type(&source), &ResolvedType::Bytes, "bytes view root")?;
                self.get_scalar(&source);
                self.output.push(0x10);
                write_u32(self.output, BYTE_AS_SLICE_IMPORT);
            }
            (
                crate::byte_ops::STR_AS_BYTES_ID,
                Value::Scalar { .. } | Value::ScalarMemory { .. },
            ) => {
                require_type(value_type(&source), &ResolvedType::Str, "borrowed str root")?;
                self.get_scalar(&source);
                self.output.push(0x10);
                write_u32(self.output, BYTE_AS_SLICE_IMPORT);
            }
            _ => {
                return Err(error(
                    "borrowed byte view root disagrees with its operation",
                ))
            }
        }
        self.output.push(0x21);
        write_u32(self.output, local);
        Ok(Value::Scalar {
            local,
            ty: ResolvedType::SliceU8,
        })
    }

    fn validate_byte_slice(&mut self, value: &Value) {
        // Tagged carrier: high 32 bits are the root word, low 32 bits length.
        self.get_scalar(value);
        self.output.extend([0xa7, 0xad, 0x42]);
        write_i64(self.output, crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES as i64);
        self.output.push(0x58); // i64.le_u
        self.get_scalar(value);
        self.output.extend([0x42, 0x20, 0x88, 0x42]);
        write_i64(self.output, 0x8000_0000u32 as i64);
        self.output.extend([0x83, 0x50, 0x45]);
        self.get_scalar(value);
        self.output.extend([0x42, 0x20, 0x88, 0xa7, 0xad, 0x42]);
        write_i64(self.output, 131_072);
        self.get_scalar(value);
        self.output.extend([
            0xa7, 0xad, 0x7d, 0x58, 0x72, 0x71, 0x45, 0x04, 0x40, 0x00, 0x0b,
        ]);
    }

    fn emit_unary(
        &mut self,
        expr: &ResolvedExpr,
        op: UnaryOp,
        operand: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let operand = self.emit_expr(operand)?;
        let destination = self.plan.expr_scalar(expr)?;
        match op {
            UnaryOp::Not => {
                self.require_scalar(&operand, &ResolvedType::Bool, "logical not")?;
                self.get_scalar(&operand);
                self.output.push(0x45);
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            UnaryOp::Neg => {
                let operand_ty = value_type(&operand);
                match operand_ty {
                    ResolvedType::F32 | ResolvedType::F64 => {
                        self.get_scalar(&operand);
                        self.output.push(match operand_ty {
                            ResolvedType::F32 => 0x8c,
                            _ => 0x9a,
                        });
                        self.output.push(0x21);
                        write_u32(self.output, destination);
                    }
                    ResolvedType::I32 => {
                        self.get_scalar(&operand);
                        self.output.push(0x41);
                        write_i64(self.output, i32::MIN as i64);
                        self.output.push(0x46);
                        self.fail_if(STATUS_NEG_OVERFLOW)?;
                        self.output.extend([0x41, 0x00]);
                        self.get_scalar(&operand);
                        self.output.push(0x6b);
                        self.output.push(0x21);
                        write_u32(self.output, destination);
                    }
                    _ => {
                        if operand_ty != &ResolvedType::I64 {
                            return Err(error("numeric negation requires an i64 or float operand"));
                        }
                        self.get_scalar(&operand);
                        self.output.push(0x42);
                        write_i64(self.output, i64::MIN);
                        self.output.push(0x51);
                        self.fail_if(STATUS_NEG_OVERFLOW)?;
                        self.output.push(0x42);
                        write_i64(self.output, 0);
                        self.get_scalar(&operand);
                        self.output.push(0x7d);
                        self.output.push(0x21);
                        write_u32(self.output, destination);
                    }
                }
            }
        }
        Ok(Value::Scalar {
            local: destination,
            ty: expr.ty.clone(),
        })
    }

    fn emit_binary(
        &mut self,
        expr: &ResolvedExpr,
        op: BinaryOp,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let left = self.emit_expr(left)?;
        let destination = self.plan.expr_scalar(expr)?;
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            self.require_scalar(&left, &ResolvedType::Bool, "lazy left operand")?;
            self.get_scalar(&left);
            self.output.extend([0x04, 0x40]);
            self.control_depth += 1;
            if op == BinaryOp::And {
                let right = self.emit_expr(right)?;
                self.require_scalar(&right, &ResolvedType::Bool, "lazy right operand")?;
                self.get_scalar(&right);
                self.output.push(0x21);
                write_u32(self.output, destination);
                self.output.push(0x05);
                self.output.extend([0x41, 0x00, 0x21]);
                write_u32(self.output, destination);
            } else {
                self.output.extend([0x41, 0x01, 0x21]);
                write_u32(self.output, destination);
                self.output.push(0x05);
                let right = self.emit_expr(right)?;
                self.require_scalar(&right, &ResolvedType::Bool, "lazy right operand")?;
                self.get_scalar(&right);
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            self.control_depth -= 1;
            self.output.push(0x0b);
            return Ok(Value::Scalar {
                local: destination,
                ty: ResolvedType::Bool,
            });
        }

        let right = self.emit_expr(right)?;
        if matches!(value_type(&left), ResolvedType::F32 | ResolvedType::F64)
            && !matches!(
                op,
                BinaryOp::Eq | BinaryOp::Ne | BinaryOp::And | BinaryOp::Or
            )
        {
            return self.emit_float_binary(expr, op, &left, &right, destination);
        }
        let int32_operands = matches!(value_type(&left), ResolvedType::I32);
        if matches!(value_type(&left), ResolvedType::U8)
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            )
        {
            let saved = self.failure_expression.replace(expr.id.clone());
            let result = self.emit_u8_binary(expr, op, &left, &right, destination);
            self.failure_expression = saved;
            return result;
        }
        if matches!(value_type(&left), ResolvedType::Usize)
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            )
        {
            let saved = self.failure_expression.replace(expr.id.clone());
            let result = self.emit_usize_binary(expr, op, &left, &right, destination);
            self.failure_expression = saved;
            return result;
        }
        let saved_failure = self.failure_expression.replace(expr.id.clone());
        match op {
            BinaryOp::Add if int32_operands => {
                self.emit_checked_i32_add(&left, &right, destination)?
            }
            BinaryOp::Sub if int32_operands => {
                self.emit_checked_i32_sub(&left, &right, destination)?
            }
            BinaryOp::Mul if int32_operands => {
                self.emit_checked_i32_mul(&left, &right, destination)?
            }
            BinaryOp::Div if int32_operands => {
                self.emit_checked_i32_div_rem(&left, &right, destination, false)?
            }
            BinaryOp::Rem if int32_operands => {
                self.emit_checked_i32_div_rem(&left, &right, destination, true)?
            }
            BinaryOp::Add => self.emit_checked_add(&left, &right, destination)?,
            BinaryOp::Sub => self.emit_checked_sub(&left, &right, destination)?,
            BinaryOp::Mul => self.emit_checked_mul(&left, &right, destination)?,
            BinaryOp::Div => self.emit_checked_div_rem(&left, &right, destination, false)?,
            BinaryOp::Rem => self.emit_checked_div_rem(&left, &right, destination, true)?,
            BinaryOp::Eq | BinaryOp::Ne => {
                if is_aggregate(self.program, value_type(&left))? {
                    return Err(error("record equality is outside executable records v1"));
                }
                require_type(value_type(&left), value_type(&right), "equality operands")?;
                self.get_scalar(&left);
                self.get_scalar(&right);
                self.output.push(match (value_type(&left), op) {
                    (ResolvedType::I64 | ResolvedType::Usize, BinaryOp::Eq) => 0x51,
                    (ResolvedType::I64 | ResolvedType::Usize, BinaryOp::Ne) => 0x52,
                    (ResolvedType::F32, BinaryOp::Eq) => 0x5b,
                    (ResolvedType::F32, BinaryOp::Ne) => 0x5c,
                    (ResolvedType::F64, BinaryOp::Eq) => 0x61,
                    (ResolvedType::F64, BinaryOp::Ne) => 0x62,
                    (_, BinaryOp::Eq) => 0x46,
                    (_, BinaryOp::Ne) => 0x47,
                    _ => unreachable!(),
                });
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
                let operand_ty = value_type(&left);
                if !matches!(
                    operand_ty,
                    ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::Char
                        | ResolvedType::U8
                        | ResolvedType::Usize
                        | ResolvedType::F32
                        | ResolvedType::F64
                ) {
                    return Err(error(format!(
                        "ordered comparison requires a scalar operand, found `{}`",
                        operand_ty.identity_key()
                    )));
                }
                require_type(
                    value_type(&right),
                    &operand_ty.clone(),
                    "ordered right operand",
                )?;
                self.get_scalar(&left);
                self.get_scalar(&right);
                self.output.push(match (&operand_ty, op) {
                    (ResolvedType::Char, BinaryOp::Lt) => 0x49,
                    (ResolvedType::Char, BinaryOp::Gt) => 0x4b,
                    (ResolvedType::Char, BinaryOp::Le) => 0x4d,
                    (ResolvedType::Char, BinaryOp::Ge) => 0x4f,
                    (ResolvedType::U8, BinaryOp::Lt) => 0x49,
                    (ResolvedType::U8, BinaryOp::Gt) => 0x4b,
                    (ResolvedType::U8, BinaryOp::Le) => 0x4d,
                    (ResolvedType::U8, BinaryOp::Ge) => 0x4f,
                    (ResolvedType::Usize, BinaryOp::Lt) => 0x54,
                    (ResolvedType::Usize, BinaryOp::Gt) => 0x56,
                    (ResolvedType::Usize, BinaryOp::Le) => 0x58,
                    (ResolvedType::Usize, BinaryOp::Ge) => 0x5a,
                    (ResolvedType::F32, BinaryOp::Lt) => 0x5d,
                    (ResolvedType::F32, BinaryOp::Gt) => 0x5e,
                    (ResolvedType::F32, BinaryOp::Le) => 0x5f,
                    (ResolvedType::F32, BinaryOp::Ge) => 0x60,
                    (ResolvedType::F64, BinaryOp::Lt) => 0x63,
                    (ResolvedType::F64, BinaryOp::Gt) => 0x64,
                    (ResolvedType::F64, BinaryOp::Le) => 0x65,
                    (ResolvedType::F64, BinaryOp::Ge) => 0x66,
                    (ResolvedType::F32 | ResolvedType::F64, _)
                        if matches!(op, BinaryOp::Rem | BinaryOp::And | BinaryOp::Or) =>
                    {
                        unreachable!("float remainder/lazy operation was matched above")
                    }
                    (ResolvedType::I32, BinaryOp::Lt) => 0x48,
                    (ResolvedType::I32, BinaryOp::Gt) => 0x4a,
                    (ResolvedType::I32, BinaryOp::Le) => 0x4c,
                    (ResolvedType::I32, BinaryOp::Ge) => 0x4e,
                    (_, BinaryOp::Lt) => 0x53,
                    (_, BinaryOp::Gt) => 0x55,
                    (_, BinaryOp::Le) => 0x57,
                    (_, BinaryOp::Ge) => 0x59,
                    _ => unreachable!("ordered operation was matched above"),
                });
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            BinaryOp::And | BinaryOp::Or => {
                unreachable!("lazy boolean operations were short-circuited above")
            }
        }
        self.failure_expression = saved_failure;
        Ok(Value::Scalar {
            local: destination,
            ty: expr.ty.clone(),
        })
    }

    /// Lowers total IEEE-754 arithmetic with native Wasm opcodes; no checked
    /// failure status exists for float operations.
    fn emit_float_binary(
        &mut self,
        expr: &ResolvedExpr,
        op: BinaryOp,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<Value, Diagnostic> {
        let operand_ty = value_type(left);
        require_type(
            value_type(right),
            &operand_ty.clone(),
            "float right operand",
        )?;
        self.get_scalar(left);
        self.get_scalar(right);
        let wide = matches!(operand_ty, ResolvedType::F64);
        self.output.push(match (op, wide) {
            (BinaryOp::Add, true) => 0xa0,
            (BinaryOp::Sub, true) => 0xa1,
            (BinaryOp::Mul, true) => 0xa2,
            (BinaryOp::Div, true) => 0xa3,
            (BinaryOp::Add, false) => 0x92,
            (BinaryOp::Sub, false) => 0x93,
            (BinaryOp::Mul, false) => 0x94,
            (BinaryOp::Div, false) => 0x95,
            (BinaryOp::Rem, _) => {
                return Err(error(
                    "floating-point remainder has no admitted Wasm lowering",
                ));
            }
            _ => unreachable!("float binary operation was matched above"),
        });
        self.output.push(0x21);
        write_u32(self.output, destination);
        Ok(Value::Scalar {
            local: destination,
            ty: expr.ty.clone(),
        })
    }

    fn emit_if(
        &mut self,
        expr: &ResolvedExpr,
        condition: &ResolvedExpr,
        then_branch: &ResolvedExpr,
        else_branch: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let condition = self.emit_expr(condition)?;
        self.require_scalar(&condition, &ResolvedType::Bool, "if condition")?;
        let destination = if is_aggregate(self.program, &expr.ty)? {
            Value::Aggregate {
                pointer: self.plan.expr_pointer(expr)?,
                ty: expr.ty.clone(),
            }
        } else {
            Value::Scalar {
                local: self.plan.expr_scalar(expr)?,
                ty: expr.ty.clone(),
            }
        };
        self.get_scalar(&condition);
        self.output.extend([0x04, 0x40]);
        self.control_depth += 1;
        let then_value = self.emit_expr(then_branch)?;
        self.copy_value(&destination, &then_value, "then branch")?;
        self.output.push(0x05);
        let else_value = self.emit_expr(else_branch)?;
        self.copy_value(&destination, &else_value, "else branch")?;
        self.control_depth -= 1;
        self.output.push(0x0b);
        Ok(destination)
    }

    fn emit_checked_add(
        &mut self,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<(), Diagnostic> {
        self.require_i64_pair(left, right, "addition")?;
        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(0x7c);
        self.output.push(0x21);
        write_u32(self.output, destination);
        self.get_scalar(left);
        self.output.push(0x20);
        write_u32(self.output, destination);
        self.output.push(0x85);
        self.get_scalar(right);
        self.output.push(0x20);
        write_u32(self.output, destination);
        self.output.push(0x85);
        self.output.push(0x83);
        self.output.push(0x42);
        write_i64(self.output, 0);
        self.output.push(0x53);
        self.fail_if(STATUS_ADD_OVERFLOW)?;
        Ok(())
    }

    fn emit_checked_sub(
        &mut self,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<(), Diagnostic> {
        self.require_i64_pair(left, right, "subtraction")?;
        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(0x7d);
        self.output.push(0x21);
        write_u32(self.output, destination);
        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(0x85);
        self.get_scalar(left);
        self.output.push(0x20);
        write_u32(self.output, destination);
        self.output.push(0x85);
        self.output.push(0x83);
        self.output.push(0x42);
        write_i64(self.output, 0);
        self.output.push(0x53);
        self.fail_if(STATUS_SUB_OVERFLOW)?;
        Ok(())
    }

    fn emit_checked_mul(
        &mut self,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<(), Diagnostic> {
        self.require_i64_pair(left, right, "multiplication")?;
        self.get_scalar(left);
        self.output.push(0x42);
        write_i64(self.output, i64::MIN);
        self.output.push(0x51);
        self.get_scalar(right);
        self.output.push(0x42);
        write_i64(self.output, -1);
        self.output.push(0x51);
        self.output.push(0x71);
        self.get_scalar(right);
        self.output.push(0x42);
        write_i64(self.output, i64::MIN);
        self.output.push(0x51);
        self.get_scalar(left);
        self.output.push(0x42);
        write_i64(self.output, -1);
        self.output.push(0x51);
        self.output.push(0x71);
        self.output.push(0x72);
        self.fail_if(STATUS_MUL_OVERFLOW)?;

        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(0x7e);
        self.output.push(0x21);
        write_u32(self.output, destination);
        self.get_scalar(right);
        self.output.push(0x50);
        self.output.push(0x45);
        self.output.extend([0x04, 0x40]);
        self.control_depth += 1;
        self.output.push(0x20);
        write_u32(self.output, destination);
        self.get_scalar(right);
        self.output.push(0x7f);
        self.get_scalar(left);
        self.output.push(0x52);
        self.fail_if(STATUS_MUL_OVERFLOW)?;
        self.control_depth -= 1;
        self.output.push(0x0b);
        Ok(())
    }

    fn emit_checked_div_rem(
        &mut self,
        left: &Value,
        right: &Value,
        destination: u32,
        remainder: bool,
    ) -> Result<(), Diagnostic> {
        self.require_i64_pair(
            left,
            right,
            if remainder { "remainder" } else { "division" },
        )?;
        self.get_scalar(right);
        self.output.push(0x50);
        self.fail_if(if remainder {
            STATUS_REM_ZERO
        } else {
            STATUS_DIV_ZERO
        })?;
        self.get_scalar(left);
        self.output.push(0x42);
        write_i64(self.output, i64::MIN);
        self.output.push(0x51);
        self.get_scalar(right);
        self.output.push(0x42);
        write_i64(self.output, -1);
        self.output.push(0x51);
        self.output.push(0x71);
        self.fail_if(if remainder {
            STATUS_REM_OVERFLOW
        } else {
            STATUS_DIV_OVERFLOW
        })?;
        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(if remainder { 0x81 } else { 0x7f });
        self.output.push(0x21);
        write_u32(self.output, destination);
        Ok(())
    }

    fn emit_checked_i32_add(
        &mut self,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<(), Diagnostic> {
        self.require_i32_pair(left, right, "addition")?;
        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(0x6a);
        self.output.push(0x21);
        write_u32(self.output, destination);
        self.get_scalar(left);
        self.output.push(0x20);
        write_u32(self.output, destination);
        self.output.push(0x73);
        self.get_scalar(right);
        self.output.push(0x20);
        write_u32(self.output, destination);
        self.output.push(0x73);
        self.output.push(0x71);
        self.output.push(0x41);
        write_i64(self.output, 0);
        self.output.push(0x48);
        self.fail_if(STATUS_ADD_OVERFLOW)?;
        Ok(())
    }

    fn emit_checked_i32_sub(
        &mut self,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<(), Diagnostic> {
        self.require_i32_pair(left, right, "subtraction")?;
        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(0x6b);
        self.output.push(0x21);
        write_u32(self.output, destination);
        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(0x73);
        self.get_scalar(left);
        self.output.push(0x20);
        write_u32(self.output, destination);
        self.output.push(0x73);
        self.output.push(0x71);
        self.output.push(0x41);
        write_i64(self.output, 0);
        self.output.push(0x48);
        self.fail_if(STATUS_SUB_OVERFLOW)?;
        Ok(())
    }

    fn emit_checked_i32_mul(
        &mut self,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<(), Diagnostic> {
        self.require_i32_pair(left, right, "multiplication")?;
        self.get_scalar(left);
        self.output.push(0xac);
        self.get_scalar(right);
        self.output.push(0xac);
        self.output.push(0x7e);
        self.output.push(0xa7);
        self.output.push(0x21);
        write_u32(self.output, destination);
        self.get_scalar(left);
        self.output.push(0xac);
        self.get_scalar(right);
        self.output.push(0xac);
        self.output.push(0x7e);
        self.output.push(0x20);
        write_u32(self.output, destination);
        self.output.push(0xac);
        self.output.push(0x51);
        self.output.push(0x45);
        self.fail_if(STATUS_MUL_OVERFLOW)?;
        Ok(())
    }

    fn emit_checked_i32_div_rem(
        &mut self,
        left: &Value,
        right: &Value,
        destination: u32,
        remainder: bool,
    ) -> Result<(), Diagnostic> {
        self.require_i32_pair(
            left,
            right,
            if remainder { "remainder" } else { "division" },
        )?;
        self.get_scalar(right);
        self.output.push(0x45);
        self.fail_if(if remainder {
            STATUS_REM_ZERO
        } else {
            STATUS_DIV_ZERO
        })?;
        if !remainder {
            self.get_scalar(right);
            self.output.push(0x41);
            write_i64(self.output, -1);
            self.output.push(0x46);
            self.get_scalar(left);
            self.output.push(0x41);
            write_i64(self.output, i32::MIN as i64);
            self.output.push(0x46);
            self.output.push(0x71);
            self.fail_if(STATUS_DIV_OVERFLOW)?;
        }
        self.get_scalar(left);
        self.get_scalar(right);
        self.output.push(if remainder { 0x6f } else { 0x6d });
        self.output.push(0x21);
        write_u32(self.output, destination);
        Ok(())
    }

    fn require_i32_pair(
        &self,
        left: &Value,
        right: &Value,
        context: &str,
    ) -> Result<(), Diagnostic> {
        self.require_scalar(left, &ResolvedType::I32, context)?;
        self.require_scalar(right, &ResolvedType::I32, context)
    }

    fn require_i64_pair(
        &self,
        left: &Value,
        right: &Value,
        context: &str,
    ) -> Result<(), Diagnostic> {
        self.require_scalar(left, &ResolvedType::I64, context)?;
        self.require_scalar(right, &ResolvedType::I64, context)
    }

    /// Checked u8 arithmetic mirrors the i64 status contract on the i32
    /// valtype: bounded operands make the unsigned opcodes exact and one
    /// unsigned range check selects the matching arithmetic status.
    fn emit_u8_binary(
        &mut self,
        expr: &ResolvedExpr,
        op: BinaryOp,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<Value, Diagnostic> {
        let context = match op {
            BinaryOp::Add => "addition",
            BinaryOp::Sub => "subtraction",
            BinaryOp::Mul => "multiplication",
            BinaryOp::Div => "division",
            BinaryOp::Rem => "remainder",
            _ => return Err(error("u8 binary operation is not arithmetic")),
        };
        self.require_scalar(left, &ResolvedType::U8, context)?;
        self.require_scalar(right, &ResolvedType::U8, context)?;
        match op {
            BinaryOp::Div | BinaryOp::Rem => {
                self.get_scalar(right);
                self.output.push(0x45);
                self.fail_if(if op == BinaryOp::Div {
                    STATUS_DIV_ZERO
                } else {
                    STATUS_REM_ZERO
                })?;
                self.get_scalar(left);
                self.get_scalar(right);
                self.output
                    .push(if op == BinaryOp::Div { 0x6e } else { 0x70 });
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
                self.get_scalar(left);
                self.get_scalar(right);
                self.output.push(match op {
                    BinaryOp::Add => 0x6a,
                    BinaryOp::Sub => 0x6b,
                    _ => 0x6c,
                });
                self.output.push(0x21);
                write_u32(self.output, destination);
                self.output.push(0x20);
                write_u32(self.output, destination);
                self.output.push(0x41);
                write_i64(self.output, 255);
                self.output.push(0x4b);
                self.fail_if(match op {
                    BinaryOp::Add => STATUS_ADD_OVERFLOW,
                    BinaryOp::Sub => STATUS_SUB_OVERFLOW,
                    _ => STATUS_MUL_OVERFLOW,
                })?;
            }
            _ => unreachable!("u8 operation was matched above"),
        }
        Ok(Value::Scalar {
            local: destination,
            ty: expr.ty.clone(),
        })
    }

    fn emit_usize_binary(
        &mut self,
        expr: &ResolvedExpr,
        op: BinaryOp,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<Value, Diagnostic> {
        self.require_scalar(left, &ResolvedType::Usize, "usize arithmetic")?;
        self.require_scalar(right, &ResolvedType::Usize, "usize arithmetic")?;
        match op {
            BinaryOp::Add => {
                self.get_scalar(left);
                self.get_scalar(right);
                self.output.push(0x7c);
                self.output.push(0x21);
                write_u32(self.output, destination);
                self.output.push(0x20);
                write_u32(self.output, destination);
                self.get_scalar(left);
                self.output.push(0x54);
                self.fail_if(STATUS_ADD_OVERFLOW)?;
            }
            BinaryOp::Sub => {
                self.get_scalar(left);
                self.get_scalar(right);
                self.output.push(0x54);
                self.fail_if(STATUS_SUB_OVERFLOW)?;
                self.get_scalar(left);
                self.get_scalar(right);
                self.output.push(0x7d);
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            BinaryOp::Mul => {
                self.get_scalar(right);
                self.output.push(0x50);
                self.output.push(0x45);
                self.get_scalar(left);
                self.output.push(0x42);
                write_i64(self.output, -1);
                self.get_scalar(right);
                self.output.push(0x80);
                self.output.push(0x56);
                self.output.push(0x71);
                self.fail_if(STATUS_MUL_OVERFLOW)?;
                self.get_scalar(left);
                self.get_scalar(right);
                self.output.push(0x7e);
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            BinaryOp::Div | BinaryOp::Rem => {
                self.get_scalar(right);
                self.output.push(0x50);
                self.fail_if(if op == BinaryOp::Div {
                    STATUS_DIV_ZERO
                } else {
                    STATUS_REM_ZERO
                })?;
                self.get_scalar(left);
                self.get_scalar(right);
                self.output
                    .push(if op == BinaryOp::Div { 0x80 } else { 0x82 });
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            _ => return Err(error("usize binary operation is not arithmetic")),
        }
        Ok(Value::Scalar {
            local: destination,
            ty: expr.ty.clone(),
        })
    }

    fn place_value(&self, place: &crate::hir::Place) -> Result<Value, Diagnostic> {
        let mut value =
            self.bindings.get(&place.root).cloned().ok_or_else(|| {
                error(format!("aggregate value `{}` is not in scope", place.root))
            })?;
        for projection in &place.projections {
            let PlaceProjection::Field(field) = projection else {
                return Err(error(
                    "variant-field place projection is outside executable records v1",
                ));
            };
            value = self.project_value(&value, field)?;
        }
        Ok(value)
    }

    fn project_value(&self, base: &Value, field: &DeclarationId) -> Result<Value, Diagnostic> {
        let Value::Aggregate { pointer, ty } = base else {
            return Err(error("record projection base is not aggregate storage"));
        };
        let record = layout(self.program, ty)?;
        let field = record
            .field(field)
            .cloned()
            .ok_or_else(|| error(format!("record `{}` has no field `{field}`", record.record)))?;
        value_at(
            Pointer {
                local: pointer.local,
                offset: pointer
                    .offset
                    .checked_add(field.offset)
                    .ok_or_else(|| error("projection pointer overflows u32"))?,
            },
            field.ty,
            self.program,
        )
    }

    fn copy_value(
        &mut self,
        destination: &Value,
        source: &Value,
        context: &str,
    ) -> Result<(), Diagnostic> {
        require_type(value_type(source), value_type(destination), context)?;
        match destination {
            Value::Scalar { local, ty } => {
                self.get_scalar(source);
                self.output.push(0x21);
                write_u32(self.output, *local);
                scalar_wasm_type(ty)?;
            }
            Value::ScalarMemory { pointer, ty } => {
                self.emit_pointer(*pointer);
                self.get_scalar(source);
                self.store_scalar(ty);
            }
            Value::Aggregate {
                pointer: destination,
                ty,
            } => {
                let Value::Aggregate {
                    pointer: source, ..
                } = source
                else {
                    return Err(error(format!(
                        "{context} mixes scalar and aggregate values"
                    )));
                };
                let (size, _) = aggregate_size_align(self.program, self.variant_layouts, ty)?;
                self.emit_pointer(*destination);
                self.emit_pointer(*source);
                self.output.push(0x41);
                write_i64(self.output, i64::from(size));
                self.output.extend([0xfc, 0x0a, 0x00, 0x00]);
            }
        }
        Ok(())
    }

    fn require_scalar(
        &self,
        value: &Value,
        ty: &ResolvedType,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if matches!(value, Value::Aggregate { .. }) {
            return Err(error(format!("{context} requires a scalar")));
        }
        require_type(value_type(value), ty, context)
    }

    fn get_scalar(&mut self, value: &Value) {
        match value {
            Value::Scalar { local, .. } => {
                self.output.push(0x20);
                write_u32(self.output, *local);
            }
            Value::ScalarMemory { pointer, ty } => {
                self.emit_pointer(*pointer);
                self.load_scalar(ty);
            }
            Value::Aggregate { .. } => unreachable!("validated scalar value"),
        }
    }

    fn emit_pointer(&mut self, pointer: Pointer) {
        self.output.push(0x20);
        write_u32(self.output, pointer.local);
        if pointer.offset != 0 {
            self.output.push(0x41);
            write_i64(self.output, i64::from(pointer.offset));
            self.output.push(0x6a);
        }
    }

    fn load_scalar(&mut self, ty: &ResolvedType) {
        match ty {
            ResolvedType::I64
            | ResolvedType::Usize
            | ResolvedType::SliceU8
            | ResolvedType::Str
            | ResolvedType::Bytes => self.output.extend([0x29, 0x03, 0x00]),
            ResolvedType::F64 => self.output.extend([0x2b, 0x03, 0x00]),
            ResolvedType::F32 => self.output.extend([0x2a, 0x02, 0x00]),
            ResolvedType::Bool | ResolvedType::Char | ResolvedType::I32 | ResolvedType::U8 => {
                self.output.extend([0x28, 0x02, 0x00])
            }
            _ => unreachable!("validated scalar load"),
        }
    }

    fn store_scalar(&mut self, ty: &ResolvedType) {
        match ty {
            ResolvedType::I64
            | ResolvedType::Usize
            | ResolvedType::SliceU8
            | ResolvedType::Str
            | ResolvedType::Bytes => self.output.extend([0x37, 0x03, 0x00]),
            ResolvedType::F64 => self.output.extend([0x39, 0x03, 0x00]),
            ResolvedType::F32 => self.output.extend([0x38, 0x02, 0x00]),
            ResolvedType::Bool | ResolvedType::Char | ResolvedType::I32 | ResolvedType::U8 => {
                self.output.extend([0x36, 0x02, 0x00])
            }
            _ => unreachable!("validated scalar store"),
        }
    }

    fn fail_if(&mut self, status: i32) -> Result<(), Diagnostic> {
        self.output.extend([0x04, 0x40, 0x41]);
        write_i64(self.output, i64::from(status));
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        if let Some(expression) = self.failure_expression.clone() {
            self.emit_failure_cleanup(&expression)?;
        }
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        Ok(())
    }

    /// Bounded While-Loops v1 lowers to a core `block`/`loop` pair: the
    /// condition re-evaluates at the top, a false condition branches out of
    /// the enclosing block, and the discarded body value falls through to the
    /// back-edge branch. Checked-arithmetic failures inside the loop keep the
    /// same sticky host-status contract as straight-line code.
    fn emit_while(
        &mut self,
        condition: &ResolvedExpr,
        body: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        self.output.extend([0x02, 0x40]); // block (empty) $exit
        self.output.extend([0x03, 0x40]); // loop (empty) $top
        self.control_depth += 2;
        let condition_value = self.emit_expr(condition)?;
        self.require_scalar(&condition_value, &ResolvedType::Bool, "while condition")?;
        self.get_scalar(&condition_value);
        self.output.push(0x45); // i32.eqz
        self.output.extend([0x0d, 0x01]); // br_if 1 -> $exit on false
        let _body_value = self.emit_expr(body)?;
        self.control_depth -= 2;
        self.output.extend([0x0c, 0x00]); // br 0 -> $top
        self.output.push(0x0b); // end loop
        self.output.push(0x0b); // end block
        Ok(())
    }
}

fn value_type(value: &Value) -> &ResolvedType {
    match value {
        Value::Scalar { ty, .. } | Value::ScalarMemory { ty, .. } | Value::Aggregate { ty, .. } => {
            ty
        }
    }
}

fn value_at(
    pointer: Pointer,
    ty: ResolvedType,
    program: &ResolvedProgram,
) -> Result<Value, Diagnostic> {
    if is_aggregate(program, &ty)? {
        Ok(Value::Aggregate { pointer, ty })
    } else {
        scalar_wasm_type(&ty)?;
        Ok(Value::ScalarMemory { pointer, ty })
    }
}

fn require_type(
    actual: &ResolvedType,
    expected: &ResolvedType,
    context: &str,
) -> Result<(), Diagnostic> {
    if actual == expected {
        Ok(())
    } else {
        Err(error(format!(
            "inconsistent HIR type for {context}: expected `{}`, found `{}`",
            expected.identity_key(),
            actual.identity_key()
        )))
    }
}

fn emit_wrapper(main_index: u32, host_output: bool) -> Vec<u8> {
    let old_stack = 0_u32;
    let frame_base = 1_u32;
    let status = 2_u32;
    let mut body = Vec::new();
    write_u32(&mut body, 3);
    write_u32(&mut body, 1);
    body.push(I32);
    write_u32(&mut body, 1);
    body.push(I32);
    write_u32(&mut body, 1);
    body.push(I32);

    if host_output {
        super::host_output::emit_reset(&mut body, super::host_output::ROOT_GLOBALS);
    }

    body.push(0x23);
    write_u32(&mut body, 0);
    body.push(0x22);
    write_u32(&mut body, old_stack);
    body.push(0x41);
    write_i64(&mut body, 8);
    body.push(0x49);
    body.extend([0x04, 0x40, 0x00, 0x0b]);
    body.push(0x20);
    write_u32(&mut body, old_stack);
    body.push(0x41);
    write_i64(&mut body, 8);
    body.push(0x6b);
    body.push(0x22);
    write_u32(&mut body, frame_base);
    body.push(0x24);
    write_u32(&mut body, 0);
    body.push(0x20);
    write_u32(&mut body, frame_base);
    body.push(0x10);
    write_u32(&mut body, main_index);
    body.push(0x21);
    write_u32(&mut body, status);
    body.push(0x20);
    write_u32(&mut body, old_stack);
    body.push(0x24);
    write_u32(&mut body, 0);

    if host_output {
        body.push(0x20);
        write_u32(&mut body, status);
        body.extend([0x04, 0x40]);
        super::host_output::emit_discard(&mut body, super::host_output::ROOT_GLOBALS);
        body.push(0x05);
        super::host_output::emit_publish(&mut body, super::host_output::ROOT_GLOBALS);
        body.push(0x0b);
    }

    body.push(0x20);
    write_u32(&mut body, status);
    body.push(0x41);
    write_i64(&mut body, i64::from(STATUS_INTERNAL_INVALID_TAG));
    body.extend([0x46, 0x04, 0x40, 0x00, 0x0b]);

    emit_arithmetic_trap_case(&mut body, status, STATUS_ADD_OVERFLOW, 0, i64::MAX, 1);
    emit_arithmetic_trap_case(&mut body, status, STATUS_SUB_OVERFLOW, 1, i64::MIN, 1);
    emit_arithmetic_trap_case(&mut body, status, STATUS_MUL_OVERFLOW, 2, i64::MAX, 2);
    emit_arithmetic_trap_case(&mut body, status, STATUS_DIV_ZERO, 3, 1, 0);
    emit_arithmetic_trap_case(&mut body, status, STATUS_DIV_OVERFLOW, 3, i64::MIN, -1);
    emit_arithmetic_trap_case(&mut body, status, STATUS_REM_ZERO, 4, 1, 0);
    emit_arithmetic_trap_case(&mut body, status, STATUS_REM_OVERFLOW, 4, i64::MIN, -1);
    body.push(0x20);
    write_u32(&mut body, status);
    body.push(0x41);
    write_i64(&mut body, i64::from(STATUS_NEG_OVERFLOW));
    body.push(0x46);
    body.extend([0x04, 0x40, 0x42]);
    write_i64(&mut body, i64::MIN);
    body.push(0x10);
    write_u32(&mut body, 5);
    body.extend([0x1a, 0x00, 0x0b]);

    body.push(0x20);
    write_u32(&mut body, status);
    body.push(0x45);
    body.extend([0x04, 0x40]);
    body.push(0x05);
    body.push(0x10);
    write_u32(&mut body, 6);
    body.push(0x00);
    body.push(0x0b);

    body.push(0x20);
    write_u32(&mut body, frame_base);
    body.extend([0x29, 0x03, 0x00, 0x0b]);
    body
}

fn emit_arithmetic_trap_case(
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

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        emit_profile, function_import, hex_identity, intern_type,
        lower_selected_function_instances, section, write_bytes, write_i64, write_name, write_u32,
        Signature, I32, SHADOW_STACK_TOP,
    };
    use crate::codegen::native_aggregate::{
        resource_harness_scenario, wasm_address, HarnessAction, ResourceHarnessScenario,
    };
    use crate::hir::{self, DeclarationId, FunctionInstanceId, ResolvedType};
    use crate::parse;

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    const BYTE_BOUNDARY_SOURCE: &str = r#"
module test.byte_boundary;
@id("bytes.total")
fn total(left: borrow Slice<u8>, right: borrow Slice<u8>) -> usize {
    byte_len(left) + byte_len(right)
}
@id("bytes.forward")
fn forward(left: borrow Slice<u8>, right: borrow Slice<u8>) -> usize {
    total(left, right)
}
@id("bytes.mixed")
fn mixed(text: borrow str, bytes: borrow Slice<u8>) -> usize {
    byte_len(str_as_bytes(text)) + byte_len(bytes)
}
@id("bytes.nul")
fn nul(text: borrow str) -> bool {
    match byte_get(str_as_bytes(text), 1usize) {
        Option::Some { value: byte } => byte == 0u8,
        Option::None {} => false,
    }
}
@id("bytes.at")
fn at(value: borrow Slice<u8>, index: usize) -> u8 {
    match byte_get(value, index) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    }
}
@id("usize.add.failure")
fn usize_add(left: usize, right: usize) -> usize { left + right }
@id("usize.sub.failure")
fn usize_sub(left: usize, right: usize) -> usize { left - right }
@id("usize.mul.failure")
fn usize_mul(left: usize, right: usize) -> usize { left * right }
@id("usize.div.failure")
fn usize_div(left: usize, right: usize) -> usize { left / right }
@id("usize.rem.failure")
fn usize_rem(left: usize, right: usize) -> usize { left % right }
@id("app.main")
fn main() -> i64 { 0 }
"#;

    #[test]
    fn node_rejects_invalid_and_cumulatively_oversized_external_byte_roots() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }
        let program = parse(BYTE_BOUNDARY_SOURCE, Path::new("byte-boundary-wasm.spx")).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        let bytes = emit_profile(&resolved, true, false).unwrap();
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-byte-boundary-wasm-{}-{id}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, bytes).unwrap();
        let export = |id: &str| format!("__spx_test_{}", hex_identity(&DeclarationId::new(id)));
        let script = format!(
            r#"import {{ readFile }} from "node:fs/promises";
const fail = (name) => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const bytes = await readFile(process.argv[2]);
let wasmInstance;
const result = await WebAssembly.instantiate(bytes, {{ env: {{
  spx_add: (a, b) => a + b, spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
  spx_bytes_copy: fail("spx_bytes_copy"), spx_bytes_drop: fail("spx_bytes_drop"),
  spx_bytes_as_slice: value => value,
  spx_bytes_get: (carrier, index) => {{
    const word=BigInt.asUintN(64,carrier);const length=Number(word&0xffffffffn);
    const offset=Number((word>>32n)&0xffffffffn);const at=BigInt.asUintN(64,index);
    return at>=BigInt(length)?-1:new Uint8Array(wasmInstance.exports.__spx_test_memory.buffer)[offset+Number(at)];
  }},
}} }});
wasmInstance=result.instance;
const {{ instance }} = result;
const view = new DataView(instance.exports.__spx_test_memory.buffer);
const output = 65536;
const pack = (offset, length) => (BigInt(offset) << 32n) | BigInt(length);
const forward = instance.exports["{forward}"];
const mixed = instance.exports["{mixed}"];
const nul = instance.exports["{nul}"];
const at = instance.exports["{at}"];
const usizeAdd = instance.exports["{usize_add}"];
const usizeSub = instance.exports["{usize_sub}"];
const usizeMul = instance.exports["{usize_mul}"];
const usizeDiv = instance.exports["{usize_div}"];
const usizeRem = instance.exports["{usize_rem}"];
if (forward(pack(0, 32768), pack(32768, 32768), output) !== 0) throw new Error("valid boundary status");
if (view.getBigUint64(output, true) !== 65536n) throw new Error("internal forwarding recharged roots");
view.setUint8(10, 0); view.setUint8(11, 255); view.setUint8(12, 7);
if (at(pack(10, 3), 1n, output) !== 0 || view.getUint8(output) !== 255) throw new Error("total indexed hit");
if (at(pack(10, 3), 3n, output) !== 0 || view.getUint8(output) !== 0) throw new Error("total indexed miss");
if (at(pack(10, 3), 0xffffffffffffffffn, output) !== 0 || view.getUint8(output) !== 0) throw new Error("total indexed max miss");
view.setUint8(20, 65); view.setUint8(21, 0); view.setUint8(22, 66);
if (nul(pack(20, 3), output) !== 0 || view.getUint8(output) !== 1) throw new Error("embedded NUL str view");
if (mixed(pack(20, 32768), pack(32768, 32768), output) !== 0 || view.getBigUint64(output,true)!==65536n) throw new Error("mixed root budget");
if (usizeAdd(-1n, 1n, output) !== 1) throw new Error("usize add overflow status");
if (usizeSub(0n, 1n, output) !== 2) throw new Error("usize sub overflow status");
if (usizeMul(-1n, 2n, output) !== 3) throw new Error("usize mul overflow status");
if (usizeDiv(1n, 0n, output) !== 4) throw new Error("usize division by zero status");
if (usizeRem(1n, 0n, output) !== 6) throw new Error("usize remainder by zero status");
let invalidRange = false;
try {{ forward(pack(65000, 1000), pack(0, 0), output); }} catch {{ invalidRange = true; }}
if (!invalidRange) throw new Error("invalid packed range was admitted");
let cumulative = false;
try {{ forward(pack(0, 40000), pack(0, 40000), output); }} catch {{ cumulative = true; }}
if (!cumulative) throw new Error("cumulative external roots were admitted");
let mixedCumulative = false;
try {{ mixed(pack(0, 40000), pack(0, 40000), output); }} catch {{ mixedCumulative = true; }}
if (!mixedCumulative) throw new Error("mixed external roots were admitted");
"#,
            forward = export("bytes.forward"),
            mixed = export("bytes.mixed"),
            nul = export("bytes.nul"),
            at = export("bytes.at"),
            usize_add = export("usize.add.failure"),
            usize_sub = export("usize.sub.failure"),
            usize_mul = export("usize.mul.failure"),
            usize_div = export("usize.div.failure"),
            usize_rem = export("usize.rem.failure")
        );
        std::fs::write(&script_path, script).unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    const GENERIC_INSTANCE_SOURCE: &str =
        include_str!("../../platform-tests/component-runtime/v9.spx");

    #[test]
    fn selected_generic_lowering_authenticates_exact_instance_sequence_and_identity() {
        let program = parse(
            GENERIC_INSTANCE_SOURCE,
            Path::new("selected-generic-instances.spx"),
        )
        .unwrap();
        let resolved = hir::resolve(&program).unwrap();
        let ordered = resolved
            .function_instances
            .iter()
            .map(|instance| instance.id.clone())
            .collect::<Vec<_>>();
        assert_eq!(ordered.len(), 6);

        let first = lower_selected_function_instances(&resolved, &ordered, &ordered[0]).unwrap();
        assert_eq!(first.selected_index, 0);
        for (index, selected) in ordered.iter().enumerate().skip(1) {
            let lowering =
                lower_selected_function_instances(&resolved, &ordered, selected).unwrap();
            assert_eq!(lowering.types, first.types);
            assert_eq!(lowering.function_type_indexes, first.function_type_indexes);
            assert_eq!(lowering.bodies, first.bodies);
            assert_eq!(lowering.selected_index, u32::try_from(index).unwrap());
        }

        assert!(lower_selected_function_instances(&resolved, &[], &ordered[0]).is_err());

        let mut missing = ordered.clone();
        missing.pop();
        assert!(lower_selected_function_instances(&resolved, &missing, &ordered[0]).is_err());

        let mut duplicate = ordered.clone();
        duplicate[5] = duplicate[0].clone();
        assert!(lower_selected_function_instances(&resolved, &duplicate, &ordered[0]).is_err());

        let mut reordered = ordered.clone();
        reordered.swap(0, 1);
        assert!(lower_selected_function_instances(&resolved, &reordered, &ordered[0]).is_err());

        let monomorphic_confusion = FunctionInstanceId::derive(
            &DeclarationId::new("generic.materialize"),
            &[ResolvedType::I64],
        );
        assert!(
            lower_selected_function_instances(&resolved, &ordered, &monomorphic_confusion,)
                .is_err()
        );

        let mut inconsistent = resolved.clone();
        inconsistent.function_instances[0].type_arguments[0] = ResolvedType::Bool;
        assert!(lower_selected_function_instances(&inconsistent, &ordered, &ordered[0]).is_err());
    }

    const SOURCE: &str = r#"
module test.aggregate_wasm;
@id("inner.type")
record Inner {
    @id("inner.value") value: i64,
    @id("inner.flag") flag: bool,
}
@id("outer.type")
record Outer {
    @id("outer.inner") inner: Inner,
    @id("outer.other") other: i64,
}
@id("case.ok")
fn ok(base: Outer) -> Outer {
    base with { inner: base.inner with { value: base.inner.value + 2 } }
}
@id("case.fail.base")
fn fail_base() -> Outer requires false {
    Outer { inner: Inner { value: 1, flag: true }, other: 2 }
}
@id("case.base.first")
fn base_first() -> Outer {
    fail_base() with { other: 9223372036854775807 + 1 }
}
@id("case.replacements")
fn replacements(base: Outer) -> Outer {
    base with {
        inner: base.inner with { value: 9223372036854775807 + 1 },
        other: 1 / 0,
    }
}
@id("case.post")
fn post(base: Outer) -> Outer ensures false { ok(base) }
@id("app.main")
fn main() -> i64 {
    let value = Outer {
        inner: Inner { value: 18, flag: true },
        other: 22,
    };
    let changed = ok(value);
    if changed.inner.flag { changed.inner.value + changed.other } else { 0 }
}
"#;

    const VARIANT_SOURCE: &str = r#"
module test.variant_wasm;
@id("choice.type")
variant Choice {
    @id("choice.none") None,
    @id("choice.number") Number {
        @id("choice.number.value") value: i64,
    },
    @id("choice.flag") Flag {
        @id("choice.flag.enabled") enabled: bool,
    },
    @id("choice.pair") Pair {
        @id("choice.pair.first") first: i64,
        @id("choice.pair.second") second: i64,
    },
}
@id("choice.make")
fn make(value: i64) -> Choice { Choice::Number { value: value } }
@id("choice.select")
fn select(choice: Choice) -> i64 {
    match choice {
        Choice::None {} => 0,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => if flag { 1 } else { 2 },
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.as_bool")
fn as_bool(choice: Choice) -> bool {
    match choice {
        Choice::None {} => false,
        Choice::Number { value: number } => number == 42,
        Choice::Flag { enabled: flag } => flag,
        Choice::Pair { first: left, second: right } => left == right,
    }
}
@id("choice.selected")
fn selected_only() -> i64 {
    match Choice::Number { value: 42 } {
        Choice::None {} => 9223372036854775807 + 1,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => 1 / 0,
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.selected_failure")
fn selected_failure() -> i64 {
    match Choice::Flag { enabled: true } {
        Choice::None {} => 1 / 0,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => 9223372036854775807 + 1,
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.construct_order")
fn construct_order() -> i64 {
    match Choice::Pair { second: 1 / 0, first: 9223372036854775807 + 1 } {
        Choice::None {} => 0,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => if flag { 1 } else { 2 },
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.scrutinee")
fn failing_scrutinee() -> Choice requires false { Choice::None {} }
@id("choice.aggregate_failure")
fn aggregate_failure() -> Choice {
    Choice::Number { value: 9223372036854775807 + 1 }
}
@id("choice.scrutinee_first")
fn scrutinee_first() -> i64 {
    match failing_scrutinee() {
        Choice::None {} => 9223372036854775807 + 1,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => if flag { 1 } else { 2 },
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.post")
fn post(choice: Choice) -> i64 ensures false { select(choice) }
@id("app.main")
fn main() -> i64 { select(make(42)) }
"#;

    const GENERIC_VARIANT_SOURCE: &str = r#"
module test.generic_variant_wasm;
@id("choice.generic")
variant Choice<T> {
    @id("choice.generic.none") None,
    @id("choice.generic.value") Value {
        @id("choice.generic.value.value") value: T,
    },
}
@id("choice.i64")
fn choice_i64() -> Choice<i64> { Choice<i64>::Value { value: 40 } }
@id("choice.bool")
fn choice_bool() -> Choice<bool> { Choice<bool>::Value { value: true } }
@id("choice.read_i64")
fn read_choice_i64(value: Choice<i64>) -> i64 {
    match value {
        Choice::None {} => 0,
        Choice::Value { value: inner } => inner,
    }
}
@id("choice.read_bool")
fn read_choice_bool(value: Choice<bool>) -> i64 {
    match value {
        Choice::None {} => 0,
        Choice::Value { value: inner } => if inner { 1 } else { 0 },
    }
}
@id("option.some")
fn option_some() -> Option<i64> { Option<i64>::Some { value: 1 } }
@id("option.read")
fn read_option(value: Option<i64>) -> i64 {
    match value {
        Option::None {} => 0,
        Option::Some { value: inner } => inner,
    }
}
@id("result.err")
fn result_err() -> Result<i64, bool> { Result<i64, bool>::Err { error: true } }
@id("result.read")
fn read_result(value: Result<i64, bool>) -> i64 {
    match value {
        Result::Ok { value: success } => success,
        Result::Err { error } => if error { 1 } else { 0 },
    }
}
@id("result.failure")
fn result_failure() -> Result<i64, bool> {
    Result<i64, bool>::Ok { value: 9223372036854775807 + 1 }
}
@id("app.main")
fn main() -> i64 {
    read_choice_i64(choice_i64()) + read_choice_bool(choice_bool()) +
        read_option(option_some()) + read_result(result_err())
}
"#;

    const RESULT_TRY_SOURCE: &str = r#"
module test.result_try_wasm;
@id("try.source_i64")
fn source_i64(residual: bool, value: i64) -> Result<i64, bool> {
    if residual {
        Result<i64, bool>::Err { error: true }
    } else {
        Result<i64, bool>::Ok { value: value }
    }
}
@id("try.source_bool")
fn source_bool(residual: bool, value: bool) -> Result<bool, bool> {
    if residual {
        Result<bool, bool>::Err { error: true }
    } else {
        Result<bool, bool>::Ok { value: value }
    }
}
@id("try.large_to_small")
fn large_to_small(residual: bool, value: i64) -> Result<bool, bool>
    ensures match result {
        Result::Ok { value: success } => success,
        Result::Err { error: failure } => failure,
    }
{
    let number = source_i64(residual, value)?;
    Result<bool, bool>::Ok { value: number > 0 }
}
@id("try.small_to_large")
fn small_to_large(residual: bool, value: bool) -> Result<i64, bool>
    ensures match result {
        Result::Ok { value: success } => success == 0 || success == 1,
        Result::Err { error: failure } => failure,
    }
{
    let flag = source_bool(residual, value)?;
    Result<i64, bool>::Ok { value: if flag { 1 } else { 0 } }
}
@id("try.post_err")
fn post_err() -> Result<bool, bool> ensures false {
    let number = source_i64(true, 7)?;
    Result<bool, bool>::Ok { value: number > 0 }
}
@id("try.physical")
fn physical() -> Result<i64, bool> requires false {
    Result<i64, bool>::Err { error: true }
}
@id("try.physical_then_post")
fn physical_then_post() -> Result<bool, bool> ensures false {
    let number = physical()?;
    Result<bool, bool>::Ok { value: number > 0 }
}
@id("try.err_skips_later")
fn err_skips_later() -> Result<bool, bool> {
    let number = source_i64(true, 7)?;
    Result<bool, bool>::Ok { value: number + 9223372036854775807 > 0 }
}
@id("try.from_input")
fn from_input(value: Result<i64, bool>) -> Result<bool, bool> {
    let number = value?;
    Result<bool, bool>::Ok { value: number > 0 }
}
@id("app.main")
fn main() -> i64 {
    let large = large_to_small(false, 42);
    let small = small_to_large(true, true);
    let left = match large {
        Result::Ok { value: success } => if success { 40 } else { 0 },
        Result::Err { error: failure } => if failure { 1 } else { 0 },
    };
    let right = match small {
        Result::Ok { value: success } => success,
        Result::Err { error: failure } => if failure { 2 } else { 0 },
    };
    left + right
}
"#;

    #[test]
    fn node_executes_aggregate_status_out_poison_order_and_shadow_stack_reentry() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }
        let program = parse(SOURCE, Path::new("aggregate-wasm.spx")).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        let bytes = emit_profile(&resolved, true, false).unwrap();
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-aggregate-wasm-{}-{id}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, bytes).unwrap();

        let export = |id: &str| format!("__spx_test_{}", hex_identity(&DeclarationId::new(id)));
        let script = format!(
            r#"import {{ readFile }} from "node:fs/promises";
const fail = (name) => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const bytes = await readFile(process.argv[2]);
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env: {{
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
}} }});
const memory = instance.exports.__spx_test_memory;
const stack = instance.exports.__spx_test_shadow_stack;
const view = new DataView(memory.buffer);
const input = 1024;
const output = 2048;
const poison = 0xa5;
const poisonOutput = () => new Uint8Array(memory.buffer, output, 24).fill(poison);
const assertPoison = () => {{
  for (const byte of new Uint8Array(memory.buffer, output, 24)) if (byte !== poison) throw new Error("aggregate failure published output");
}};
view.setBigInt64(input, 18n, true);
view.setInt32(input + 8, 1, true);
view.setBigInt64(input + 16, 22n, true);
poisonOutput();
if (instance.exports["{ok}"](input, output) !== 0) throw new Error("success status");
if (view.getBigInt64(output, true) !== 20n || view.getInt32(output + 8, true) !== 1 || view.getBigInt64(output + 16, true) !== 22n) throw new Error("success aggregate");
if (stack.value !== {stack_top}) throw new Error("success stack restore");
for (let index = 0; index < 4096; index += 1) {{
  poisonOutput();
  if (instance.exports["{base_first}"](output) !== 9) throw new Error("base-first status");
  assertPoison();
  if (stack.value !== {stack_top}) throw new Error("base-first stack restore");
  if (instance.exports["{replacements}"](input, output) !== 1) throw new Error("replacement-order status");
  assertPoison();
  if (stack.value !== {stack_top}) throw new Error("replacement stack restore");
  if (instance.exports["{post}"](input, output) !== 10) throw new Error("postcondition status");
  assertPoison();
  if (stack.value !== {stack_top}) throw new Error("postcondition stack restore");
}}
if (instance.exports.semaprax_main() !== 42n) throw new Error("public aggregate result");
if (stack.value !== {stack_top}) throw new Error("public stack restore");
console.log("aggregate-wasm-v1-ok");
"#,
            ok = export("case.ok"),
            base_first = export("case.base.first"),
            replacements = export("case.replacements"),
            post = export("case.post"),
            stack_top = SHADOW_STACK_TOP,
        );
        std::fs::write(&script_path, script).unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        assert!(
            output.status.success(),
            "Node aggregate runtime failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "aggregate-wasm-v1-ok"
        );
    }

    #[test]
    fn node_executes_copy_variants_selected_arms_invalid_tags_and_reentry() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }
        let program = parse(VARIANT_SOURCE, Path::new("variant-wasm.spx")).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        let bytes = emit_profile(&resolved, true, false).unwrap();
        assert_eq!(bytes, emit_profile(&resolved, true, false).unwrap());
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-variant-wasm-{}-{id}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, bytes).unwrap();

        let export = |id: &str| format!("__spx_test_{}", hex_identity(&DeclarationId::new(id)));
        let script = format!(
            r#"import {{ readFile }} from "node:fs/promises";
const fail = (name) => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const bytes = await readFile(process.argv[2]);
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env: {{
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
}} }});
const memory = instance.exports.__spx_test_memory;
const stack = instance.exports.__spx_test_shadow_stack;
const view = new DataView(memory.buffer);
const input = 1024;
const output = 2048;
const poison = 0xa5;
const poisonOutput = (length) => new Uint8Array(memory.buffer, output, length).fill(poison);
const assertPoison = (length) => {{
  for (const byte of new Uint8Array(memory.buffer, output, length)) if (byte !== poison) throw new Error("variant failure published output");
}};
const assertStack = (label) => {{ if (stack.value !== {stack_top}) throw new Error(`${{label}} stack restore`); }};
view.setUint32(input, 1, true);
view.setBigInt64(input + 8, 42n, true);
poisonOutput(8);
if (instance.exports["{select}"](input, output) !== 0 || view.getBigInt64(output, true) !== 42n) throw new Error("number match");
assertStack("number");
poisonOutput(4);
if (instance.exports["{as_bool}"](input, output) !== 0 || view.getInt32(output, true) !== 1) throw new Error("bool match");
assertStack("bool");
poisonOutput(24);
if (instance.exports["{failing_scrutinee}"](output) !== 9) throw new Error("aggregate failure status");
assertPoison(24);
assertStack("aggregate failure");
poisonOutput(24);
if (instance.exports["{aggregate_failure}"](output) !== 1) throw new Error("aggregate arithmetic failure status");
assertPoison(24);
assertStack("aggregate arithmetic failure");
poisonOutput(24);
if (instance.exports["{make}"](42n, output) !== 0) throw new Error("construct status");
if (view.getUint32(output, true) !== 1 || view.getBigInt64(output + 8, true) !== 42n) throw new Error("construct payload");
for (const offset of [4, 5, 6, 7, 16, 17, 18, 19, 20, 21, 22, 23]) if (view.getUint8(output + offset) !== 0) throw new Error("variant padding not zero");
assertStack("construct");
for (let index = 0; index < 4096; index += 1) {{
  poisonOutput(8);
  if (instance.exports["{selected}"](output) !== 0 || view.getBigInt64(output, true) !== 42n) throw new Error("selected arm");
  assertStack("selected");
  poisonOutput(8);
  if (instance.exports["{selected_failure}"](output) !== 1) throw new Error("selected failure status");
  assertPoison(8);
  assertStack("selected failure");
  if (instance.exports["{construct_order}"](output) !== 4) throw new Error("constructor source order status");
  assertPoison(8);
  assertStack("constructor order");
  if (instance.exports["{scrutinee_first}"](output) !== 9) throw new Error("scrutinee-first status");
  assertPoison(8);
  assertStack("scrutinee first");
  if (instance.exports["{post}"](input, output) !== 10) throw new Error("postcondition status");
  assertPoison(8);
  assertStack("postcondition");
}}
view.setUint32(input, 0xffffffff, true);
poisonOutput(8);
if (instance.exports["{select}"](input, output) !== -1) throw new Error("invalid tag did not fail out-of-band");
assertPoison(8);
assertStack("invalid tag");
if (instance.exports.semaprax_main() !== 42n) throw new Error("public variant result");
assertStack("public");
console.log("variant-wasm-v1-ok");
"#,
            select = export("choice.select"),
            as_bool = export("choice.as_bool"),
            make = export("choice.make"),
            selected = export("choice.selected"),
            selected_failure = export("choice.selected_failure"),
            construct_order = export("choice.construct_order"),
            scrutinee_first = export("choice.scrutinee_first"),
            post = export("choice.post"),
            failing_scrutinee = export("choice.scrutinee"),
            aggregate_failure = export("choice.aggregate_failure"),
            stack_top = SHADOW_STACK_TOP,
        );
        std::fs::write(&script_path, script).unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        assert!(
            output.status.success(),
            "Node variant runtime failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "variant-wasm-v1-ok"
        );
    }

    #[test]
    fn node_executes_generic_option_result_and_preserves_full_failure_poison() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }
        let program = parse(
            GENERIC_VARIANT_SOURCE,
            Path::new("generic-variant-wasm.spx"),
        )
        .unwrap();
        let resolved = hir::resolve(&program).unwrap();
        let bytes = emit_profile(&resolved, true, false).unwrap();
        assert_eq!(bytes, emit_profile(&resolved, true, false).unwrap());
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-generic-variant-wasm-{}-{id}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, bytes).unwrap();

        let export = |id: &str| format!("__spx_test_{}", hex_identity(&DeclarationId::new(id)));
        let script = format!(
            r#"import {{ readFile }} from "node:fs/promises";
const fail = (name) => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const bytes = await readFile(process.argv[2]);
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env: {{
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
}} }});
const memory = instance.exports.__spx_test_memory;
const stack = instance.exports.__spx_test_shadow_stack;
const view = new DataView(memory.buffer);
const input = 1024;
const output = 2048;
const poison = 0xa5;
const poisonOutput = (length) => new Uint8Array(memory.buffer, output, length).fill(poison);
const assertPoison = (length) => {{
  for (const byte of new Uint8Array(memory.buffer, output, length)) if (byte !== poison) throw new Error("generic failure published output");
}};
const assertStack = (label) => {{ if (stack.value !== {stack_top}) throw new Error(`${{label}} stack restore`); }};
poisonOutput(16);
if (instance.exports["{result_err}"](output) !== 0) throw new Error("Result Err status");
if (view.getUint32(output, true) !== 1 || view.getInt32(output + 8, true) !== 1) throw new Error("Result Err publication");
for (const offset of [4, 5, 6, 7, 12, 13, 14, 15]) if (view.getUint8(output + offset) !== 0) throw new Error("Result padding not zero");
assertStack("Result Err");
for (let index = 0; index < 4096; index += 1) {{
  poisonOutput(16);
  if (instance.exports["{result_failure}"](output) !== 1) throw new Error("Result failure status");
  assertPoison(16);
  assertStack("Result failure");
}}
view.setUint32(input, 0xffffffff, true);
poisonOutput(8);
if (instance.exports["{read_result}"](input, output) !== -1) throw new Error("invalid generic Result tag");
assertPoison(8);
assertStack("invalid Result tag");
if (instance.exports.semaprax_main() !== 43n) throw new Error("generic/prelude public result");
assertStack("public generic result");
console.log("generic-variant-wasm-v2-ok");
"#,
            result_err = export("result.err"),
            result_failure = export("result.failure"),
            read_result = export("result.read"),
            stack_top = SHADOW_STACK_TOP,
        );
        std::fs::write(&script_path, script).unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        assert!(
            output.status.success(),
            "Node generic/prelude variant runtime failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "generic-variant-wasm-v2-ok"
        );
    }

    #[test]
    fn node_executes_result_try_reconstruction_status_poison_and_reentry() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }
        let program = parse(RESULT_TRY_SOURCE, Path::new("result-try-wasm.spx")).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        let bytes = emit_profile(&resolved, true, false).unwrap();
        assert_eq!(bytes, emit_profile(&resolved, true, false).unwrap());
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-result-try-wasm-{}-{id}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, bytes).unwrap();

        let export = |id: &str| format!("__spx_test_{}", hex_identity(&DeclarationId::new(id)));
        let script = format!(
            r#"import {{ readFile }} from "node:fs/promises";
const fail = (name) => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const bytes = await readFile(process.argv[2]);
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env: {{
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
}} }});
const memory = instance.exports.__spx_test_memory;
const stack = instance.exports.__spx_test_shadow_stack;
const view = new DataView(memory.buffer);
const input = 1024;
const output = 2048;
const poison = 0xa5;
const poisonOutput = (length) => new Uint8Array(memory.buffer, output, length).fill(poison);
const assertPoison = (length, label) => {{
  for (const byte of new Uint8Array(memory.buffer, output, length)) if (byte !== poison) throw new Error(`${{label}} published output`);
}};
const assertStack = (label) => {{ if (stack.value !== {stack_top}) throw new Error(`${{label}} stack restore`); }};
const assertSmall = (tag, payload, label) => {{
  if (view.getUint32(output, true) !== tag || view.getInt32(output + 4, true) !== payload) throw new Error(`${{label}} value`);
}};
const assertLarge = (tag, payload, label) => {{
  if (view.getUint32(output, true) !== tag) throw new Error(`${{label}} tag`);
  if (tag === 0 && view.getBigInt64(output + 8, true) !== payload) throw new Error(`${{label}} Ok payload`);
  if (tag === 1 && view.getInt32(output + 8, true) !== Number(payload)) throw new Error(`${{label}} Err payload`);
  for (const offset of [4, 5, 6, 7]) if (view.getUint8(output + offset) !== 0) throw new Error(`${{label}} tag padding`);
  if (tag === 1) for (let offset = 9; offset < 16; offset += 1) if (view.getUint8(output + offset) !== 0) throw new Error(`${{label}} payload padding`);
}};

for (let index = 0; index < 4096; index += 1) {{
  poisonOutput(8);
  if (instance.exports["{large_to_small}"](0, 42n, output) !== 0) throw new Error("large-to-small Ok status");
  assertSmall(0, 1, "large-to-small Ok");
  assertStack("large-to-small Ok");

  poisonOutput(8);
  if (instance.exports["{large_to_small}"](1, 42n, output) !== 0) throw new Error("large-to-small Err status");
  assertSmall(1, 1, "large-to-small Err");
  assertStack("large-to-small Err");

  poisonOutput(16);
  if (instance.exports["{small_to_large}"](0, 1, output) !== 0) throw new Error("small-to-large Ok status");
  assertLarge(0, 1n, "small-to-large Ok");
  assertStack("small-to-large Ok");

  poisonOutput(16);
  if (instance.exports["{small_to_large}"](1, 1, output) !== 0) throw new Error("small-to-large Err status");
  assertLarge(1, 1n, "small-to-large Err");
  assertStack("small-to-large Err");

  poisonOutput(8);
  if (instance.exports["{post_err}"](output) !== 10) throw new Error("Err did not run ensures");
  assertPoison(8, "postcondition failure");
  assertStack("postcondition failure");

  poisonOutput(8);
  if (instance.exports["{physical_then_post}"](output) !== 9) throw new Error("physical status was replaced");
  assertPoison(8, "physical failure");
  assertStack("physical failure");

  poisonOutput(8);
  if (instance.exports["{err_skips_later}"](output) !== 0) throw new Error("Err residual status");
  assertSmall(1, 1, "Err skips later body");
  assertStack("Err skips later body");
}}

new Uint8Array(memory.buffer, input, 16).fill(0);
view.setUint32(input, 0xffffffff, true);
poisonOutput(8);
if (instance.exports["{from_input}"](input, output) !== -1) throw new Error("invalid Result tag did not fail out-of-band");
assertPoison(8, "invalid tag");
assertStack("invalid tag");
if (instance.exports.semaprax_main() !== 42n) throw new Error("typed ? public result");
assertStack("public typed ?");
console.log("result-try-wasm-v1-ok");
"#,
            large_to_small = export("try.large_to_small"),
            small_to_large = export("try.small_to_large"),
            post_err = export("try.post_err"),
            physical_then_post = export("try.physical_then_post"),
            err_skips_later = export("try.err_skips_later"),
            from_input = export("try.from_input"),
            stack_top = SHADOW_STACK_TOP,
        );
        std::fs::write(&script_path, script).unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        assert!(
            output.status.success(),
            "Node typed ? runtime failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "result-try-wasm-v1-ok"
        );
    }

    #[test]
    fn private_node_resource_records_follow_plan_order_and_finish_with_zero_liveness() {
        if Command::new("node").arg("--version").output().is_err() {
            return;
        }
        let scenario = resource_harness_scenario();
        let bytes = private_resource_harness_wasm(&scenario);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "semaprax-resource-aggregate-wasm-{}-{id}",
            std::process::id()
        );
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, bytes).unwrap();
        let expected = scenario
            .expected_trace
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let live = scenario
            .actions
            .iter()
            .filter_map(|action| match action {
                HarnessAction::Store(_, value) => Some(value.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            r#"import {{ readFile }} from "node:fs/promises";
const expected = [{expected}];
const live = new Set([{live}]);
const log = [];
const bytes = await readFile(process.argv[2]);
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env: {{
  finalize(handle) {{
    if (!live.delete(handle)) throw new Error(`duplicate/unknown finalizer ${{handle}}`);
    log.push(handle);
  }},
}} }});
if (instance.exports.run() !== 1) throw new Error("resource aggregate poison check failed");
if (live.size !== 0) throw new Error(`resource aggregate liveness ${{[...live]}}`);
if (log.join(",") !== expected.join(",")) throw new Error(`resource aggregate order ${{log}}`);
console.log("aggregate-resource-wasm-v1-ok");
"#
        );
        std::fs::write(&script_path, script).unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        assert!(
            output.status.success(),
            "Node private resource aggregate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "aggregate-resource-wasm-v1-ok"
        );
    }

    fn private_resource_harness_wasm(scenario: &ResourceHarnessScenario) -> Vec<u8> {
        let mut types = Vec::new();
        let mut indexes = std::collections::HashMap::new();
        let finalize_type = intern_type(
            Signature {
                params: vec![I32],
                results: vec![],
            },
            &mut types,
            &mut indexes,
        );
        let run_type = intern_type(
            Signature {
                params: vec![],
                results: vec![I32],
            },
            &mut types,
            &mut indexes,
        );
        let mut module = b"\0asm\x01\0\0\0".to_vec();
        let mut type_section = Vec::new();
        write_u32(&mut type_section, types.len() as u32);
        for signature in &types {
            type_section.push(0x60);
            write_bytes(&mut type_section, &signature.params);
            write_bytes(&mut type_section, &signature.results);
        }
        section(&mut module, 1, type_section);
        let mut imports = Vec::new();
        write_u32(&mut imports, 1);
        function_import(&mut imports, "env", "finalize", finalize_type);
        section(&mut module, 2, imports);
        let mut functions = Vec::new();
        write_u32(&mut functions, 1);
        write_u32(&mut functions, run_type);
        section(&mut module, 3, functions);
        let mut memory = Vec::new();
        write_u32(&mut memory, 1);
        memory.extend([0x00, 0x01]);
        section(&mut module, 5, memory);
        let mut exports = Vec::new();
        write_u32(&mut exports, 1);
        write_name(&mut exports, "run");
        exports.push(0x00);
        write_u32(&mut exports, 1);
        section(&mut module, 7, exports);

        let mut body = vec![0x00];
        for action in &scenario.actions {
            match *action {
                HarnessAction::Store(slot, value) => {
                    store(&mut body, wasm_address(slot), value as i32)
                }
                HarnessAction::Transfer(source, destination) => {
                    transfer(&mut body, wasm_address(source), wasm_address(destination))
                }
                HarnessAction::Finalize(slot) => finalize(&mut body, wasm_address(slot)),
                HarnessAction::PoisonPartialResult => {
                    // Wasm32 Pair is exactly two four-byte resource leaves;
                    // poison the entire caller result slot, not just field 0.
                    store(&mut body, 2048, 0x7f7f_7f7f);
                    store(&mut body, 2052, 0x7f7f_7f7f);
                }
            }
        }
        load(&mut body, 2048);
        body.push(0x41);
        write_i64(&mut body, 0x7f7f_7f7f);
        body.push(0x46);
        load(&mut body, 2052);
        body.push(0x41);
        write_i64(&mut body, 0x7f7f_7f7f);
        body.extend([0x46, 0x71, 0x0b]);
        let mut code = Vec::new();
        write_u32(&mut code, 1);
        write_u32(&mut code, body.len() as u32);
        code.extend(body);
        section(&mut module, 10, code);
        module
    }

    fn store(body: &mut Vec<u8>, address: i32, value: i32) {
        body.push(0x41);
        write_i64(body, i64::from(address));
        body.push(0x41);
        write_i64(body, i64::from(value));
        body.extend([0x36, 0x02, 0x00]);
    }

    fn load(body: &mut Vec<u8>, address: i32) {
        body.push(0x41);
        write_i64(body, i64::from(address));
        body.extend([0x28, 0x02, 0x00]);
    }

    fn transfer(body: &mut Vec<u8>, source: i32, destination: i32) {
        body.push(0x41);
        write_i64(body, i64::from(destination));
        load(body, source);
        body.extend([0x36, 0x02, 0x00]);
        store(body, source, 0);
    }

    fn finalize(body: &mut Vec<u8>, address: i32) {
        load(body, address);
        body.push(0x10);
        write_u32(body, 0);
        store(body, address, 0);
    }
}
