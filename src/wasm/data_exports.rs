//! Admission and raw-wrapper planning for Public Useful Data Export v1.
//!
//! The public boundary is deliberately smaller than the internal byte-data
//! language. Selected functions accept only full-root borrowed byte slices and
//! return one scalar. Raw wrappers alone expand each slice into an authenticated
//! scratch-memory `(offset, length)` pair.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, ByteSliceExtent, ByteSliceRootKind, DeclarationId, IdentityOrigin, OwnershipMode,
    ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedProgram, ResolvedStatement,
    ResolvedType, ValueId,
};

use super::{write_i64, write_u32, ByteOutput, I32, I64};

pub(super) const STATUS_GLOBAL_EXPORT: &str = "__spx_data_status_v1";
pub(super) const SCRATCH_BASE_EXPORT: &str = "__spx_data_scratch_base_v1";
pub(super) const SCRATCH_CAPACITY_EXPORT: &str = "__spx_data_scratch_capacity_v1";
pub(super) const MEMORY_EXPORT: &str = "memory";
pub(super) const SCRATCH_BASE: u32 = 0;
pub(super) const SCRATCH_CAPACITY: u32 = 65_536;
pub(super) const FIXED_MEMORY_PAGES: u8 = 2;
pub(super) const BOUNDARY_STATUS: i32 = 11;

const MAX_EXPORTS: usize = 32;
const MAX_FUNCTIONS: usize = 256;
const MAX_PARAMETERS: usize = 8;
const MAX_STABLE_ID_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DataResultType {
    I64,
    Bool,
    Usize,
}

