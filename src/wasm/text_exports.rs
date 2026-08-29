//! Admission and raw-wrapper planning for Public Borrowed Text Export Profile v1.
//!
//! The profile is additive: it never changes the legacy web or Public Scalar
//! Export v1 module. Borrowed UTF-8 is represented internally as one i64 with
//! pointer in the low 32 bits and byte length in the high 32 bits. Raw public
//! wrappers alone expand a `str` parameter into `(i32, i32)`.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, FunctionExecutionId, IdentityOrigin, OwnershipMode, ResolvedExprKind,
    ResolvedFunction, ResolvedProgram, ResolvedStatement, ResolvedType,
};

use super::{write_i64, write_u32, ByteOutput, I32, I64};

pub(super) const STATUS_GLOBAL_EXPORT: &str = "__spx_text_status_v1";
pub(super) const SCRATCH_BASE_EXPORT: &str = "__spx_text_scratch_base_v1";
pub(super) const SCRATCH_CAPACITY_EXPORT: &str = "__spx_text_scratch_capacity_v1";
pub(super) const MEMORY_EXPORT: &str = "memory";
pub(super) const SCRATCH_BASE: u32 = 0;
pub(super) const SCRATCH_CAPACITY: u32 = 65_536;
pub(super) const KMP_TABLE_BASE: u32 = SCRATCH_CAPACITY;
pub(super) const FIXED_MEMORY_PAGES: u8 = 3;

const MAX_EXPORTS: usize = 32;
const MAX_FUNCTIONS: usize = 256;
const MAX_PARAMETERS: usize = 8;
const MAX_STABLE_ID_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TextAbiType {
    Str,
    I64,
    Bool,
}

impl TextAbiType {
    pub(super) fn internal_wasm_type(self) -> u8 {
        match self {
            Self::Str | Self::I64 => I64,
            Self::Bool => I32,
        }
    }

