//! Byte-data capacity inputs derived from resolved HIR.
//!
//! Walks resolved expressions in authored order to build the bounded
//! capacity facts the byte-data analysis consumes.

use crate::diagnostic::Diagnostic;

use super::expr_nodes::{
    ResolvedExpr, ResolvedExprKind, ResolvedMatchPattern, ResolvedRecordMatchFieldPattern,
    ResolvedStatement,
};
use super::hir_error;
use super::ids::{DeclarationId, FunctionInstanceId, ValueId};
use super::monomorphize::substitute_type;
use super::nodes::{
    ByteSliceExtent, ByteSliceRootKind, ResolvedFunction, ResolvedHostCommandCall,
    ResolvedHostCommandOperation, ResolvedProgram, ResolvedType, ResolvedTypeDeclarationKind,
};

impl ResolvedProgram {
    pub fn resolve_call_target(
        &self,
        callee: &DeclarationId,
        instance: Option<&FunctionInstanceId>,
    ) -> Option<&ResolvedFunction> {
        match instance {
            None => self
                .functions
                .iter()
                .find(|function| function.id == *callee),
            Some(instance) => self
                .function_instances
                .iter()
                .find(|candidate| candidate.id == *instance && candidate.template == *callee)
                .map(|candidate| &candidate.function),
        }
    }
}

pub(super) fn inline_array_payload_bytes(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> Result<u32, Diagnostic> {
    let mut total = 0_u32;
    let mut pending = vec![ty.clone()];
    let mut expanded = 0_usize;
    while let Some(ty) = pending.pop() {
        expanded = expanded
            .checked_add(1)
            .ok_or_else(|| hir_error("inline-array type traversal overflowed"))?;
        if expanded > 65_536 {
            return Err(hir_error(
                "inline-array type traversal exceeds the compiler bound",
            ));
        }
        match ty {
            ResolvedType::ArrayU8(length) => {
                total = total
                    .checked_add(length)
                    .ok_or_else(|| hir_error("inline-array payload calculation overflowed"))?;
            }
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                let declaration = program
                    .types
                    .iter()
                    .find(|candidate| candidate.id == declaration)
                    .ok_or_else(|| hir_error("inline-array slot references an unknown type"))?;
                let fields = match &declaration.kind {
                    ResolvedTypeDeclarationKind::Record { fields }
                    | ResolvedTypeDeclarationKind::Class { fields, .. } => fields.as_slice(),
                    ResolvedTypeDeclarationKind::Variant { .. }
                    | ResolvedTypeDeclarationKind::Resource { .. } => &[],
                };
                for field in fields.iter().rev() {
                    pending.push(substitute_type(&field.ty, &declaration.id, &arguments)?);
                }
            }
            ResolvedType::Unit
            | ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
            | ResolvedType::String
            | ResolvedType::Bytes
            | ResolvedType::Str
            | ResolvedType::SliceU8 => {}
            ResolvedType::TypeParameter { .. } => {
                return Err(hir_error(
                    "inline-array capacity cannot inspect an unresolved type parameter",
                ));
            }
        }
    }
    Ok(total)
}

pub(super) fn push_array_slot(
    program: &ResolvedProgram,
    slots: &mut Vec<crate::byte_data_capacity::ArrayStorageSlot>,
    identity: String,
    kind: crate::byte_data_capacity::ArrayStorageKind,
    ty: &ResolvedType,
) -> Result<(), Diagnostic> {
    let length = inline_array_payload_bytes(program, ty)?;
    if length != 0 || matches!(ty, ResolvedType::ArrayU8(0)) {
        slots.push(crate::byte_data_capacity::ArrayStorageSlot {
            identity,
            kind,
            length,
        });
    }
    Ok(())
}