impl DataResultType {
    pub(super) const fn raw_wasm_type(self) -> u8 {
        match self {
            Self::I64 | Self::Usize => I64,
            Self::Bool => I32,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DataExportPlan {
    pub(super) stable_id: String,
    pub(super) wasm_export: String,
    pub(super) function_id: DeclarationId,
    pub(super) parameter_count: usize,
    pub(super) result: DataResultType,
}

impl DataExportPlan {
    pub(super) fn raw_params(&self) -> Vec<u8> {
        vec![I32; self.parameter_count * 2]
    }

    pub(super) fn emit_wrapper_body(
        &self,
        target_index: u32,
        stack_global_index: u32,
        status_global_index: u32,
    ) -> Result<Vec<u8>, Diagnostic> {
        self.emit_wrapper_body_profile(
            target_index,
            stack_global_index,
            status_global_index,
            None,
            false,
        )
    }

    pub(super) fn emit_wrapper_body_with_stdout_transcript(
        &self,
        target_index: u32,
        stack_global_index: u32,
        status_global_index: u32,
        globals: super::host_output::Globals,
    ) -> Result<Vec<u8>, Diagnostic> {
        self.emit_wrapper_body_profile(
            target_index,
            stack_global_index,
            status_global_index,
            Some(globals),
            false,
        )
    }

    pub(super) fn emit_command_v2_wrapper_body(
        &self,
        target_index: u32,
        stack_global_index: u32,
        status_global_index: u32,
        globals: super::host_output::Globals,
    ) -> Result<Vec<u8>, Diagnostic> {
        self.emit_wrapper_body_profile(
            target_index,
            stack_global_index,
            status_global_index,
            Some(globals),
            true,
        )
    }

    fn emit_wrapper_body_profile(
        &self,
        target_index: u32,
        stack_global_index: u32,
        status_global_index: u32,
        host_output: Option<super::host_output::Globals>,
        publish_only_truthy: bool,
    ) -> Result<Vec<u8>, Diagnostic> {
        let raw_count = u32::try_from(self.parameter_count * 2)
            .map_err(|_| admission("data wrapper raw parameter count overflows u32"))?;
        let old_stack = raw_count;
        let result_out = raw_count + 1;
        let status = raw_count + 2;
        let charged = raw_count + 3;
        let mut body = Vec::new();
        write_u32(&mut body, 1);
        write_u32(&mut body, 4);
        body.push(I32);

        if let Some(globals) = host_output {
            super::host_output::emit_reset(&mut body, globals);
        }

        i32_const(&mut body, 0);
        global_set(&mut body, status_global_index);
        i32_const(&mut body, 0);
        local_set(&mut body, charged);

        for parameter in 0..self.parameter_count {
            let offset = u32::try_from(parameter * 2)
                .map_err(|_| admission("data wrapper offset index overflows u32"))?;
            let length = offset + 1;

            // length <= capacity
            local_get(&mut body, length);
            i32_const(&mut body, SCRATCH_CAPACITY as i32);
            body.push(0x4b); // i32.gt_u
            emit_boundary_return(&mut body, status_global_index, self.result);

            // offset <= capacity - length; subtraction is now proven safe.
            local_get(&mut body, offset);
            i32_const(&mut body, SCRATCH_CAPACITY as i32);
            local_get(&mut body, length);
            body.push(0x6b); // i32.sub
            body.push(0x4b); // i32.gt_u
            emit_boundary_return(&mut body, status_global_index, self.result);

            // charged <= capacity - length before the cumulative addition.
            local_get(&mut body, charged);
            i32_const(&mut body, SCRATCH_CAPACITY as i32);
            local_get(&mut body, length);
            body.push(0x6b); // i32.sub
            body.push(0x4b); // i32.gt_u
            emit_boundary_return(&mut body, status_global_index, self.result);
            local_get(&mut body, charged);
            local_get(&mut body, length);
            body.push(0x6a); // i32.add
            local_set(&mut body, charged);
        }

        global_get(&mut body, stack_global_index);
        local_tee(&mut body, old_stack);
        i32_const(&mut body, 8);
        body.push(0x49); // i32.lt_u
        body.extend_from_slice(&[0x04, 0x40, 0x00, 0x0b]); // invariant trap
        local_get(&mut body, old_stack);
        i32_const(&mut body, 8);
        body.push(0x6b); // i32.sub
        local_tee(&mut body, result_out);
        global_set(&mut body, stack_global_index);

        for parameter in 0..self.parameter_count {
            let offset = u32::try_from(parameter * 2)
                .map_err(|_| admission("data wrapper offset index overflows u32"))?;
            let length = offset + 1;
            local_get(&mut body, offset);
            body.push(0xad); // i64.extend_i32_u
            i64_const(&mut body, 32);
            body.push(0x86); // i64.shl
            local_get(&mut body, length);
            body.push(0xad); // i64.extend_i32_u
            body.push(0x84); // i64.or: root-high / length-low
        }
        local_get(&mut body, result_out);
        call(&mut body, target_index);
        local_set(&mut body, status);
        local_get(&mut body, old_stack);
        global_set(&mut body, stack_global_index);
        local_get(&mut body, status);
        global_set(&mut body, status_global_index);
        local_get(&mut body, status);
        body.extend_from_slice(&[0x04, 0x40]);
        if let Some(globals) = host_output {
            super::host_output::emit_discard(&mut body, globals);
        }
        emit_zero(&mut body, self.result);
        body.push(0x0f); // return without observing the unpublished result slot
        body.push(0x0b);

        local_get(&mut body, result_out);
        match self.result {
            DataResultType::I64 | DataResultType::Usize => {
                body.extend_from_slice(&[0x29, 0x03, 0x00]); // i64.load align=8
            }
            DataResultType::Bool => {
                body.extend_from_slice(&[0x28, 0x02, 0x00]); // i32.load align=4
                local_tee(&mut body, status);
                i32_const(&mut body, 1);
                body.push(0x4b); // i32.gt_u
                body.extend_from_slice(&[0x04, 0x40, 0x00, 0x0b]);
                local_get(&mut body, status);
            }
        }
        if let Some(globals) = host_output {
            // The bool carrier check above is still part of target
            // authentication. Seal only after it has succeeded.
            if publish_only_truthy {
                debug_assert_eq!(self.result, DataResultType::Bool);
                local_get(&mut body, status);
                body.extend_from_slice(&[0x45, 0x04, 0x40]); // i32.eqz; if void
                super::host_output::emit_discard(&mut body, globals);
                body.push(0x05); // else
                super::host_output::emit_publish(&mut body, globals);
                body.push(0x0b);
            } else {
                super::host_output::emit_publish(&mut body, globals);
            }
        }
        body.push(0x0b);
        Ok(body)
    }
}

pub(super) fn prepare(
    program: &ResolvedProgram,
    export_ids: &[String],
) -> Result<Vec<DataExportPlan>, Diagnostic> {
    prepare_profile(program, export_ids, false)
}

pub(super) fn prepare_with_stdout_transcript(
    program: &ResolvedProgram,
    export_ids: &[String],
) -> Result<Vec<DataExportPlan>, Diagnostic> {
    prepare_profile(program, export_ids, true)
}

pub(super) fn prepare_command_v2(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<Vec<DataExportPlan>, Diagnostic> {
    crate::command_profile::CommandProfilePlan::prepare(program, command_id)?;
    prepare_profile(program, &[command_id.to_owned()], true)
}

fn prepare_profile(
    program: &ResolvedProgram,
    export_ids: &[String],
    host_output: bool,
) -> Result<Vec<DataExportPlan>, Diagnostic> {
    validate_selection(export_ids)?;
    hir::validate(program)?;
    let permits_are_admitted = if host_output {
        program.permits == [crate::host_io_ops::STDOUT_WRITE_EFFECT]
    } else {
        program.permits.is_empty()
    };
    if !permits_are_admitted || !program.interfaces.is_empty() {
        return Err(admission(
            "Public Useful Data Export v1 does not admit permits or interfaces",
        ));
    }
    if !program.function_templates.is_empty() || !program.function_instances.is_empty() {
        return Err(admission(
            "Public Useful Data Export v1 does not admit generic functions",
        ));
    }
    if program.functions.is_empty() || program.functions.len() > MAX_FUNCTIONS {
        return Err(capacity(format!(
            "Public Useful Data Export v1 admits 1..={MAX_FUNCTIONS} functions"
        )));
    }
    if program.types.iter().any(|declaration| {
        program
            .declarations
            .declaration(&declaration.id)
            .is_none_or(|item| item.identity_origin != IdentityOrigin::CompilerOwned)
    }) {
        return Err(admission(
            "Public Useful Data Export v1 does not admit authored aggregates or resources",
        ));
    }

    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let selected_ids = export_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let entry = functions
        .get(program.entrypoint.as_str())
        .copied()
        .ok_or_else(|| admission("data-profile entrypoint is absent"))?;
    if entry.name != "main" || !entry.params.is_empty() || entry.return_type != ResolvedType::I64 {
        return Err(admission(
            "Public Useful Data Export v1 entrypoint must be an exact `fn main() -> i64`",
        ));
    }

    // The aggregate emitter materializes the complete function inventory.
    // Validate exactly that inventory, not merely the selected call closure.
    let mut call_graph = BTreeMap::new();
    for function in &program.functions {
        let mut callees = Vec::new();
        let stdout_external_roots = (host_output && selected_ids.contains(function.id.as_str()))
            .then(|| {
                function
                    .params
                    .iter()
                    .filter(|parameter| parameter.ty == ResolvedType::SliceU8)
                    .map(|parameter| parameter.id.clone())
                    .collect::<BTreeSet<_>>()
            });
        validate_function(
            program,
            function,
            &functions,
            &mut callees,
            host_output,
            stdout_external_roots.as_ref(),
        )?;
        callees.sort();
        callees.dedup();
        call_graph.insert(function.id.clone(), callees);
    }
    reject_call_cycles(&call_graph)?;

    let mut sorted = export_ids.to_vec();
    sorted.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    let mut symbols = BTreeSet::new();
    let mut plans = Vec::with_capacity(sorted.len());
    for stable_id in sorted {
        let function = functions.get(stable_id.as_str()).copied().ok_or_else(|| {
            admission(format!(
                "selected data export identity `{stable_id}` does not name a monomorphic function"
            ))
        })?;
        require_explicit(program, function)?;
        if function.params.is_empty() || function.params.len() > MAX_PARAMETERS {
            return Err(capacity(format!(
                "Public Useful Data Export v1 function `{}` requires 1..={MAX_PARAMETERS} slice parameters",
                function.id
            )));
        }
        for parameter in &function.params {
            if parameter.ty != ResolvedType::SliceU8 || parameter.ownership != OwnershipMode::Borrow
            {
                return Err(admission(format!(
                    "selected data export `{stable_id}` accepts only `borrow Slice<u8>` parameters"
                )));
            }
            let provenance = program
                .declarations
                .byte_slice_provenance(&parameter.id)
                .ok_or_else(|| admission("selected byte-slice parameter lacks provenance"))?;
            if provenance.root != parameter.id
                || !provenance.projections.is_empty()
                || provenance.projected_type != ResolvedType::SliceU8
                || provenance.root_kind != ByteSliceRootKind::FunctionParameter
                || provenance.root_length != ByteSliceExtent::ParameterLength
                || provenance.offset != ByteSliceExtent::Constant(0)
                || provenance.length != ByteSliceExtent::ParameterLength
                || provenance.producer.is_some()
            {
                return Err(admission(format!(
                    "selected data export `{stable_id}` parameter is not an exact full external root"
                )));
            }
        }
        let result = public_result(&function.return_type).ok_or_else(|| {
            admission(format!(
                "selected data export `{stable_id}` must return i64, bool, or usize"
            ))
        })?;
        let wasm_export = raw_symbol(&stable_id);
        if !symbols.insert(wasm_export.clone()) {
            return Err(admission(format!(
                "selected data export `{stable_id}` collides with another raw symbol"
            )));
        }
        plans.push(DataExportPlan {
            stable_id,
            wasm_export,
            function_id: function.id.clone(),
            parameter_count: function.params.len(),
            result,
        });
    }
    Ok(plans)
}

fn validate_function(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    functions: &BTreeMap<&str, &ResolvedFunction>,
    callees: &mut Vec<DeclarationId>,
    host_output: bool,
    stdout_external_roots: Option<&BTreeSet<ValueId>>,
) -> Result<(), Diagnostic> {
    let effects_are_admitted = if host_output {
        function.effects.is_empty() || function.effects == [crate::host_io_ops::STDOUT_WRITE_EFFECT]
    } else {
        function.effects.is_empty()
    };
    if !effects_are_admitted || !function.requires.is_empty() || !function.ensures.is_empty() {
        return Err(admission(format!(
            "Public Useful Data Export v1 function `{}` must be effect- and contract-free",
            function.id
        )));
    }
    for parameter in &function.params {
        if !internal_parameter(&parameter.ty, parameter.ownership) {
            return Err(admission(format!(
                "Public Useful Data Export v1 function `{}` has an unsupported parameter",
                function.id
            )));
        }
    }
    if !internal_result(&function.return_type) {
        return Err(admission(format!(
            "Public Useful Data Export v1 function `{}` has an unsupported result",
            function.id
        )));
    }

    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        if !internal_expression_type(&expression.ty) {
            return Err(admission(format!(
                "Public Useful Data Export v1 function `{}` reaches an unsupported type",
                function.id
            )));
        }
        match &expression.kind {
            ResolvedExprKind::String(_)
            | ResolvedExprKind::NativeRustImportCall(_)
            | ResolvedExprKind::HostCommandCall(_) => {
                return Err(admission(format!(
                    "Public Useful Data Export v1 function `{}` reaches text allocation, an import, or command I/O",
                    function.id
                )));
            }
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            } => {
                if instance.is_some() || !type_arguments.is_empty() {
                    return Err(admission(format!(
                        "Public Useful Data Export v1 function `{}` reaches a generic call",
                        function.id
                    )));
                }
                pending.extend(args);
                if host_output && crate::host_io_ops::by_id(callee.as_str()).is_some() {
                    let roots = stdout_external_roots.ok_or_else(|| {
                        admission(format!(
                            "Public Useful Data Export v1 function `{}` may write stdout only from the selected command boundary",
                            function.id
                        ))
                    })?;
                    validate_stdout_external_argument(program, function, args, roots)?;
                } else if crate::byte_ops::by_id(callee.as_str()).is_none() {
                    if !functions.contains_key(callee.as_str()) {
                        return Err(admission(format!(
                            "Public Useful Data Export v1 function `{}` reaches unavailable call `{callee}`",
                            function.id
                        )));
                    }
                    callees.push(callee.clone());
                }
            }
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    if matches!(statement, ResolvedStatement::Unsafe { .. }) {
                        return Err(admission(format!(
                            "Public Useful Data Export v1 function `{}` reaches an unsafe boundary",
                            function.id
                        )));
                    }
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child);
                        }
                    }
                }
                pending.push(tail);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(condition);
                pending.push(then_branch);
                pending.push(else_branch);
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.push(scrutinee);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        pending.push(guard);
                    }
                    pending.push(&arm.value);
                }
            }
            ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
                pending.push(operand)
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push(base);
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Project { base, .. } => pending.push(base),
            ResolvedExprKind::Upcast { source } => pending.push(source),
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => pending.extend([source.as_ref(), start.as_ref(), end.as_ref()]),
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::Place(_) => {}
            ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_) => {
                return Err(admission(format!(
                    "Public Useful Data Export v1 function `{}` reaches a non-profile scalar",
                    function.id
                )));
            }
        }
    }
    let _ = program;
    Ok(())
}