    pub(super) fn raw_wasm_types(self) -> &'static [u8] {
        match self {
            Self::Str => &[I32, I32],
            Self::I64 => &[I64],
            Self::Bool => &[I32],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TextExportPlan {
    pub(super) stable_id: String,
    pub(super) wasm_export: String,
    pub(super) function_id: DeclarationId,
    pub(super) params: Vec<TextAbiType>,
    pub(super) result: TextAbiType,
}

impl TextExportPlan {
    pub(super) fn raw_params(&self) -> Vec<u8> {
        self.params
            .iter()
            .flat_map(|ty| ty.raw_wasm_types().iter().copied())
            .collect()
    }

    pub(super) fn emit_wrapper_body(
        &self,
        body: &mut impl ByteOutput,
        function_indexes: &HashMap<FunctionExecutionId, u32>,
        validator_index: u32,
        status_global_index: u32,
    ) -> Result<(), Diagnostic> {
        // One i32 local carries validator status and one carries the exact
        // cumulative borrowed-input byte charge. Raw parameters precede both.
        let raw_parameter_count = self.raw_params().len() as u32;
        write_u32(body, 1);
        write_u32(body, 2);
        body.push(I32);
        let status_local = raw_parameter_count;
        let cumulative_local = raw_parameter_count + 1;

        i32_const(body, 0);
        global_set(body, status_global_index);
        i32_const(body, 0);
        local_set(body, cumulative_local);

        let mut raw_index = 0_u32;
        for parameter in &self.params {
            match parameter {
                TextAbiType::Str => {
                    local_get(body, raw_index);
                    local_get(body, raw_index + 1);
                    call(body, validator_index);
                    body.push(0x22); // local.tee validation status
                    write_u32(body, status_local);
                    body.push(0x04); // if
                    body.push(0x40); // empty block type
                    local_get(body, status_local);
                    global_set(body, status_global_index);
                    emit_zero(body, self.result);
                    body.push(0x0f); // return: selected target was not entered
                    body.push(0x0b); // end if

                    local_get(body, cumulative_local);
                    local_get(body, raw_index + 1);
                    body.push(0x6a); // add: both operands were individually capped
                    body.push(0x22); // local.tee cumulative
                    write_u32(body, cumulative_local);
                    i32_const(body, SCRATCH_CAPACITY as i32);
                    body.push(0x4b); // cumulative > exact profile budget
                    body.extend_bytes(&[0x04, 0x40]);
                    i32_const(body, 1);
                    global_set(body, status_global_index);
                    emit_zero(body, self.result);
                    body.push(0x0f);
                    body.push(0x0b);
                    raw_index += 2;
                }
                TextAbiType::Bool => {
                    emit_bool_trap(body, raw_index);
                    raw_index += 1;
                }
                TextAbiType::I64 => raw_index += 1,
            }
        }

        raw_index = 0;
        for parameter in &self.params {
            match parameter {
                TextAbiType::Str => {
                    // Zero-extend both halves before combining them. This is
                    // independent of the host's signed i32 presentation.
                    local_get(body, raw_index);
                    body.push(0xad); // i64.extend_i32_u
                    local_get(body, raw_index + 1);
                    body.push(0xad); // i64.extend_i32_u
                    i64_const(body, 32);
                    body.push(0x86); // i64.shl
                    body.push(0x84); // i64.or
                    raw_index += 2;
                }
                TextAbiType::Bool | TextAbiType::I64 => {
                    local_get(body, raw_index);
                    raw_index += 1;
                }
            }
        }
        let execution = FunctionExecutionId::Monomorphic(self.function_id.clone());
        let target = function_indexes.get(&execution).copied().ok_or_else(|| {
            admission(format!(
                "selected text export `{}` has no monomorphic Wasm target",
                self.stable_id
            ))
        })?;
        call(body, target);
        if self.result == TextAbiType::Bool {
            // A malformed internal boolean is a compiler invariant failure,
            // never a text-input status.
            let result_local = cumulative_local;
            // Add a second i32 local lazily is impossible after declarations;
            // validate on-stack with a duplicate-free canonicalization trap by
            // storing in the already-declared status local after the call.
            body.push(0x21); // local.set
            write_u32(body, status_local);
            emit_bool_trap(body, status_local);
            local_get(body, status_local);
            let _ = result_local;
        }
        body.push(0x0b);
        Ok(())
    }
}

pub(super) fn prepare(
    program: &ResolvedProgram,
    export_ids: &[String],
) -> Result<Vec<TextExportPlan>, Diagnostic> {
    validate_selection(export_ids)?;
    hir::validate(program)?;
    if !program.permits.is_empty() || !program.interfaces.is_empty() {
        return Err(admission(
            "Public Borrowed Text Export Profile v1 does not admit permits or interfaces",
        ));
    }
    if !program.function_templates.is_empty() || !program.function_instances.is_empty() {
        return Err(admission(
            "Public Borrowed Text Export Profile v1 does not admit generic functions",
        ));
    }
    if program.functions.len() > MAX_FUNCTIONS {
        return Err(capacity(format!(
            "Public Borrowed Text Export Profile v1 admits at most {MAX_FUNCTIONS} functions"
        )));
    }
    if program.types.iter().any(|declaration| {
        program
            .declarations
            .declaration(&declaration.id)
            .is_none_or(|item| item.identity_origin != IdentityOrigin::CompilerOwned)
    }) {
        return Err(admission(
            "Public Borrowed Text Export Profile v1 does not admit authored resources or aggregates",
        ));
    }

    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let mut sorted = export_ids.to_vec();
    sorted.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    let mut symbols = BTreeSet::new();
    let mut plans = Vec::with_capacity(sorted.len());
    let mut closure = BTreeSet::<DeclarationId>::new();
    let mut frontier = Vec::<DeclarationId>::new();

    for stable_id in sorted {
        let function = functions.get(stable_id.as_str()).copied().ok_or_else(|| {
            admission(format!(
                "selected text export identity `{stable_id}` does not name a monomorphic function"
            ))
        })?;
        require_explicit(program, function)?;
        let params = function
            .params
            .iter()
            .map(|parameter| abi_parameter(function, parameter))
            .collect::<Result<Vec<_>, _>>()?;
        if params.len() > MAX_PARAMETERS {
            return Err(capacity(format!(
                "Public Borrowed Text Export Profile v1 function `{}` exceeds the {MAX_PARAMETERS}-parameter limit",
                function.id
            )));
        }
        if !params.contains(&TextAbiType::Str) {
            return Err(admission(format!(
                "selected text export identity `{stable_id}` has no borrowed `str` parameter"
            )));
        }
        let result = abi_result(&function.return_type).ok_or_else(|| {
            admission(format!(
                "selected text export identity `{stable_id}` has a non-scalar result"
            ))
        })?;
        let wasm_export = raw_symbol(&stable_id);
        if !symbols.insert(wasm_export.clone()) {
            return Err(admission(format!(
                "selected text export identity `{stable_id}` collides with another raw symbol"
            )));
        }
        frontier.push(function.id.clone());
        plans.push(TextExportPlan {
            stable_id,
            wasm_export,
            function_id: function.id.clone(),
            params,
            result,
        });
    }

    while let Some(id) = frontier.pop() {
        if !closure.insert(id.clone()) {
            continue;
        }
        let function = functions
            .get(id.as_str())
            .copied()
            .ok_or_else(|| admission(format!("text-profile closure target `{id}` is absent")))?;
        require_explicit(program, function)?;
        validate_function(function, &functions, &mut frontier)?;
    }
    // The shared core emitter materializes every monomorphic function, not
    // merely the selected closure. Validate that exact compiled inventory as
    // well so an unreachable owned-string helper cannot silently reintroduce
    // host string imports or an unbounded loop into the text-profile module.
    let mut call_graph = BTreeMap::new();
    for function in &program.functions {
        let mut callees = Vec::new();
        validate_function(function, &functions, &mut callees)?;
        callees.sort();
        callees.dedup();
        call_graph.insert(function.id.clone(), callees);
    }
    reject_call_cycles(&call_graph)?;
    Ok(plans)
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
                "Public Borrowed Text Export Profile v1 reaches a recursive call cycle at `{id}`"
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

fn validate_function(
    function: &ResolvedFunction,
    functions: &BTreeMap<&str, &ResolvedFunction>,
    frontier: &mut Vec<DeclarationId>,
) -> Result<(), Diagnostic> {
    if !function.effects.is_empty() || !function.requires.is_empty() || !function.ensures.is_empty()
    {
        return Err(admission(format!(
            "Public Borrowed Text Export Profile v1 function `{}` must be effect- and contract-free",
            function.id
        )));
    }
    for parameter in &function.params {
        let _ = abi_parameter(function, parameter)?;
    }
    if abi_result(&function.return_type).is_none() {
        return Err(admission(format!(
            "Public Borrowed Text Export Profile v1 function `{}` has a non-scalar result",
            function.id
        )));
    }
    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ResolvedExprKind::String(_)
            | ResolvedExprKind::NativeRustImportCall(_)
            | ResolvedExprKind::HostCommandCall(_) => {
                return Err(admission(format!(
                    "Public Borrowed Text Export Profile v1 function `{}` reaches an owned string, import, or command I/O",
                    function.id
                )));
            }
            ResolvedExprKind::Call {
                callee,
                instance,
                type_arguments,
                args,
            } => {
                if instance.is_some() || !type_arguments.is_empty() {
                    return Err(admission(format!(
                        "Public Borrowed Text Export Profile v1 function `{}` reaches a generic call",
                        function.id
                    )));
                }
                pending.extend(args);
                if crate::str_ops::by_id(callee.as_str()).is_none() {
                    if !functions.contains_key(callee.as_str()) {
                        return Err(admission(format!(
                            "Public Borrowed Text Export Profile v1 function `{}` reaches an unavailable call `{callee}`",
                            function.id
                        )));
                    }
                    frontier.push(callee.clone());
                }
            }
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    if matches!(statement, ResolvedStatement::While { .. }) {
                        return Err(admission(format!(
                            "Public Borrowed Text Export Profile v1 function `{}` reaches a loop",
                            function.id
                        )));
                    }
                    pending.push(statement.value());
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
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::Place(_) => {}
            ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::ByteRange { .. } => {
                return Err(admission(format!(
                    "Public Borrowed Text Export Profile v1 function `{}` reaches portable byte data",
                    function.id
                )));
            }
            ResolvedExprKind::ConstructRecord { .. }
            | ResolvedExprKind::ConstructVariant { .. }
            | ResolvedExprKind::Match { .. }
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::UpdateRecord { .. }
            | ResolvedExprKind::Project { .. }
            | ResolvedExprKind::Upcast { .. } => {
                return Err(admission(format!(
                    "Public Borrowed Text Export Profile v1 function `{}` reaches an aggregate or variant expression",
                    function.id
                )));
            }
        }
    }
    Ok(())
}

fn abi_parameter(
    function: &ResolvedFunction,
    parameter: &crate::hir::ResolvedParam,
) -> Result<TextAbiType, Diagnostic> {
    match (&parameter.ty, parameter.ownership) {
        (ResolvedType::Str, OwnershipMode::Borrow) => Ok(TextAbiType::Str),
        (ResolvedType::I64, OwnershipMode::Value) => Ok(TextAbiType::I64),
        (ResolvedType::Bool, OwnershipMode::Value) => Ok(TextAbiType::Bool),
        _ => Err(admission(format!(
            "Public Borrowed Text Export Profile v1 function `{}` has an unsupported parameter",
            function.id
        ))),
    }
}

fn abi_result(ty: &ResolvedType) -> Option<TextAbiType> {
    match ty {
        ResolvedType::I64 => Some(TextAbiType::I64),
        ResolvedType::Bool => Some(TextAbiType::Bool),
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
            "Public Borrowed Text Export Profile v1 function `{}` must have an explicit stable identity",
            function.id
        )));
    }
    Ok(())
}