pub(super) fn push_array_pattern_slots(
    program: &ResolvedProgram,
    pattern: &ResolvedMatchPattern,
    slots: &mut Vec<crate::byte_data_capacity::ArrayStorageSlot>,
) -> Result<(), Diagnostic> {
    match pattern {
        ResolvedMatchPattern::Binding(binding) => push_array_slot(
            program,
            slots,
            binding.id.as_str().to_owned(),
            crate::byte_data_capacity::ArrayStorageKind::Binding,
            &binding.ty,
        ),
        ResolvedMatchPattern::Variant { fields, .. } => {
            for field in fields {
                push_array_slot(
                    program,
                    slots,
                    field.binding.id.as_str().to_owned(),
                    crate::byte_data_capacity::ArrayStorageKind::Binding,
                    &field.binding.ty,
                )?;
            }
            Ok(())
        }
        ResolvedMatchPattern::Record { fields, .. } => {
            let mut pending = fields
                .iter()
                .rev()
                .map(|field| &field.pattern)
                .collect::<Vec<_>>();
            while let Some(pattern) = pending.pop() {
                match pattern {
                    ResolvedRecordMatchFieldPattern::Binding(binding) => push_array_slot(
                        program,
                        slots,
                        binding.id.as_str().to_owned(),
                        crate::byte_data_capacity::ArrayStorageKind::Binding,
                        &binding.ty,
                    )?,
                    ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
                        pending.extend(fields.iter().rev().map(|field| &field.pattern));
                    }
                    ResolvedRecordMatchFieldPattern::Wildcard => {}
                }
            }
            Ok(())
        }
        ResolvedMatchPattern::Wildcard
        | ResolvedMatchPattern::Literal(_)
        | ResolvedMatchPattern::Or(_) => Ok(()),
    }
}

pub(super) fn byte_slice_transcript_source(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
) -> crate::byte_data_capacity::TranscriptSource {
    use crate::byte_data_capacity::TranscriptSource;
    enum Frame<'a> {
        Visit(&'a ResolvedExpr),
        If,
    }
    let mut frames = vec![Frame::Visit(expression)];
    let mut results = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Visit(expression) => match &expression.kind {
                ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. } => {
                    if let Some(fact) = program.declarations.byte_slice_provenance(&place.root) {
                        results.push(match fact.root_kind {
                            ByteSliceRootKind::CommandArguments => {
                                TranscriptSource::CommandArguments
                            }
                            ByteSliceRootKind::FixedArray => match fact.length {
                                ByteSliceExtent::Constant(length) => {
                                    TranscriptSource::Fixed(length)
                                }
                                ByteSliceExtent::ParameterLength | ByteSliceExtent::ValueLength => {
                                    TranscriptSource::Unknown
                                }
                            },
                            ByteSliceRootKind::OwnedBytes
                                if resolved_value_is_stdin(program, &fact.root) =>
                            {
                                TranscriptSource::Stdin
                            }
                            ByteSliceRootKind::FunctionParameter
                            | ByteSliceRootKind::OwnedBytes
                            | ByteSliceRootKind::BorrowedStr => TranscriptSource::Unknown,
                        });
                    } else {
                        results.push(resolved_value_type(program, &place.root).map_or(
                            TranscriptSource::Unknown,
                            |ty| match ty {
                                ResolvedType::ArrayU8(length) => {
                                    TranscriptSource::Fixed(u64::from(length))
                                }
                                _ => TranscriptSource::Unknown,
                            },
                        ));
                    }
                }
                ResolvedExprKind::Call { callee, args, .. }
                    if callee.as_str() == crate::byte_ops::ARRAY_AS_SLICE_ID =>
                {
                    results.push(args.first().map_or(TranscriptSource::Unknown, |argument| {
                        match argument.ty {
                            ResolvedType::ArrayU8(length) => {
                                TranscriptSource::Fixed(u64::from(length))
                            }
                            _ => TranscriptSource::Unknown,
                        }
                    }));
                }
                ResolvedExprKind::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    frames.push(Frame::If);
                    frames.push(Frame::Visit(else_branch));
                    frames.push(Frame::Visit(then_branch));
                }
                ResolvedExprKind::Block { tail, .. } => frames.push(Frame::Visit(tail)),
                _ => results.push(TranscriptSource::Unknown),
            },
            Frame::If => {
                let else_source = results.pop().unwrap_or(TranscriptSource::Unknown);
                let then_source = results.pop().unwrap_or(TranscriptSource::Unknown);
                results.push(if then_source == else_source {
                    then_source
                } else {
                    TranscriptSource::Unknown
                });
            }
        }
    }
    results.pop().unwrap_or(TranscriptSource::Unknown)
}