fn validate_stdout_external_argument(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    args: &[ResolvedExpr],
    external_roots: &BTreeSet<ValueId>,
) -> Result<(), Diagnostic> {
    let [argument] = args else {
        return Err(admission("stdout_write argument inventory is not exact"));
    };
    let place = match &argument.kind {
        ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. }
            if place.projections.is_empty() => place,
        _ => {
            return Err(admission(format!(
                "Public Useful Data Export v1 function `{}` must write an external Slice parameter or immutable alias",
                function.id
            )))
        }
    };
    let provenance = program
        .declarations
        .byte_slice_provenance(&place.root)
        .ok_or_else(|| admission("stdout_write operand lacks authenticated byte provenance"))?;
    if provenance.root_kind != ByteSliceRootKind::FunctionParameter
        || !provenance.projections.is_empty()
        || provenance.projected_type != ResolvedType::SliceU8
        || provenance.root_length != ByteSliceExtent::ParameterLength
        || provenance.offset != ByteSliceExtent::Constant(0)
        || provenance.length != ByteSliceExtent::ParameterLength
        || !external_roots.contains(&provenance.root)
    {
        return Err(admission(format!(
            "Public Useful Data Export v1 function `{}` stdout_write operand is not rooted in a selected external Slice parameter",
            function.id
        )));
    }
    Ok(())
}