fn validate_selection(ids: &[String]) -> Result<(), Diagnostic> {
    if !(1..=MAX_EXPORTS).contains(&ids.len()) {
        return Err(capacity(format!(
            "Public Borrowed Text Export Profile v1 requires 1..={MAX_EXPORTS} selected IDs"
        )));
    }
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.as_str()) {
            return Err(admission(format!(
                "selected text export ID `{id}` appears more than once"
            )));
        }
        if !(1..=MAX_STABLE_ID_BYTES).contains(&id.len()) {
            return Err(capacity(format!(
                "text export IDs must contain 1..={MAX_STABLE_ID_BYTES} bytes"
            )));
        }
        if !id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        }) {
            return Err(admission(format!(
                "text export ID `{id}` must use lowercase [a-z0-9._-]"
            )));
        }
    }
    Ok(())
}

pub(super) fn raw_symbol(stable_id: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut symbol = String::with_capacity(9 + stable_id.len() * 2);
    symbol.push_str("spx_text_");
    for byte in stable_id.bytes() {
        symbol.push(HEX[(byte >> 4) as usize] as char);
        symbol.push(HEX[(byte & 0x0f) as usize] as char);
    }
    symbol
}

/// `(i32 pointer, i32 length) -> i32 status`, where zero is admitted, one is
/// range/OOB, and two is malformed UTF-8. Every load is dominated by the
/// exact end-range check and by a remaining-byte check for its sequence.
pub(super) fn emit_utf8_validator_body(body: &mut impl ByteOutput) {
    // locals: end, cursor, lead, continuation
    write_u32(body, 1);
    write_u32(body, 4);
    body.push(I32);

    local_get(body, 0);
    local_get(body, 1);
    body.push(0x6a); // i32.add
    body.push(0x22); // local.tee end
    write_u32(body, 2);
    local_get(body, 0);
    body.push(0x49); // i32.lt_u: wrapped end < ptr
    if_return_i32(body, 1);

    i32_const(body, SCRATCH_CAPACITY as i32);
    local_get(body, 2);
    body.push(0x49); // scratch capacity < end
    if_return_i32(body, 1);

    local_get(body, 0);
    body.push(0x21); // local.set cursor
    write_u32(body, 3);
    body.extend_bytes(&[0x02, 0x40, 0x03, 0x40]); // block; loop
    local_get(body, 3);
    local_get(body, 2);
    body.push(0x4f); // cursor >= end
    body.extend_bytes(&[0x0d, 0x01]); // br_if exit

    load8(body, 3);
    body.push(0x22); // local.tee lead
    write_u32(body, 4);
    i32_const(body, 0x80);
    body.push(0x49); // lead < 0x80
    body.extend_bytes(&[0x04, 0x40]);
    advance_and_continue(body, 3, 1);
    body.push(0x0b);

    // Two-byte sequence: C2..DF 80..BF.
    local_get(body, 4);
    i32_const(body, 0xc2);
    body.push(0x4f); // >=
    local_get(body, 4);
    i32_const(body, 0xdf);
    body.push(0x4d); // <=
    body.push(0x71); // and
    body.extend_bytes(&[0x04, 0x40]);
    require_remaining(body, 3, 2, 2);
    load8_offset(body, 3, 1);
    require_continuation_or_return(body, 5, 2);
    advance_and_continue(body, 3, 2);
    body.push(0x0b);

    // Three-byte lead E0..EF. The second byte has the canonical overlong and
    // surrogate exclusions; the third is an ordinary continuation.
    local_get(body, 4);
    i32_const(body, 0xe0);
    body.push(0x4f);
    local_get(body, 4);
    i32_const(body, 0xef);
    body.push(0x4d);
    body.push(0x71);
    body.extend_bytes(&[0x04, 0x40]);
    require_remaining(body, 3, 3, 2);
    load8_offset(body, 3, 1);
    body.push(0x22);
    write_u32(body, 5);
    require_continuation_or_return(body, 5, 2);
    local_get(body, 4);
    i32_const(body, 0xe0);
    body.push(0x46); // lead == E0
    body.extend_bytes(&[0x04, 0x40]);
    local_get(body, 5);
    i32_const(body, 0xa0);
    body.push(0x49); // second < A0 => overlong
    if_return_i32(body, 2);
    body.push(0x0b);
    local_get(body, 4);
    i32_const(body, 0xed);
    body.push(0x46); // lead == ED
    body.extend_bytes(&[0x04, 0x40]);
    local_get(body, 5);
    i32_const(body, 0xa0);
    body.push(0x4f); // second >= A0 => surrogate
    if_return_i32(body, 2);
    body.push(0x0b);
    load8_offset(body, 3, 2);
    require_continuation_or_return(body, 5, 2);
    advance_and_continue(body, 3, 3);
    body.push(0x0b);

    // Four-byte lead F0..F4. Reject overlong F0 and > U+10FFFF F4.
    local_get(body, 4);
    i32_const(body, 0xf0);
    body.push(0x4f);
    local_get(body, 4);
    i32_const(body, 0xf4);
    body.push(0x4d);
    body.push(0x71);
    body.extend_bytes(&[0x04, 0x40]);
    require_remaining(body, 3, 4, 2);
    load8_offset(body, 3, 1);
    body.push(0x22);
    write_u32(body, 5);
    require_continuation_or_return(body, 5, 2);
    local_get(body, 4);
    i32_const(body, 0xf0);
    body.push(0x46);
    body.extend_bytes(&[0x04, 0x40]);
    local_get(body, 5);
    i32_const(body, 0x90);
    body.push(0x49);
    if_return_i32(body, 2);
    body.push(0x0b);
    local_get(body, 4);
    i32_const(body, 0xf4);
    body.push(0x46);
    body.extend_bytes(&[0x04, 0x40]);
    local_get(body, 5);
    i32_const(body, 0x90);
    body.push(0x4f);
    if_return_i32(body, 2);
    body.push(0x0b);
    load8_offset(body, 3, 2);
    require_continuation_or_return(body, 5, 2);
    load8_offset(body, 3, 3);
    require_continuation_or_return(body, 5, 2);
    advance_and_continue(body, 3, 4);
    body.push(0x0b);

    i32_const(body, 2); // all other lead bytes are invalid
    body.push(0x0f);
    body.push(0x0b); // end loop
    body.push(0x0b); // end block
    i32_const(body, 0);
    body.push(0x0b);
}