pub(crate) fn push_resolved_expression_children_in_authored_order<'a>(
    expression: &'a ResolvedExpr,
    pending: &mut Vec<&'a ResolvedExpr>,
) {
    match &expression.kind {
        ResolvedExprKind::Block { statements, tail } => {
            pending.push(tail);
            for statement in statements.iter().rev() {
                for index in (0..statement.child_count()).rev() {
                    if let Some(child) = statement.child(index) {
                        pending.push(child);
                    }
                }
            }
        }
        ResolvedExprKind::Call { args, .. } => pending.extend(args.iter().rev()),
        ResolvedExprKind::NativeRustImportCall(call) => pending.extend(call.args.iter().rev()),
        ResolvedExprKind::HostCommandCall(call) => pending.extend(call.args.iter().rev()),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            pending.push(end);
            pending.push(start);
            pending.push(source);
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => pending.push(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            pending.push(right);
            pending.push(left);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            pending.push(else_branch);
            pending.push(then_branch);
            pending.push(condition);
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value));
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            for arm in arms.iter().rev() {
                pending.push(&arm.value);
                if let Some(guard) = &arm.guard {
                    pending.push(guard);
                }
            }
            pending.push(scrutinee);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value));
            pending.push(base);
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
    }
}

pub(super) fn resolved_value_is_stdin(program: &ResolvedProgram, value: &ValueId) -> bool {
    let in_expression = |root: &ResolvedExpr| {
        let mut pending = vec![root];
        while let Some(expression) = pending.pop() {
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                for statement in statements {
                    if let ResolvedStatement::Let {
                        binding,
                        value: initializer,
                        ..
                    } = statement
                    {
                        if binding.id == *value {
                            return matches!(
                                &initializer.kind,
                                ResolvedExprKind::HostCommandCall(ResolvedHostCommandCall {
                                    operation: ResolvedHostCommandOperation::StdinRead,
                                    ..
                                })
                            );
                        }
                    }
                }
            }
            push_resolved_expression_children_in_authored_order(expression, &mut pending);
        }
        false
    };
    program
        .functions
        .iter()
        .any(|function| in_expression(&function.body))
        || program
            .function_instances
            .iter()
            .any(|instance| in_expression(&instance.function.body))
}