fn internal_parameter(ty: &ResolvedType, ownership: OwnershipMode) -> bool {
    match ty {
        ResolvedType::Bytes => ownership == OwnershipMode::Own,
        ResolvedType::SliceU8 => ownership == OwnershipMode::Borrow,
        _ => ownership == OwnershipMode::Value && internal_result(ty),
    }
}

fn internal_result(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::Bool
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::ArrayU8(_)
            | ResolvedType::Bytes
    ) || ty.is_compiler_byte_option()
}

fn internal_expression_type(ty: &ResolvedType) -> bool {
    *ty == ResolvedType::Unit || internal_result(ty) || *ty == ResolvedType::SliceU8
}

fn public_result(ty: &ResolvedType) -> Option<DataResultType> {
    match ty {
        ResolvedType::I64 => Some(DataResultType::I64),
        ResolvedType::Bool => Some(DataResultType::Bool),
        ResolvedType::Usize => Some(DataResultType::Usize),
        _ => None,
    }
}

fn require_explicit(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    if program
        .declarations
        .declaration(&function.id)
        .is_none_or(|declaration| declaration.identity_origin != IdentityOrigin::Explicit)
    {
        return Err(admission(format!(
            "Public Useful Data Export v1 function `{}` must have an explicit stable identity",
            function.id
        )));
    }
    Ok(())
}