/// Internal packed-view starts-with helper: `(i64 value, i64 prefix) -> i32`.
pub(super) fn emit_starts_with_body(body: &mut impl ByteOutput) {
    // value_ptr, value_len, prefix_ptr, prefix_len, i
    write_u32(body, 1);
    write_u32(body, 5);
    body.push(I32);
    unpack_view(body, 0, 2, 3);
    unpack_view(body, 1, 4, 5);
    local_get(body, 5);
    local_get(body, 3);
    body.push(0x4b); // prefix_len > value_len
    if_return_i32(body, 0);
    i32_const(body, 0);
    body.push(0x21);
    write_u32(body, 6);
    body.extend_bytes(&[0x02, 0x40, 0x03, 0x40]);
    local_get(body, 6);
    local_get(body, 5);
    body.push(0x4f);
    body.extend_bytes(&[0x0d, 0x01]);
    load8_indexed(body, 2, 6);
    load8_indexed(body, 4, 6);
    body.push(0x47); // ne
    if_return_i32(body, 0);
    increment_local(body, 6);
    body.extend_bytes(&[0x0c, 0x00, 0x0b, 0x0b]);
    i32_const(body, 1);
    body.push(0x0b);
}

/// Internal packed-view contains helper: `(i64 value, i64 needle) -> i32`.
pub(super) fn emit_contains_body(body: &mut impl ByteOutput) {
    emit_contains_body_at(body, KMP_TABLE_BASE);
}