pub(super) fn resolved_value_type(
    program: &ResolvedProgram,
    value: &ValueId,
) -> Option<ResolvedType> {
    let in_expression = |root: &ResolvedExpr| {
        let mut pending = vec![root];
        while let Some(expression) = pending.pop() {
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                for statement in statements {
                    if let ResolvedStatement::Let { binding, .. }
                    | ResolvedStatement::Assign { binding, .. } = statement
                    {
                        if binding.id == *value {
                            return Some(binding.ty.clone());
                        }
                    }
                }
            }
            push_resolved_expression_children_in_authored_order(expression, &mut pending);
        }
        None
    };

    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .find_map(|function| {
            function
                .params
                .iter()
                .find(|parameter| parameter.id == *value)
                .map(|parameter| parameter.ty.clone())
                .or_else(|| in_expression(&function.body))
        })
}
pub(super) fn byte_capacity_expression(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    slots: &mut Vec<crate::byte_data_capacity::ArrayStorageSlot>,
    direct_destination: bool,
) -> Result<crate::byte_data_capacity::CapacityFlow, Diagnostic> {
    use crate::byte_data_capacity::{ArrayStorageKind, CapacityFlow};

    enum Frame<'a> {
        Visit(&'a ResolvedExpr, bool),
        Argument(
            &'a ResolvedExpr,
            Option<(String, ArrayStorageKind, ResolvedType)>,
            bool,
        ),
        Sequence(usize),
        Alternative(usize),
        Loop,
        Match(usize),
        Emit(CapacityFlow),
    }
    fn sequence(children: Vec<CapacityFlow>) -> CapacityFlow {
        if children.is_empty() {
            CapacityFlow::Empty
        } else {
            CapacityFlow::Sequence(children)
        }
    }

    let mut frames = vec![Frame::Visit(expression, direct_destination)];
    let mut results = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Visit(expression, direct_destination) => {
                let payload = inline_array_payload_bytes(program, &expression.ty)?;
                if payload != 0 || matches!(expression.ty, ResolvedType::ArrayU8(0)) {
                    let kind = match &expression.kind {
                        ResolvedExprKind::Call { .. } => Some(ArrayStorageKind::CallStaging),
                        ResolvedExprKind::ArrayU8(_) | ResolvedExprKind::RepeatArrayU8 { .. }
                            if direct_destination =>
                        {
                            None
                        }
                        ResolvedExprKind::Place(_) | ResolvedExprKind::BorrowPlace { .. } => None,
                        ResolvedExprKind::ByteRange { .. } => None,
                        ResolvedExprKind::Block { .. }
                        | ResolvedExprKind::If { .. }
                        | ResolvedExprKind::Match { .. } => None,
                        _ => Some(ArrayStorageKind::Temporary),
                    };
                    if let Some(kind) = kind {
                        slots.push(crate::byte_data_capacity::ArrayStorageSlot {
                            identity: expression.id.as_str().to_owned(),
                            kind,
                            length: payload,
                        });
                    }
                }
                match &expression.kind {
                    ResolvedExprKind::Call {
                        callee,
                        instance,
                        args,
                        ..
                    } => {
                        let effect = if callee.as_str() == crate::byte_ops::COPY_ID {
                            Some(CapacityFlow::BytesCopy {
                                site: expression.id.as_str().to_owned(),
                                conservative_payload_bytes:
                                    crate::byte_data_capacity::MAX_ARRAY_BYTES,
                            })
                        } else if callee.as_str() == crate::host_io_ops::STDOUT_WRITE_ID {
                            Some(CapacityFlow::StdoutWrite {
                                site: expression.id.as_str().to_owned(),
                                source: byte_slice_transcript_source(program, &args[0]),
                            })
                        } else if program
                            .resolve_call_target(callee, instance.as_ref())
                            .is_some()
                        {
                            Some(CapacityFlow::Call {
                                site: expression.id.as_str().to_owned(),
                                callee: instance
                                    .as_ref()
                                    .map_or_else(|| callee.as_str(), FunctionInstanceId::as_str)
                                    .to_owned(),
                            })
                        } else {
                            None
                        };
                        frames.push(Frame::Sequence(args.len() + usize::from(effect.is_some())));
                        if let Some(effect) = effect {
                            frames.push(Frame::Emit(effect));
                        }
                        for (index, argument) in args.iter().enumerate().rev() {
                            frames.push(Frame::Argument(
                                argument,
                                Some((
                                    format!("{}.arg.{index}", expression.id.as_str()),
                                    ArrayStorageKind::CallStaging,
                                    argument.ty.clone(),
                                )),
                                false,
                            ));
                        }
                    }
                    ResolvedExprKind::NativeRustImportCall(call) => {
                        frames.push(Frame::Sequence(call.args.len()));
                        for argument in call.args.iter().rev() {
                            frames.push(Frame::Visit(argument, false));
                        }
                    }
                    ResolvedExprKind::HostCommandCall(call) => {
                        let effect = if call.operation == ResolvedHostCommandOperation::StdinRead {
                            Some(CapacityFlow::StdinRead {
                                site: expression.id.as_str().to_owned(),
                                conservative_payload_bytes: crate::command_io_ops::MAX_INPUT_BYTES,
                            })
                        } else if call.operation == ResolvedHostCommandOperation::StderrWrite {
                            Some(CapacityFlow::StderrWrite {
                                site: expression.id.as_str().to_owned(),
                                source: byte_slice_transcript_source(program, &call.args[0]),
                            })
                        } else if call.operation == ResolvedHostCommandOperation::NetRecv {
                            // One bounded network read is an owned-byte
                            // allocation site with the conservative chunk
                            // payload; it is not a stdin read.
                            Some(CapacityFlow::BytesCopy {
                                site: expression.id.as_str().to_owned(),
                                conservative_payload_bytes: crate::network_io_ops::MAX_CHUNK_BYTES,
                            })
                        } else {
                            None
                        };
                        frames.push(Frame::Sequence(
                            call.args.len() + usize::from(effect.is_some()),
                        ));
                        if let Some(effect) = effect {
                            frames.push(Frame::Emit(effect));
                        }
                        for argument in call.args.iter().rev() {
                            frames.push(Frame::Visit(argument, false));
                        }
                    }
                    ResolvedExprKind::ByteRange {
                        source, start, end, ..
                    } => {
                        frames.push(Frame::Sequence(3));
                        frames.push(Frame::Visit(end, false));
                        frames.push(Frame::Visit(start, false));
                        frames.push(Frame::Visit(source, false));
                    }
                    ResolvedExprKind::Unary { value, .. }
                    | ResolvedExprKind::Try { operand: value, .. }
                    | ResolvedExprKind::TryOption { operand: value, .. }
                    | ResolvedExprKind::Project { base: value, .. }
                    | ResolvedExprKind::Upcast { source: value } => {
                        frames.push(Frame::Visit(value, false));
                    }
                    ResolvedExprKind::Binary { left, right, .. } => {
                        frames.push(Frame::Sequence(2));
                        frames.push(Frame::Visit(right, false));
                        frames.push(Frame::Visit(left, false));
                    }
                    ResolvedExprKind::Block { statements, tail } => {
                        frames.push(Frame::Sequence(statements.len() + 1));
                        frames.push(Frame::Visit(tail, direct_destination));
                        for statement in statements.iter().rev() {
                            match statement {
                                ResolvedStatement::Let { binding, value, .. } => {
                                    frames.push(Frame::Argument(
                                        value,
                                        Some((
                                            binding.id.as_str().to_owned(),
                                            ArrayStorageKind::Binding,
                                            binding.ty.clone(),
                                        )),
                                        true,
                                    ));
                                }
                                ResolvedStatement::Assign { value, .. } => {
                                    frames.push(Frame::Visit(value, true));
                                }
                                ResolvedStatement::Unsafe { body, .. } => {
                                    frames.push(Frame::Visit(body, true));
                                }
                                ResolvedStatement::While {
                                    condition, body, ..
                                } => {
                                    frames.push(Frame::Loop);
                                    frames.push(Frame::Visit(body, false));
                                    frames.push(Frame::Visit(condition, false));
                                }
                            }
                        }
                    }
                    ResolvedExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        frames.push(Frame::Sequence(2));
                        frames.push(Frame::Alternative(2));
                        frames.push(Frame::Visit(else_branch, direct_destination));
                        frames.push(Frame::Visit(then_branch, direct_destination));
                        frames.push(Frame::Visit(condition, false));
                    }
                    ResolvedExprKind::ConstructRecord { fields, .. }
                    | ResolvedExprKind::ConstructVariant { fields, .. } => {
                        frames.push(Frame::Sequence(fields.len()));
                        for field in fields.iter().rev() {
                            frames.push(Frame::Visit(&field.value, false));
                        }
                    }
                    ResolvedExprKind::Match {
                        scrutinee, arms, ..
                    } => {
                        for arm in arms {
                            push_array_pattern_slots(program, &arm.pattern, slots)?;
                        }
                        frames.push(Frame::Match(arms.len()));
                        frames.push(Frame::Visit(scrutinee, false));
                        for arm in arms.iter().rev() {
                            frames.push(Frame::Sequence(1 + usize::from(arm.guard.is_some())));
                            frames.push(Frame::Visit(&arm.value, direct_destination));
                            if let Some(guard) = &arm.guard {
                                frames.push(Frame::Visit(guard, false));
                            }
                        }
                    }
                    ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                        frames.push(Frame::Sequence(1 + fields.len()));
                        for field in fields.iter().rev() {
                            frames.push(Frame::Visit(&field.value, false));
                        }
                        frames.push(Frame::Visit(base, false));
                    }
                    ResolvedExprKind::Int(_)
                    | ResolvedExprKind::Int32(_)
                    | ResolvedExprKind::Char(_)
                    | ResolvedExprKind::Uint8(_)
                    | ResolvedExprKind::Usize(_)
                    | ResolvedExprKind::ArrayU8(_)
                    | ResolvedExprKind::RepeatArrayU8 { .. }
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_)
                    | ResolvedExprKind::Bool(_)
                    | ResolvedExprKind::String(_)
                    | ResolvedExprKind::Place(_)
                    | ResolvedExprKind::BorrowPlace { .. } => results.push(CapacityFlow::Empty),
                }
            }
            Frame::Argument(expression, slot, direct_destination) => {
                if let Some((identity, kind, ty)) = slot {
                    push_array_slot(program, slots, identity, kind, &ty)?;
                }
                frames.push(Frame::Visit(expression, direct_destination));
            }
            Frame::Sequence(count) => {
                let start = results
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| hir_error("byte-capacity traversal stack underflowed"))?;
                let children = results.drain(start..).collect::<Vec<_>>();
                results.push(sequence(children));
            }
            Frame::Alternative(count) => {
                let start = results
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| hir_error("byte-capacity traversal stack underflowed"))?;
                let children = results.drain(start..).collect::<Vec<_>>();
                results.push(CapacityFlow::Alternative(children));
            }
            Frame::Loop => {
                let body = results
                    .pop()
                    .ok_or_else(|| hir_error("byte-capacity traversal stack underflowed"))?;
                let condition = results
                    .pop()
                    .ok_or_else(|| hir_error("byte-capacity traversal stack underflowed"))?;
                results.push(CapacityFlow::Loop {
                    condition: Box::new(condition),
                    body: Box::new(body),
                });
            }
            Frame::Match(arm_count) => {
                let scrutinee = results
                    .pop()
                    .ok_or_else(|| hir_error("byte-capacity traversal stack underflowed"))?;
                let start = results
                    .len()
                    .checked_sub(arm_count)
                    .ok_or_else(|| hir_error("byte-capacity traversal stack underflowed"))?;
                let alternatives = results.drain(start..).collect::<Vec<_>>();
                results.push(sequence(vec![
                    scrutinee,
                    CapacityFlow::Alternative(alternatives),
                ]));
            }
            Frame::Emit(flow) => results.push(flow),
        }
    }
    if results.len() == 1 {
        results
            .pop()
            .ok_or_else(|| hir_error("byte-capacity traversal produced no result"))
    } else {
        Err(hir_error(
            "byte-capacity traversal produced an invalid result stack",
        ))
    }
}