fn reject_call_cycles(
    call_graph: &BTreeMap<DeclarationId, Vec<DeclarationId>>,
) -> Result<(), Diagnostic> {
    fn visit(
        id: &DeclarationId,
        call_graph: &BTreeMap<DeclarationId, Vec<DeclarationId>>,
        active: &mut BTreeSet<DeclarationId>,
        complete: &mut BTreeSet<DeclarationId>,
    ) -> Result<(), Diagnostic> {
        if complete.contains(id) {
            return Ok(());
        }
        if !active.insert(id.clone()) {
            return Err(admission(format!(
                "Public Useful Data Export v1 reaches a recursive call cycle at `{id}`"
            )));
        }
        if let Some(callees) = call_graph.get(id) {
            for callee in callees {
                visit(callee, call_graph, active, complete)?;
            }
        }
        active.remove(id);
        complete.insert(id.clone());
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for id in call_graph.keys() {
        visit(id, call_graph, &mut active, &mut complete)?;
    }
    Ok(())
}

fn validate_selection(ids: &[String]) -> Result<(), Diagnostic> {
    if !(1..=MAX_EXPORTS).contains(&ids.len()) {
        return Err(capacity(format!(
            "Public Useful Data Export v1 requires 1..={MAX_EXPORTS} selected IDs"
        )));
    }
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.as_str()) {
            return Err(admission(format!(
                "selected data export ID `{id}` appears more than once"
            )));
        }
        if !(1..=MAX_STABLE_ID_BYTES).contains(&id.len()) {
            return Err(capacity(format!(
                "data export IDs must contain 1..={MAX_STABLE_ID_BYTES} bytes"
            )));
        }
        if !id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        }) {
            return Err(admission(format!(
                "data export ID `{id}` must use lowercase [a-z0-9._-]"
            )));
        }
    }
    Ok(())
}