pub(super) fn emit_contains_body_at(body: &mut impl ByteOutput, kmp_table_base: u32) {
    // value_ptr, value_len, needle_ptr, needle_len, matched, index, table_value
    write_u32(body, 1);
    write_u32(body, 7);
    body.push(I32);
    unpack_view(body, 0, 2, 3);
    unpack_view(body, 1, 4, 5);
    local_get(body, 5);
    body.push(0x45); // empty needle
    if_return_i32(body, 1);
    local_get(body, 5);
    local_get(body, 3);
    body.push(0x4b);
    if_return_i32(body, 0);
    // Build the fixed u16 KMP prefix table in the reserved work region. The
    // module exports its memory, so reset every prefix cell read before it can
    // influence control flow, including the index-zero sentinel.
    i32_const(body, 0);
    local_set(body, 6); // matched
    i32_const(body, 0);
    local_set(body, 7); // index-zero sentinel
    store_kmp_prefix(body, 7, 6, kmp_table_base);
    i32_const(body, 1);
    local_set(body, 7); // index
    body.extend_bytes(&[0x02, 0x40, 0x03, 0x40]);
    local_get(body, 7);
    local_get(body, 5);
    body.push(0x4f);
    body.extend_bytes(&[0x0d, 0x01]);
    emit_kmp_fallback(body, 4, 7, 6, kmp_table_base);
    load8_indexed(body, 4, 6);
    load8_indexed(body, 4, 7);
    body.push(0x46);
    body.extend_bytes(&[0x04, 0x40]);
    increment_local(body, 6);
    body.push(0x0b);
    store_kmp_prefix(body, 7, 6, kmp_table_base);
    increment_local(body, 7);
    body.extend_bytes(&[0x0c, 0x00, 0x0b, 0x0b]);

    // Search: each mismatch strictly shortens `matched`; total work is
    // linear in value_len + needle_len.
    i32_const(body, 0);
    local_set(body, 6);
    i32_const(body, 0);
    local_set(body, 7);
    body.extend_bytes(&[0x02, 0x40, 0x03, 0x40]);
    local_get(body, 7);
    local_get(body, 3);
    body.push(0x4f);
    body.extend_bytes(&[0x0d, 0x01]);
    emit_kmp_fallback(body, 2, 7, 6, kmp_table_base);
    load8_indexed(body, 4, 6);
    load8_indexed(body, 2, 7);
    body.push(0x46);
    body.extend_bytes(&[0x04, 0x40]);
    increment_local(body, 6);
    local_get(body, 6);
    local_get(body, 5);
    body.push(0x46);
    if_return_i32(body, 1);
    body.push(0x0b);
    increment_local(body, 7);
    body.extend_bytes(&[0x0c, 0x00, 0x0b, 0x0b]);
    i32_const(body, 0);
    body.push(0x0b);
}