pub(crate) fn byte_data_capacity_inputs(
    program: &ResolvedProgram,
) -> Result<Vec<crate::byte_data_capacity::FunctionCapacityInput>, Diagnostic> {
    use crate::byte_data_capacity::{ArrayStorageKind, CapacityFlow, FunctionCapacityInput};

    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| (instance.id.as_str(), &instance.function)),
        );
    functions
        .map(|(identity, function)| {
            let mut slots = Vec::new();
            for parameter in &function.params {
                push_array_slot(
                    program,
                    &mut slots,
                    parameter.id.as_str().to_owned(),
                    ArrayStorageKind::Parameter,
                    &parameter.ty,
                )?;
            }
            push_array_slot(
                program,
                &mut slots,
                function.result_id.as_str().to_owned(),
                ArrayStorageKind::ProvisionalResult,
                &function.return_type,
            )?;
            let mut execution = function
                .requires
                .iter()
                .map(|expression| byte_capacity_expression(program, expression, &mut slots, false))
                .collect::<Result<Vec<_>, _>>()?;
            execution.push(byte_capacity_expression(
                program,
                &function.body,
                &mut slots,
                true,
            )?);
            execution.extend(
                function
                    .ensures
                    .iter()
                    .map(|expression| {
                        byte_capacity_expression(program, expression, &mut slots, false)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            Ok(FunctionCapacityInput {
                function: identity.to_owned(),
                array_slots: slots,
                execution: CapacityFlow::Sequence(execution),
            })
        })
        .collect()
}

pub(crate) fn analyze_byte_data_capacity(
    program: &ResolvedProgram,
) -> Result<crate::byte_data_capacity::ProgramCapacitySummary, Diagnostic> {
    let inputs = byte_data_capacity_inputs(program)?;
    crate::byte_data_capacity::analyze(&inputs)
        .map_err(|error| Diagnostic::io(error.diagnostic.code(), error.to_string()))
}