pub(super) fn raw_symbol(stable_id: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut symbol = String::with_capacity(9 + stable_id.len() * 2);
    symbol.push_str("spx_data_");
    for byte in stable_id.bytes() {
        symbol.push(HEX[(byte >> 4) as usize] as char);
        symbol.push(HEX[(byte & 0x0f) as usize] as char);
    }
    symbol
}

fn emit_boundary_return(body: &mut impl ByteOutput, status_global: u32, result: DataResultType) {
    body.extend_bytes(&[0x04, 0x40]);
    i32_const(body, BOUNDARY_STATUS);
    global_set(body, status_global);
    emit_zero(body, result);
    body.push(0x0f);
    body.push(0x0b);
}

fn emit_zero(body: &mut impl ByteOutput, result: DataResultType) {
    match result {
        DataResultType::I64 | DataResultType::Usize => i64_const(body, 0),
        DataResultType::Bool => i32_const(body, 0),
    }
}

fn local_get(body: &mut impl ByteOutput, index: u32) {
    body.push(0x20);
    write_u32(body, index);
}

fn local_set(body: &mut impl ByteOutput, index: u32) {
    body.push(0x21);
    write_u32(body, index);
}

fn local_tee(body: &mut impl ByteOutput, index: u32) {
    body.push(0x22);
    write_u32(body, index);
}

fn global_get(body: &mut impl ByteOutput, index: u32) {
    body.push(0x23);
    write_u32(body, index);
}

fn global_set(body: &mut impl ByteOutput, index: u32) {
    body.push(0x24);
    write_u32(body, index);
}

fn call(body: &mut impl ByteOutput, index: u32) {
    body.push(0x10);
    write_u32(body, index);
}

fn i32_const(body: &mut impl ByteOutput, value: i32) {
    body.push(0x41);
    write_i64(body, i64::from(value));
}

fn i64_const(body: &mut impl ByteOutput, value: i64) {
    body.push(0x42);
    write_i64(body, value);
}

fn admission(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W121", message)
}

fn capacity(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W122", message)
}

#[cfg(test)]
#[path = "data_exports/tests.rs"]
mod tests;