/// Internal packed-view contains helper for mixed aggregate/data modules.
/// Those modules reserve their fixed memory for byte arenas and owned UTF-8
/// literals, so this bounded scan uses locals only and preserves that ABI.
pub(super) fn emit_contains_bounded_scan_body(body: &mut impl ByteOutput) {
    // value_ptr, value_len, needle_ptr, needle_len, start, offset
    write_u32(body, 1);
    write_u32(body, 6);
    body.push(I32);
    unpack_view(body, 0, 2, 3);
    unpack_view(body, 1, 4, 5);
    local_get(body, 5);
    body.push(0x45); // empty needle
    if_return_i32(body, 1);
    local_get(body, 5);
    local_get(body, 3);
    body.push(0x4b); // needle_len > value_len
    if_return_i32(body, 0);
    i32_const(body, 0);
    local_set(body, 6);
    body.extend_bytes(&[0x02, 0x40, 0x03, 0x40]); // outer block + loop
    local_get(body, 6);
    local_get(body, 5);
    body.push(0x6a); // start + needle_len
    local_get(body, 3);
    body.push(0x4b); // beyond value_len
    body.extend_bytes(&[0x0d, 0x01]); // break outer block
    i32_const(body, 0);
    local_set(body, 7);
    body.extend_bytes(&[0x02, 0x40, 0x03, 0x40]); // inner block + loop
    local_get(body, 7);
    local_get(body, 5);
    body.push(0x4f); // offset >= needle_len
    if_return_i32(body, 1);
    local_get(body, 2);
    local_get(body, 6);
    body.push(0x6a);
    local_get(body, 7);
    body.push(0x6a);
    body.extend_bytes(&[0x2d, 0x00, 0x00]); // i32.load8_u
    load8_indexed(body, 4, 7);
    body.push(0x47); // ne
    body.extend_bytes(&[0x0d, 0x01]); // break inner block
    increment_local(body, 7);
    body.extend_bytes(&[0x0c, 0x00, 0x0b, 0x0b]);
    increment_local(body, 6);
    body.extend_bytes(&[0x0c, 0x00, 0x0b, 0x0b]);
    i32_const(body, 0);
    body.push(0x0b);
}

