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
        self.emit_wrapper_body_profile(target_index, stack_global_index, status_global_index, None)
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
        )
    }

    fn emit_wrapper_body_profile(
        &self,
        target_index: u32,
        stack_global_index: u32,
        status_global_index: u32,
        host_output: Option<super::host_output::Globals>,
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
            super::host_output::emit_publish(&mut body, globals);
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
            ResolvedExprKind::String(_) | ResolvedExprKind::NativeRustImportCall(_) => {
                return Err(admission(format!(
                    "Public Useful Data Export v1 function `{}` reaches text allocation or an import",
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
            ResolvedExprKind::Match { scrutinee, arms } => {
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
mod tests {
    use std::path::Path;

    use super::{prepare, prepare_with_stdout_transcript, raw_symbol, DataResultType};

    const SOURCE: &str = r#"
module test.data_exports;
@id("data.main")
fn main() -> i64 { 0 }
@id("data.length")
fn length(value: borrow Slice<u8>) -> usize { byte_len(value) }
@id("data.present")
fn present(value: borrow Slice<u8>) -> bool {
    match byte_get(value, 0usize) {
        Option::Some { value: byte } => byte == 255u8,
        Option::None {} => false,
    }
}
@id("data.copy")
fn copy(value: borrow Slice<u8>) -> i64 {
    let owned = bytes_copy(value);
    let view = bytes_as_slice(owned);
    if byte_len(view) == 0usize { 0 } else { 1 }
}
@id("data.total")
fn total(left: borrow Slice<u8>, right: borrow Slice<u8>) -> usize {
    byte_len(left) + byte_len(right)
}
@id("data.fail")
fn fail(value: borrow Slice<u8>) -> i64 {
    let owned = bytes_copy(value);
    let view = bytes_as_slice(owned);
    if byte_len(view) == 3usize { 9223372036854775807 + 1 } else { 0 }
}
"#;

    #[test]
    fn exact_public_abi_is_sorted_and_stable() {
        let parsed = crate::parse(SOURCE, Path::new("data-exports.spx")).unwrap();
        let resolved = crate::hir::resolve(&parsed).unwrap();
        let plans = prepare(
            &resolved,
            &["data.present".to_owned(), "data.length".to_owned()],
        )
        .unwrap();
        assert_eq!(plans[0].stable_id, "data.length");
        assert_eq!(plans[0].result, DataResultType::Usize);
        assert_eq!(plans[1].result, DataResultType::Bool);
        assert_eq!(raw_symbol("data.length"), "spx_data_646174612e6c656e677468");
    }

    #[test]
    fn hostile_public_shapes_and_closed_programs_are_rejected() {
        let parsed = crate::parse(SOURCE, Path::new("data-exports.spx")).unwrap();
        let resolved = crate::hir::resolve(&parsed).unwrap();
        assert!(prepare(&resolved, &[]).is_err());
        assert!(prepare(&resolved, &["missing".to_owned()]).is_err());
        assert!(prepare(
            &resolved,
            &["data.length".to_owned(), "data.length".to_owned()]
        )
        .is_err());
        assert!(prepare(&resolved, &["data.main".to_owned()]).is_err());

        let scalar = crate::parse(
            &SOURCE.replace(
                "fn length(value: borrow Slice<u8>) -> usize { byte_len(value) }",
                "fn length(value: i64) -> usize { 0usize }",
            ),
            Path::new("data-export-scalar.spx"),
        )
        .unwrap();
        assert!(prepare(
            &crate::hir::resolve(&scalar).unwrap(),
            &["data.length".to_owned()]
        )
        .is_err());

        let contracted = crate::parse(
            &SOURCE.replace(
                "fn length(value: borrow Slice<u8>) -> usize {",
                "fn length(value: borrow Slice<u8>) -> usize requires true {",
            ),
            Path::new("data-export-contract.spx"),
        )
        .unwrap();
        assert!(prepare(
            &crate::hir::resolve(&contracted).unwrap(),
            &["data.length".to_owned()]
        )
        .is_err());
    }

    #[test]
    fn command_stdout_accepts_only_selected_external_slice_roots() {
        const EXTERNAL: &str = r#"
module test.command_external;
permit { process.stdout.write }
@id("command.run")
fn run(input: borrow Slice<u8>) -> bool uses { process.stdout.write } {
    let alias = input;
    stdout_write(alias) == byte_len(input)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
        let parsed = crate::parse(EXTERNAL, Path::new("command-external.spx")).unwrap();
        let resolved = crate::hir::resolve(&parsed).unwrap();
        prepare_with_stdout_transcript(&resolved, &["command.run".to_owned()]).unwrap();

        for (name, replacement) in [
            (
                "array",
                "let local = [65u8]; let view = array_as_slice(local); stdout_write(view) == 1usize",
            ),
            (
                "owned",
                "let owned = bytes_copy(input); let view = bytes_as_slice(owned); stdout_write(view) == byte_len(input)",
            ),
        ] {
            let hostile = EXTERNAL.replace(
                "let alias = input;\n    stdout_write(alias) == byte_len(input)",
                replacement,
            );
            let parsed = crate::parse(
                &hostile,
                Path::new(&format!("command-hostile-{name}.spx")),
            )
            .unwrap();
            let resolved = crate::hir::resolve(&parsed).unwrap();
            let error = prepare_with_stdout_transcript(&resolved, &["command.run".to_owned()])
                .unwrap_err();
            assert_eq!(error.code, "SPX-W121");
            assert!(error.message.contains("external Slice parameter"));
        }

        let helper = EXTERNAL
            .replace(
                "@id(\"command.run\")",
                r#"@id("command.helper")
fn helper(input: borrow Slice<u8>) -> usize uses { process.stdout.write } {
    stdout_write(input)
}
@id("command.run")"#,
            )
            .replace("stdout_write(alias)", "helper(alias)");
        let parsed = crate::parse(&helper, Path::new("command-helper-write.spx")).unwrap();
        let resolved = crate::hir::resolve(&parsed).unwrap();
        let error =
            prepare_with_stdout_transcript(&resolved, &["command.run".to_owned()]).unwrap_err();
        assert_eq!(error.code, "SPX-W121");
        assert!(error.message.contains("selected command boundary"));
    }

    #[test]
    fn throwing_checked_import_cannot_expose_staged_stdout_bytes() {
        use std::process::Command;

        if Command::new("node").arg("--version").output().is_err() {
            return;
        }
        const COMMAND: &str = r#"
module test.command_throw;
permit { process.stdout.write }
@id("command.run")
fn run(input: borrow Slice<u8>) -> bool uses { process.stdout.write } {
    let written = stdout_write(input);
    match byte_get(input, 0usize) {
        Option::Some { value } => written == byte_len(input) && value == 65u8,
        Option::None {} => false,
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
        let parsed = crate::parse(COMMAND, Path::new("command-throw.spx")).unwrap();
        let resolved = crate::hir::resolve(&parsed).unwrap();
        let plans = prepare_with_stdout_transcript(&resolved, &["command.run".to_owned()]).unwrap();
        let wasm =
            crate::wasm::aggregate::emit_byte_exports_with_stdout_transcript(&resolved, &plans)
                .unwrap();
        wasmparser::Validator::new().validate_all(&wasm).unwrap();

        let root = std::env::temp_dir().join(format!(
            "semaprax-command-throw-{}-{}",
            std::process::id(),
            wasm.len()
        ));
        std::fs::create_dir(&root).unwrap();
        let wasm_path = root.join("app.wasm");
        let script_path = root.join("probe.mjs");
        std::fs::write(&wasm_path, wasm).unwrap();
        let script = format!(
            r#"import {{ readFile }} from "node:fs/promises";
let instance;
const imports={{env:{{
spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{{throw Error("contract");}},
spx_bytes_copy:()=>{{throw Error("unused copy");}},spx_bytes_get:()=>{{throw Error("injected checked read failure");}},spx_bytes_drop:()=>{{throw Error("unused drop");}},spx_bytes_as_slice:()=>{{throw Error("unused slice");}}
}}}};
({{instance}}=await WebAssembly.instantiate(await readFile(process.argv[2]),imports));
const e=instance.exports,memory=new Uint8Array(e.memory.buffer);memory.set([65,0,66],0);
let failed=false;try{{e["{symbol}"](0,3)}}catch{{failed=true}}if(!failed)throw Error("import failure hidden");
if(e.__spx_stdout_length_v1.value!==0)throw Error("failed length published");
if(memory.subarray(131072,196608).some(byte=>byte!==0))throw Error("staged bytes escaped");
console.log("command-throw-pristine");
"#,
            symbol = raw_symbol("command.run")
        );
        std::fs::write(&script_path, script).unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        let _ = std::fs::remove_dir(&root);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"command-throw-pristine\n");
    }

    #[test]
    fn core_wasm_exports_only_the_exact_public_boundary_and_node_executes_it() {
        use std::process::Command;

        use wasmparser::{ExternalKind, Parser, Payload, TypeRef, Validator};

        let parsed = crate::parse(SOURCE, Path::new("data-exports-node.spx")).unwrap();
        let selected = [
            "data.copy".to_owned(),
            "data.fail".to_owned(),
            "data.length".to_owned(),
            "data.present".to_owned(),
            "data.total".to_owned(),
        ];
        let first = crate::wasm::emit_module_with_byte_exports(&parsed, &selected).unwrap();
        let second = crate::wasm::emit_module_with_byte_exports(&parsed, &selected).unwrap();
        assert_eq!(first, second);
        Validator::new().validate_all(&first).unwrap();

        let mut imports = Vec::new();
        let mut exports = Vec::new();
        for payload in Parser::new(0).parse_all(&first) {
            match payload.unwrap() {
                Payload::ImportSection(section) => {
                    for import in section.into_imports() {
                        let import = import.unwrap();
                        let TypeRef::Func(_) = import.ty else {
                            panic!("data profile imported non-function authority")
                        };
                        imports.push((import.module.to_owned(), import.name.to_owned()));
                    }
                }
                Payload::ExportSection(section) => {
                    for export in section {
                        let export = export.unwrap();
                        exports.push((export.name.to_owned(), export.kind));
                    }
                }
                _ => {}
            }
        }
        assert_eq!(
            imports,
            [
                "spx_add",
                "spx_sub",
                "spx_mul",
                "spx_div",
                "spx_rem",
                "spx_neg",
                "spx_contract_fail",
                "spx_bytes_copy",
                "spx_bytes_get",
                "spx_bytes_drop",
                "spx_bytes_as_slice",
            ]
            .map(|name| ("env".to_owned(), name.to_owned()))
        );
        assert_eq!(
            exports,
            vec![
                ("memory".to_owned(), ExternalKind::Memory),
                ("__spx_data_status_v1".to_owned(), ExternalKind::Global),
                (
                    "__spx_data_scratch_base_v1".to_owned(),
                    ExternalKind::Global
                ),
                (
                    "__spx_data_scratch_capacity_v1".to_owned(),
                    ExternalKind::Global
                ),
                (raw_symbol("data.copy"), ExternalKind::Func),
                (raw_symbol("data.fail"), ExternalKind::Func),
                (raw_symbol("data.length"), ExternalKind::Func),
                (raw_symbol("data.present"), ExternalKind::Func),
                (raw_symbol("data.total"), ExternalKind::Func),
            ]
        );

        if Command::new("node").arg("--version").output().is_err() {
            return;
        }
        let root = std::env::temp_dir().join(format!(
            "semaprax-data-exports-{}-{}",
            std::process::id(),
            first.len()
        ));
        std::fs::create_dir(&root).unwrap();
        let wasm_path = root.join("app.wasm");
        let script_path = root.join("test.mjs");
        std::fs::write(&wasm_path, &first).unwrap();
        let script = format!(
            r#"import {{ readFile }} from "node:fs/promises";
const entries = new Map(); let next = 1; let instance;
const decode = carrier => {{ const word=BigInt.asUintN(64,carrier), length=Number(word&0xffffffffn), root=Number((word>>32n)&0xffffffffn); if(length>65536)throw Error("length"); return {{word,length,root,tagged:(root&0x80000000)!==0,token:root&0x7fffffff}}; }};
const read = decoded => {{ if(decoded.tagged){{const value=entries.get(decoded.token);if(!(value instanceof Uint8Array)||value.length!==decoded.length)throw Error("stale");return value;}} const memory=new Uint8Array(instance.exports.memory.buffer);if(decoded.root>memory.length-decoded.length)throw Error("range");return memory.slice(decoded.root,decoded.root+decoded.length); }};
const allocate = bytes => {{const token=next++, owned=new Uint8Array(bytes);entries.set(token,owned);return BigInt.asIntN(64,((0x80000000n|BigInt(token))<<32n)|BigInt(owned.length));}};
const imports={{env:{{
spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{{throw Error("contract");}},
spx_bytes_copy:c=>allocate(read(decode(c))),spx_bytes_get:(c,i)=>{{const b=read(decode(c)),u=BigInt.asUintN(64,i);return u>=BigInt(b.length)?-1:b[Number(u)];}},spx_bytes_drop:c=>{{const d=decode(c);read(d);entries.delete(d.token);}},spx_bytes_as_slice:c=>{{const d=decode(c);read(d);return BigInt.asIntN(64,d.word);}}
}}}};
({{instance}}=await WebAssembly.instantiate(await readFile(process.argv[2]),imports));
const e=instance.exports, memory=new Uint8Array(e.memory.buffer);
if(memory.length!==131072)throw Error("memory"); let fixed=false;try{{e.memory.grow(1)}}catch{{fixed=true}}if(!fixed)throw Error("grow");
if(e.__spx_data_scratch_base_v1.value!==0||e.__spx_data_scratch_capacity_v1.value!==65536)throw Error("metadata");
memory.set([255,0,7],0);
if(e["{length}"](0,3)!==3n||e.__spx_data_status_v1.value!==0)throw Error("length");
if(e["{present}"](0,3)!==1||e.__spx_data_status_v1.value!==0)throw Error("bool");
if(e["{copy}"](0,3)!==1n||entries.size!==0||e.__spx_data_status_v1.value!==0)throw Error("copy-cleanup");
if(e["{fail}"](0,3)!==0n||entries.size!==0||e.__spx_data_status_v1.value!==1)throw Error("failure-cleanup");
if(e["{length}"](65536,0)!==0n||e.__spx_data_status_v1.value!==0)throw Error("empty-boundary");
if(e["{length}"](0,65536)!==65536n||e.__spx_data_status_v1.value!==0)throw Error("exact-root-capacity");
if(e["{length}"](0,65537)!==0n||e.__spx_data_status_v1.value!==11)throw Error("root-capacity-plus-one");
if(e["{length}"](65536,1)!==0n||e.__spx_data_status_v1.value!==11)throw Error("range-status");
if(e["{length}"](-1,0)!==0n||e.__spx_data_status_v1.value!==11)throw Error("unsigned-offset");
if(e["{total}"](0,40000,0,30000)!==0n||e.__spx_data_status_v1.value!==11)throw Error("cumulative");
if(e["{total}"](0,32768,32768,32768)!==65536n||e.__spx_data_status_v1.value!==0)throw Error("exact-capacity");
console.log("public-data-core-wasm-ok");
"#,
            length = raw_symbol("data.length"),
            present = raw_symbol("data.present"),
            copy = raw_symbol("data.copy"),
            fail = raw_symbol("data.fail"),
            total = raw_symbol("data.total"),
        );
        std::fs::write(&script_path, script).unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        let _ = std::fs::remove_dir(&root);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"public-data-core-wasm-ok\n");
    }
}