fn emit_kmp_fallback(
    body: &mut impl ByteOutput,
    haystack_ptr: u32,
    index: u32,
    matched: u32,
    kmp_table_base: u32,
) {
    body.extend_bytes(&[0x02, 0x40, 0x03, 0x40]);
    local_get(body, matched);
    body.push(0x45);
    body.extend_bytes(&[0x0d, 0x01]);
    load8_indexed(body, 4, matched);
    load8_indexed(body, haystack_ptr, index);
    body.push(0x46);
    body.extend_bytes(&[0x0d, 0x01]);
    load_kmp_prefix_before(body, matched, kmp_table_base);
    local_set(body, matched);
    body.extend_bytes(&[0x0c, 0x00, 0x0b, 0x0b]);
}

fn store_kmp_prefix(body: &mut impl ByteOutput, index: u32, value: u32, kmp_table_base: u32) {
    i32_const(body, kmp_table_base as i32);
    local_get(body, index);
    i32_const(body, 1);
    body.push(0x74); // shl => u16 byte offset
    body.push(0x6a);
    local_get(body, value);
    body.extend_bytes(&[0x3b, 0x01, 0x00]); // i32.store16 align=2 offset=0
}

fn load_kmp_prefix_before(body: &mut impl ByteOutput, matched: u32, kmp_table_base: u32) {
    i32_const(body, kmp_table_base as i32 - 2);
    local_get(body, matched);
    i32_const(body, 1);
    body.push(0x74);
    body.push(0x6a);
    body.extend_bytes(&[0x2f, 0x01, 0x00]); // i32.load16_u align=2 offset=0
}

fn unpack_view(body: &mut impl ByteOutput, parameter: u32, pointer: u32, length: u32) {
    local_get(body, parameter);
    body.push(0xa7); // i32.wrap_i64
    body.push(0x21);
    write_u32(body, pointer);
    local_get(body, parameter);
    i64_const(body, 32);
    body.push(0x88); // i64.shr_u
    body.push(0xa7);
    body.push(0x21);
    write_u32(body, length);
}

fn require_remaining(body: &mut impl ByteOutput, cursor: u32, needed: i32, status: i32) {
    local_get(body, cursor);
    i32_const(body, needed);
    body.push(0x6a);
    local_get(body, 2); // validator end local
    body.push(0x4b); // cursor + needed > end
    if_return_i32(body, status);
}

fn require_continuation_or_return(body: &mut impl ByteOutput, local: u32, status: i32) {
    body.push(0x21);
    write_u32(body, local);
    local_get(body, local);
    i32_const(body, 0xc0);
    body.push(0x71); // byte & C0
    i32_const(body, 0x80);
    body.push(0x47); // != 80
    if_return_i32(body, status);
}

fn if_return_i32(body: &mut impl ByteOutput, value: i32) {
    body.extend_bytes(&[0x04, 0x40]);
    i32_const(body, value);
    body.push(0x0f);
    body.push(0x0b);
}

fn load8(body: &mut impl ByteOutput, pointer_local: u32) {
    local_get(body, pointer_local);
    body.extend_bytes(&[0x2d, 0x00, 0x00]);
}

fn load8_offset(body: &mut impl ByteOutput, pointer_local: u32, offset: i32) {
    local_get(body, pointer_local);
    i32_const(body, offset);
    body.push(0x6a);
    body.extend_bytes(&[0x2d, 0x00, 0x00]);
}

fn load8_indexed(body: &mut impl ByteOutput, pointer: u32, index: u32) {
    local_get(body, pointer);
    local_get(body, index);
    body.push(0x6a);
    body.extend_bytes(&[0x2d, 0x00, 0x00]);
}

fn advance_and_continue(body: &mut impl ByteOutput, cursor: u32, amount: i32) {
    local_get(body, cursor);
    i32_const(body, amount);
    body.push(0x6a);
    body.push(0x21);
    write_u32(body, cursor);
    // Every use is inside one sequence-classification `if`, nested directly
    // in the validator loop. Depth one is therefore the loop's back-edge;
    // depth zero would merely leave the `if` and incorrectly continue testing
    // the already-consumed lead byte against later sequence classes.
    body.extend_bytes(&[0x0c, 0x01]);
}

fn increment_local(body: &mut impl ByteOutput, local: u32) {
    local_get(body, local);
    i32_const(body, 1);
    body.push(0x6a);
    body.push(0x21);
    write_u32(body, local);
}

fn emit_zero(body: &mut impl ByteOutput, ty: TextAbiType) {
    match ty {
        TextAbiType::I64 | TextAbiType::Str => i64_const(body, 0),
        TextAbiType::Bool => i32_const(body, 0),
    }
}

fn emit_bool_trap(body: &mut impl ByteOutput, local: u32) {
    local_get(body, local);
    i32_const(body, 1);
    body.push(0x4b); // i32.gt_u
    body.extend_bytes(&[0x04, 0x40, 0x00, 0x0b]);
}

fn local_get(body: &mut impl ByteOutput, index: u32) {
    body.push(0x20);
    write_u32(body, index);
}

fn local_set(body: &mut impl ByteOutput, index: u32) {
    body.push(0x21);
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
    super::write_i32(body, value);
}

fn i64_const(body: &mut impl ByteOutput, value: i64) {
    body.push(0x42);
    write_i64(body, value);
}

fn admission(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W119", message)
}

fn capacity(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W120", message)
}
