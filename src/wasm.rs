use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use same_file::Handle;
use sha2::{Digest, Sha256};

use crate::ast::{BinaryOp, Program, UnaryOp};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::graph;
use crate::hir::{
    self, FunctionExecutionId, IdentityOrigin, ResolvedExpr, ResolvedExprKind, ResolvedProgram,
    ResolvedStatement, ResolvedType, ResolvedTypeDeclarationKind, ValueId,
};
use crate::variant_layout::{VariantLayoutCache, VariantTarget};

mod aggregate;
mod command_io;
mod data_exports;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod generic_function_component_v9;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod generic_record_component_v7;
mod host_output;
mod line_command_io;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod nested_record_component_v6;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod option_propagation_component_v10;
mod owned;
mod owned_data_exports;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod record_pattern_component_v8;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod result_component_v3;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod scalar_algebra_component_v5;
mod scalar_exports;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod source_result_component_v4;
mod text_exports;

#[cfg(any(test, feature = "unstable-wit-component-harness"))]
pub(crate) use generic_function_component_v9::{
    emit_private_generic_function_core_v9,
    CANONICAL_EXPORTS as GENERIC_FUNCTION_COMPONENT_CANONICAL_EXPORTS_V9,
    SOURCE_V9 as GENERIC_FUNCTION_COMPONENT_SOURCE_V9,
};
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
pub(crate) use generic_record_component_v7::{
    emit_private_generic_record_core_v7,
    CANONICAL_EXPORTS as GENERIC_RECORD_COMPONENT_CANONICAL_EXPORTS_V7,
    SOURCE_V7 as GENERIC_RECORD_COMPONENT_SOURCE_V7,
};
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
pub(crate) use nested_record_component_v6::{
    emit_private_nested_record_core_v6,
    CANONICAL_EXPORT as NESTED_RECORD_COMPONENT_CANONICAL_EXPORT_V6,
    SOURCE_V6 as NESTED_RECORD_COMPONENT_SOURCE_V6,
};
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
pub(crate) use option_propagation_component_v10::{
    emit_private_option_propagation_core_v10,
    CANONICAL_EXPORT as OPTION_PROPAGATION_COMPONENT_CANONICAL_EXPORT_V10,
    SOURCE_V10 as OPTION_PROPAGATION_SOURCE_V10,
    STATUS_OUT_EXPORT as OPTION_PROPAGATION_COMPONENT_STATUS_OUT_EXPORT_V10,
};
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
pub(crate) use record_pattern_component_v8::{
    emit_private_record_pattern_core_v8,
    CANONICAL_EXPORTS as RECORD_PATTERN_COMPONENT_CANONICAL_EXPORTS_V8,
    SOURCE_V8 as RECORD_PATTERN_COMPONENT_SOURCE_V8,
};
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
pub(crate) use result_component_v3::{
    emit_private_result_core_v3, CANONICAL_EXPORT as RESULT_COMPONENT_CANONICAL_EXPORT_V3,
    STATUS_OUT_EXPORT as RESULT_COMPONENT_STATUS_OUT_EXPORT_V3,
};
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
pub(crate) use scalar_algebra_component_v5::{
    emit_private_scalar_algebra_core_v5,
    CANONICAL_EXPORTS as SCALAR_ALGEBRA_COMPONENT_CANONICAL_EXPORTS_V5,
    SOURCE_V5 as SCALAR_ALGEBRA_COMPONENT_SOURCE_V5,
};
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
pub(crate) use source_result_component_v4::{
    emit_private_source_result_core_v4,
    CANONICAL_EXPORT as SOURCE_RESULT_COMPONENT_CANONICAL_EXPORT_V4,
    STATUS_OUT_EXPORT as SOURCE_RESULT_COMPONENT_STATUS_OUT_EXPORT_V4,
};

const I32: u8 = 0x7f;
const I64: u8 = 0x7e;
const F32: u8 = 0x7d;
const F64: u8 = 0x7c;
const SCALAR_IMPORT_COUNT: u32 = 7;
/// Host imports backing owned strings, appended after the scalar imports:
/// `spx_string_new`, `spx_string_eq`, `spx_string_clone`.
const STRING_IMPORT_COUNT: u32 = 3;
/// Linear-memory base offset for string literal bytes.
const STRING_DATA_BASE: u32 = 1024;
/// Fixed import indexes on the scalar-only string path (no owned adapters).
const STRING_IMPORT_BASE_NEW: u32 = 7;
const STRING_IMPORT_BASE_EQ: u32 = 8;
const STRING_IMPORT_BASE_CLONE: u32 = 9;
/// Host imports backing compiler-owned string operations, appended after the
/// base string imports only when a program reaches the operations so existing
/// modules keep their exact bytes: `spx_string_len`, `spx_string_concat`.
const STRING_OPS_IMPORT_COUNT: u32 = 2;
const STRING_OPS_IMPORT_BASE_LEN: u32 = 10;
const STRING_OPS_IMPORT_BASE_CONCAT: u32 = 11;
/// Host imports backing breadth-v2 compiler-owned string operations, emitted
/// as one group only when a program reaches a v2 operation: first-wave-only
/// modules keep their exact bytes, and the group's base index follows the
/// first wave so every admitted combination stays deterministic:
/// `spx_string_starts_with`, `spx_string_contains`, `spx_string_len_chars`,
/// `spx_string_from_char`.
const STRING_OPS_V2_IMPORT_COUNT: u32 = 4;

/// Deterministic literal table shared by the data segment and expression
/// lowering; identical contents always map to one offset.
#[derive(Default)]
struct StringData {
    offsets: HashMap<String, u32>,
    bytes: Vec<u8>,
}

impl StringData {
    fn intern(&mut self, value: &str) -> (u32, u32) {
        if let Some(offset) = self.offsets.get(value) {
            return (*offset, value.len() as u32);
        }
        let offset = STRING_DATA_BASE + self.bytes.len() as u32;
        self.offsets.insert(value.to_owned(), offset);
        self.bytes.extend_from_slice(value.as_bytes());
        (offset, value.len() as u32)
    }
}

/// Whether any resolved function admits an owned string in a signature,
/// body, or contract.
fn program_uses_strings(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        if matches!(function.return_type, ResolvedType::String)
            || function
                .params
                .iter()
                .any(|param| matches!(param.ty, ResolvedType::String))
        {
            return true;
        }
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if matches!(expression.ty, ResolvedType::String)
            || matches!(expression.kind, ResolvedExprKind::String(_))
        {
            return true;
        }
        match &expression.kind {
            ResolvedExprKind::Call { args, .. } => pending.extend(args.iter()),
            ResolvedExprKind::NativeRustImportCall(call) => pending.extend(call.args.iter()),
            ResolvedExprKind::HostCommandCall(call) => pending.extend(call.args.iter()),
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => pending.extend([source.as_ref(), start.as_ref(), end.as_ref()]),
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
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
                        pending.push(guard.as_ref());
                    }
                    pending.push(&arm.value);
                }
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push(base);
                pending.extend(fields.iter().map(|field| &field.value));
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
    }
    false
}

fn program_uses_byte_data(program: &ResolvedProgram) -> bool {
    let mut pending = Vec::new();
    for function in &program.functions {
        if matches!(
            function.return_type,
            ResolvedType::SliceU8 | ResolvedType::Bytes | ResolvedType::ArrayU8(_)
        ) || function.params.iter().any(|param| {
            matches!(
                param.ty,
                ResolvedType::SliceU8 | ResolvedType::Bytes | ResolvedType::ArrayU8(_)
            )
        }) {
            return true;
        }
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if matches!(
            expression.ty,
            ResolvedType::SliceU8 | ResolvedType::Bytes | ResolvedType::ArrayU8(_)
        ) || matches!(
            expression.kind,
            ResolvedExprKind::ArrayU8(_)
                | ResolvedExprKind::RepeatArrayU8 { .. }
                | ResolvedExprKind::BorrowPlace { .. }
        ) {
            return true;
        }
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::byte_ops::by_id(callee.as_str()).is_some() {
                return true;
            }
        }
        match &expression.kind {
            ResolvedExprKind::Call { args, .. } => pending.extend(args),
            ResolvedExprKind::NativeRustImportCall(call) => pending.extend(&call.args),
            ResolvedExprKind::HostCommandCall(call) => pending.extend(&call.args),
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => pending.extend([source.as_ref(), start.as_ref(), end.as_ref()]),
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
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
                pending.extend(fields.iter().map(|field| &field.value))
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
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push(base);
                pending.extend(fields.iter().map(|field| &field.value));
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
    }
    false
}

/// Whether any resolved function body or contract calls a compiler-owned
/// string operation intrinsic.
fn program_uses_string_ops(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str()).is_some() {
                return true;
            }
        }
        match &expression.kind {
            ResolvedExprKind::Call { args, .. } => pending.extend(args.iter()),
            ResolvedExprKind::NativeRustImportCall(call) => pending.extend(call.args.iter()),
            ResolvedExprKind::HostCommandCall(call) => pending.extend(call.args.iter()),
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => pending.extend([source.as_ref(), start.as_ref(), end.as_ref()]),
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
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
                        pending.push(guard.as_ref());
                    }
                    pending.push(&arm.value);
                }
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push(base);
                pending.extend(fields.iter().map(|field| &field.value));
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
    }
    false
}

/// Whether any resolved function body or contract calls a breadth-v2
/// compiler-owned string operation intrinsic.
fn program_uses_string_ops_v2(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str())
                .is_some_and(crate::string_ops::StringOp::is_breadth_v2)
            {
                return true;
            }
        }
        match &expression.kind {
            ResolvedExprKind::Call { args, .. } => pending.extend(args.iter()),
            ResolvedExprKind::NativeRustImportCall(call) => pending.extend(call.args.iter()),
            ResolvedExprKind::HostCommandCall(call) => pending.extend(call.args.iter()),
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => pending.extend([source.as_ref(), start.as_ref(), end.as_ref()]),
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
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
                pending.extend(arms.iter().map(|arm| &arm.value));
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push(base);
                pending.extend(fields.iter().map(|field| &field.value));
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
    }
    false
}

/// Collect every distinct literal in deterministic pre-order with offsets.
fn collect_string_data(program: &ResolvedProgram) -> StringData {
    let mut data = StringData::default();
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::String(value) = &expression.kind {
            data.intern(value);
        }
        // Reuse the same traversal shape as `program_uses_strings`, pushing
        // children in reverse so pre-order stays deterministic.
        match &expression.kind {
            ResolvedExprKind::Call { args, .. } => {
                for arg in args.iter().rev() {
                    pending.push(arg);
                }
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                for arg in call.args.iter().rev() {
                    pending.push(arg);
                }
            }
            ResolvedExprKind::HostCommandCall(call) => {
                for arg in call.args.iter().rev() {
                    pending.push(arg);
                }
            }
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
                for field in fields.iter().rev() {
                    pending.push(&field.value);
                }
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                for arm in arms.iter().rev() {
                    if let Some(guard) = &arm.guard {
                        pending.push(guard.as_ref());
                    }
                    pending.push(&arm.value);
                }
                pending.push(scrutinee);
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                for field in fields.iter().rev() {
                    pending.push(&field.value);
                }
                pending.push(base);
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
    }
    data
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct Signature {
    pub(super) params: Vec<u8>,
    pub(super) results: Vec<u8>,
}

#[derive(Default)]
struct LocalLayout<'a> {
    declarations: Vec<ResolvedType>,
    lets: HashMap<ValueId, u32>,
    /// Two reserved i64 scratch slots used by inline checked i32 arithmetic.
    wide_scratch: [u32; 2],
    /// Checked u8 arithmetic stages operands and results in two trailing
    /// scratch locals so the failure trap never taints live stack values.
    u8_scratch: Option<(u32, u32)>,
    /// Portable usize arithmetic stages two semantic-u64 operands without
    /// reinterpreting them as host- or pointer-width integers.
    usize_scratch: Option<(u32, u32)>,
    /// Refutable Match v1: one staging local per scalar match expression,
    /// keyed by expression identity. The scrutinee evaluates once here and
    /// every arm test re-reads it.
    match_scratch: HashMap<String, u32>,
    /// Interned string literal offsets for the whole program, when strings
    /// are admitted at all.
    string_data: Option<&'a StringData>,
    /// Base import index of the breadth-v2 string operation group; only v2
    /// call sites consult it, so first-wave modules are unaffected.
    string_ops_v2_base: u32,
    /// Module-local helper indexes used only by the additive borrowed-text
    /// profile. Legacy modules leave this absent and retain exact bytes.
    text_intrinsics: Option<TextIntrinsicIndexes>,
}

#[derive(Clone, Copy)]
struct TextIntrinsicIndexes {
    starts_with: u32,
    contains: u32,
}

trait ByteOutput: std::ops::Deref<Target = [u8]> {
    fn push(&mut self, value: u8);
    fn extend_bytes(&mut self, values: &[u8]);
}

impl ByteOutput for Vec<u8> {
    fn push(&mut self, value: u8) {
        Vec::push(self, value);
    }

    fn extend_bytes(&mut self, values: &[u8]) {
        self.extend_from_slice(values);
    }
}

impl ByteOutput for crate::bounded_output::CappedVec {
    fn push(&mut self, value: u8) {
        self.push(value);
    }

    fn extend_bytes(&mut self, values: &[u8]) {
        self.extend_from_slice(values);
    }
}

pub fn emit_module(program: &Program) -> Result<Vec<u8>, Diagnostic> {
    reject_native_rust_imports(program)?;
    let resolved = hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|item| item.severity.is_error())
            .unwrap_or_else(|| Diagnostic::io("SPX-W100", "HIR resolution failed"))
    })?;
    emit_resolved_module(&resolved)
}

/// Test-only raw module for target-level transcript evidence. Production
/// consumers must use the trusted Project facade, which owns failure wiping.
#[cfg(test)]
pub(crate) fn emit_module_with_stdout_transcript(program: &Program) -> Result<Vec<u8>, Diagnostic> {
    let resolved = hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|item| item.severity.is_error())
            .unwrap_or_else(|| Diagnostic::io("SPX-W100", "HIR resolution failed"))
    })?;
    emit_resolved_module_with_stdout_transcript(&resolved)
}

/// Emit the bounded Public Scalar Export Profile v1 for the selected stable IDs.
///
/// Unlike the legacy web module, this profile exports only the selected scalar
/// adapters and deliberately omits `semaprax_main`.
pub fn emit_module_with_scalar_exports(
    program: &Program,
    export_ids: &[String],
) -> Result<Vec<u8>, Diagnostic> {
    reject_native_rust_imports(program)?;
    let resolved = hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|item| item.severity.is_error())
            .unwrap_or_else(|| Diagnostic::io("SPX-W100", "HIR resolution failed"))
    })?;
    emit_resolved_module_with_scalar_exports(&resolved, export_ids)
}

/// Emit the bounded Public Borrowed Text Export Profile v1 for selected
/// stable identities. Each borrowed `str` parameter expands to an exact
/// `(i32 pointer, i32 byte_length)` pair at the raw boundary; internally it
/// remains one packed i64 view.
pub fn emit_module_with_text_exports(
    program: &Program,
    export_ids: &[String],
) -> Result<Vec<u8>, Diagnostic> {
    reject_native_rust_imports(program)?;
    let resolved = hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|item| item.severity.is_error())
            .unwrap_or_else(|| Diagnostic::io("SPX-W100", "HIR resolution failed"))
    })?;
    emit_resolved_module_with_text_exports(&resolved, export_ids)
}

/// Emit the bounded Public Useful Data Export v1 profile for selected stable
/// identities. Every public parameter is an exact `borrow Slice<u8>` root and
/// expands to `(i32 scratch_offset, i32 byte_length)` at the raw boundary.
pub fn emit_module_with_byte_exports(
    program: &Program,
    export_ids: &[String],
) -> Result<Vec<u8>, Diagnostic> {
    reject_native_rust_imports(program)?;
    let resolved = hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|item| item.severity.is_error())
            .unwrap_or_else(|| Diagnostic::io("SPX-W100", "HIR resolution failed"))
    })?;
    emit_resolved_module_with_byte_exports(&resolved, export_ids)
}

/// Emit a WebAssembly core module from verified, identity-resolved HIR.
///
/// Most callers should use [`emit_module`], which resolves and verifies parsed
/// source first. This entry point exists for semantic consumers that already
/// hold HIR and keeps all backend lowering independent of source-level names.
pub fn emit_resolved_module(program: &ResolvedProgram) -> Result<Vec<u8>, Diagnostic> {
    emit_resolved_module_internal(program, &[], &[])
}

/// Test-only raw target projection; not a public semantic runtime API.
#[cfg(test)]
pub(crate) fn emit_resolved_module_with_stdout_transcript(
    program: &ResolvedProgram,
) -> Result<Vec<u8>, Diagnostic> {
    crate::host_io_ops::validate_stdout_profile_authority(program)?;
    aggregate::emit_stdout_transcript(program)
}

/// Emit the bounded Public Scalar Export Profile v1 from resolved HIR.
pub fn emit_resolved_module_with_scalar_exports(
    program: &ResolvedProgram,
    export_ids: &[String],
) -> Result<Vec<u8>, Diagnostic> {
    let plans = scalar_exports::prepare(program, export_ids)?;
    emit_resolved_module_internal(program, &plans, &[])
}

/// Emit Public Borrowed Text Export Profile v1 from resolved HIR.
pub fn emit_resolved_module_with_text_exports(
    program: &ResolvedProgram,
    export_ids: &[String],
) -> Result<Vec<u8>, Diagnostic> {
    let plans = text_exports::prepare(program, export_ids)?;
    emit_resolved_module_internal(program, &[], &plans)
}

/// Emit Public Useful Data Export v1 from already validated resolved HIR.
/// The aggregate byte backend emits only selected raw adapters and the exact
/// public scratch metadata; internal functions remain unexported.
pub fn emit_resolved_module_with_byte_exports(
    program: &ResolvedProgram,
    export_ids: &[String],
) -> Result<Vec<u8>, Diagnostic> {
    let plans = data_exports::prepare(program, export_ids)?;
    aggregate::emit_byte_exports(program, &plans)
}

/// Emit descriptor-driven raw adapters for the closed WP-10/WP-11 results.
pub fn emit_resolved_module_with_owned_data_exports(
    program: &ResolvedProgram,
    descriptor: &crate::project::PublicApiDescriptor,
) -> Result<Vec<u8>, Diagnostic> {
    let selected = descriptor
        .exports()
        .iter()
        .map(|export| export.stable_id().as_str().to_owned())
        .collect::<Vec<_>>();
    let subject = crate::project::PublicApiSubject {
        project_schema: descriptor.project_schema(),
        project_revision: descriptor.project_revision(),
        workspace_revision: descriptor.workspace_revision(),
        project_graph_digest: descriptor.project_graph_digest(),
    };
    let replayed = crate::project::replay_public_api_descriptor(
        program,
        &selected,
        subject,
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )?;
    if &replayed != descriptor {
        return Err(Diagnostic::io(
            "SPX-W124",
            "owned-data target descriptor does not match held HIR",
        ));
    }
    let plans = owned_data_exports::prepare(program, descriptor)?;
    aggregate::emit_owned_data_exports(program, &plans)
}

/// Emit selected Useful Data wrappers plus success-only stdout transcript
/// exports from already validated resolved HIR.
pub(crate) fn emit_resolved_module_with_byte_exports_and_stdout_transcript(
    program: &ResolvedProgram,
    export_ids: &[String],
) -> Result<Vec<u8>, Diagnostic> {
    let plans = data_exports::prepare_with_stdout_transcript(program, export_ids)?;
    aggregate::emit_byte_exports_with_stdout_transcript(program, &plans)
}

/// Emit the closed Useful Data Command v2 Wasm boundary. Unlike the frozen
/// v1 helper above, this authenticates the exact two-slice/bool command shape
/// through the target-neutral command plan shared with native projection.
pub(crate) fn emit_resolved_useful_data_command_v2(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let plans = data_exports::prepare_command_v2(program, command_id)?;
    aggregate::emit_useful_data_command_v2(program, &plans)
}

/// Emit the additive Project-v6 Language Command I/O v1 boundary.
pub(crate) fn emit_resolved_language_command_io_v1(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let plan = command_io::prepare(
        program,
        command_id,
        crate::command_io_ops::CommandOperationProfile::LanguageV1,
    )?;
    aggregate::emit_language_command_io(program, &plan)
}

/// Emit the additive Project-v7 line-command boundary. Admission remains in
/// the shared command profile; the backend adds range descriptors and the
/// independent command-output status marker only when those operations are
/// actually reachable, preserving Project-v6 bytes otherwise.
pub(crate) fn emit_resolved_line_command_io_v1(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let plan = command_io::prepare(
        program,
        command_id,
        crate::command_io_ops::CommandOperationProfile::LineV1,
    )?;
    aggregate::emit_language_command_io(program, &plan)
}

fn emit_resolved_module_internal(
    program: &ResolvedProgram,
    scalar_exports: &[scalar_exports::ScalarExportPlan],
    text_exports: &[text_exports::TextExportPlan],
) -> Result<Vec<u8>, Diagnostic> {
    if !scalar_exports.is_empty() && !text_exports.is_empty() {
        return Err(Diagnostic::io(
            "SPX-W119",
            "scalar-v1 and borrowed-text-v1 exports cannot share one module",
        ));
    }
    let has_public_profile = !scalar_exports.is_empty() || !text_exports.is_empty();
    if program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .any(|import| import.native_rust)
    {
        return Err(Diagnostic::io(
            "SPX-W114",
            "Native Rust imports are unavailable for WebAssembly targets",
        ));
    }
    hir::validate(program)?;
    let concrete_variants = VariantLayoutCache::build(program, VariantTarget::Wasm32)?;
    let has_authored_aggregate = program.types.iter().any(|declaration| {
        matches!(
            &declaration.kind,
            ResolvedTypeDeclarationKind::Record { .. }
                | ResolvedTypeDeclarationKind::Class { .. }
                | ResolvedTypeDeclarationKind::Variant { .. }
        ) && !program
            .declarations
            .declaration(&declaration.id)
            .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned)
    });
    if has_authored_aggregate || !concrete_variants.is_empty() || program_uses_byte_data(program) {
        if has_public_profile {
            return Err(Diagnostic::io(
                "SPX-W115",
                "Public Scalar Export Profile v1 does not admit aggregate or variant lowering",
            ));
        }
        return aggregate::emit(program);
    }
    let owned_plans = owned::plan(program)?;
    if has_public_profile && !owned_plans.is_empty() {
        return Err(Diagnostic::io(
            "SPX-W115",
            "Public Scalar Export Profile v1 does not admit owned-resource adapters",
        ));
    }
    // Owned strings lower through dedicated host imports; they are admitted
    // only on the scalar core path so import indexes stay deterministic.
    let uses_strings = program_uses_strings(program);
    if uses_strings && (!owned_plans.is_empty() || has_authored_aggregate) {
        return Err(Diagnostic::io(
            "SPX-W116",
            "string values are outside aggregate and resource WebAssembly lowering",
        ));
    }
    let uses_string_ops = program_uses_string_ops(program);
    let uses_string_ops_v2 = program_uses_string_ops_v2(program);
    let string_data = if uses_strings {
        collect_string_data(program)
    } else {
        StringData::default()
    };
    let string_import_base = SCALAR_IMPORT_COUNT;
    let import_count = if owned_plans.is_empty() {
        SCALAR_IMPORT_COUNT
    } else {
        SCALAR_IMPORT_COUNT + owned::IMPORT_NAMES.len() as u32
    } + if uses_strings { STRING_IMPORT_COUNT } else { 0 }
        + if uses_string_ops {
            STRING_OPS_IMPORT_COUNT
        } else {
            0
        }
        + if uses_string_ops_v2 {
            STRING_OPS_V2_IMPORT_COUNT
        } else {
            0
        };
    let mut types = Vec::<Signature>::new();
    let mut type_indexes = HashMap::<Signature, u32>::new();
    // The v2 operation group's base index follows the first wave so every
    // admitted subset keeps deterministic, gap-free import indexes.
    let string_ops_v2_base = SCALAR_IMPORT_COUNT
        + if uses_strings { STRING_IMPORT_COUNT } else { 0 }
        + if uses_string_ops {
            STRING_OPS_IMPORT_COUNT
        } else {
            0
        };
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
            params: if !has_public_profile {
                vec![]
            } else {
                vec![I32]
            },
            results: vec![],
        },
        &mut types,
        &mut type_indexes,
    );
    let string_import_types = if uses_strings {
        Some((
            string_import_base,
            [
                // spx_string_new(ptr: i32, len: i32) -> handle: i64
                intern_type(
                    Signature {
                        params: vec![I32, I32],
                        results: vec![I64],
                    },
                    &mut types,
                    &mut type_indexes,
                ),
                // spx_string_eq(a: i64, b: i64) -> bool: i32
                intern_type(
                    Signature {
                        params: vec![I64, I64],
                        results: vec![I32],
                    },
                    &mut types,
                    &mut type_indexes,
                ),
                // spx_string_clone(handle: i64) -> handle: i64
                intern_type(
                    Signature {
                        params: vec![I64],
                        results: vec![I64],
                    },
                    &mut types,
                    &mut type_indexes,
                ),
            ],
        ))
    } else {
        None
    };
    let string_ops_import_types = if uses_string_ops {
        Some([
            // spx_string_len(handle: i64) -> byte length: i64
            intern_type(
                Signature {
                    params: vec![I64],
                    results: vec![I64],
                },
                &mut types,
                &mut type_indexes,
            ),
            // spx_string_concat(a: i64, b: i64) -> joined handle: i64
            intern_type(
                Signature {
                    params: vec![I64, I64],
                    results: vec![I64],
                },
                &mut types,
                &mut type_indexes,
            ),
        ])
    } else {
        None
    };
    let string_ops_v2_import_types = if uses_string_ops_v2 {
        Some([
            // spx_string_starts_with(value: i64, prefix: i64) -> bool: i32
            intern_type(
                Signature {
                    params: vec![I64, I64],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            // spx_string_contains(value: i64, needle: i64) -> bool: i32
            intern_type(
                Signature {
                    params: vec![I64, I64],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            // spx_string_len_chars(handle: i64) -> scalar count: i64
            intern_type(
                Signature {
                    params: vec![I64],
                    results: vec![I64],
                },
                &mut types,
                &mut type_indexes,
            ),
            // spx_string_from_char(scalar: i32) -> handle: i64
            intern_type(
                Signature {
                    params: vec![I32],
                    results: vec![I64],
                },
                &mut types,
                &mut type_indexes,
            ),
        ])
    } else {
        None
    };

    let owned_import_types = if owned_plans.is_empty() {
        None
    } else {
        Some([
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
                    params: vec![I32, I32],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32],
                    results: vec![],
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
                    params: vec![I32],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32, I32],
                    results: vec![],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32],
                    results: vec![],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32, I32],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32, I32, I32],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32],
                    results: vec![],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32, I32, I32],
                    results: vec![],
                },
                &mut types,
                &mut type_indexes,
            ),
        ])
    };

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
    let mut function_types = Vec::new();
    for (function, _) in &executable_functions {
        let signature = Signature {
            params: function
                .params
                .iter()
                .map(|param| wasm_type(&param.ty))
                .collect::<Result<Vec<_>, _>>()?,
            results: vec![wasm_type(&function.return_type)?],
        };
        function_types.push(intern_type(signature, &mut types, &mut type_indexes));
    }
    let scalar_export_types = scalar_exports
        .iter()
        .map(|plan| {
            intern_type(
                Signature {
                    params: plan.params.iter().map(|ty| ty.wasm_type()).collect(),
                    results: vec![plan.result.wasm_type()],
                },
                &mut types,
                &mut type_indexes,
            )
        })
        .collect::<Vec<_>>();
    let text_validator_type = (!text_exports.is_empty()).then(|| {
        intern_type(
            Signature {
                params: vec![I32, I32],
                results: vec![I32],
            },
            &mut types,
            &mut type_indexes,
        )
    });
    let text_binary_type = (!text_exports.is_empty()).then(|| {
        intern_type(
            Signature {
                params: vec![I64, I64],
                results: vec![I32],
            },
            &mut types,
            &mut type_indexes,
        )
    });
    let text_export_types = text_exports
        .iter()
        .map(|plan| {
            intern_type(
                Signature {
                    params: plan.raw_params(),
                    results: vec![plan.result.internal_wasm_type()],
                },
                &mut types,
                &mut type_indexes,
            )
        })
        .collect::<Vec<_>>();
    let owned_function_types = owned_plans
        .iter()
        .map(|plan| {
            let (params, results) = plan.signature();
            intern_type(Signature { params, results }, &mut types, &mut type_indexes)
        })
        .collect::<Vec<_>>();

    let function_indexes: HashMap<_, _> = executable_functions
        .iter()
        .enumerate()
        .map(|(index, (_, execution))| (execution.clone(), import_count + index as u32))
        .collect();
    let text_helper_base = import_count
        + executable_functions.len() as u32
        + owned_plans.len() as u32
        + scalar_exports.len() as u32;
    let text_intrinsics = (!text_exports.is_empty()).then_some(TextIntrinsicIndexes {
        starts_with: text_helper_base + 1,
        contains: text_helper_base + 2,
    });

    let mut module = crate::bounded_output::CappedVec::from_slice(b"\0asm\x01\0\0\0");
    let mut type_section = crate::bounded_output::CappedVec::new();
    write_u32(&mut type_section, types.len() as u32);
    for signature in &types {
        type_section.push(0x60);
        write_bytes(&mut type_section, &signature.params);
        write_bytes(&mut type_section, &signature.results);
    }
    section(&mut module, 1, type_section);

    let mut imports = crate::bounded_output::CappedVec::new();
    write_u32(&mut imports, import_count);
    for name in ["spx_add", "spx_sub", "spx_mul", "spx_div", "spx_rem"] {
        function_import(&mut imports, "env", name, binary_checked);
    }
    function_import(&mut imports, "env", "spx_neg", unary_checked);
    function_import(&mut imports, "env", "spx_contract_fail", contract_fail);
    if let Some((base, [string_new, string_eq, string_clone])) = string_import_types {
        let _ = base;
        function_import(&mut imports, "env", "spx_string_new", string_new);
        function_import(&mut imports, "env", "spx_string_eq", string_eq);
        function_import(&mut imports, "env", "spx_string_clone", string_clone);
    }
    if let Some([string_len, string_concat]) = string_ops_import_types {
        function_import(&mut imports, "env", "spx_string_len", string_len);
        function_import(&mut imports, "env", "spx_string_concat", string_concat);
    }
    if let Some([starts_with, contains, len_chars, from_char]) = string_ops_v2_import_types {
        function_import(&mut imports, "env", "spx_string_starts_with", starts_with);
        function_import(&mut imports, "env", "spx_string_contains", contains);
        function_import(&mut imports, "env", "spx_string_len_chars", len_chars);
        function_import(&mut imports, "env", "spx_string_from_char", from_char);
    }
    if let Some(type_indexes) = owned_import_types {
        for (name, type_index) in owned::IMPORT_NAMES.into_iter().zip(type_indexes) {
            function_import(&mut imports, "env", name, type_index);
        }
    }
    section(&mut module, 2, imports);

    let mut functions = crate::bounded_output::CappedVec::new();
    write_u32(
        &mut functions,
        (function_types.len()
            + owned_function_types.len()
            + scalar_export_types.len()
            + usize::from(text_validator_type.is_some()) * 3
            + text_export_types.len()) as u32,
    );
    for type_index in function_types {
        write_u32(&mut functions, type_index);
    }
    for type_index in owned_function_types {
        write_u32(&mut functions, type_index);
    }
    for type_index in scalar_export_types {
        write_u32(&mut functions, type_index);
    }
    if let (Some(validator), Some(binary)) = (text_validator_type, text_binary_type) {
        write_u32(&mut functions, validator);
        write_u32(&mut functions, binary);
        write_u32(&mut functions, binary);
    }
    for type_index in text_export_types {
        write_u32(&mut functions, type_index);
    }
    section(&mut module, 3, functions);

    if !owned_plans.is_empty() || uses_strings || !text_exports.is_empty() {
        let mut memories = crate::bounded_output::CappedVec::new();
        write_u32(&mut memories, 1);
        if text_exports.is_empty() {
            memories.extend([0x00, 0x01]); // one-page, unbounded memory
        } else {
            let pages = text_exports::FIXED_MEMORY_PAGES;
            memories.extend([0x01, pages, pages]); // fixed scratch plus private KMP table
        }
        section(&mut module, 5, memories);
    }

    if !text_exports.is_empty() {
        let mut globals = crate::bounded_output::CappedVec::new();
        write_u32(&mut globals, 3);
        // Mutable exact invocation status.
        globals.extend_bytes(&[I32, 0x01, 0x41, 0x00, 0x0b]);
        // Immutable scratch base and capacity metadata.
        globals.extend_bytes(&[I32, 0x00, 0x41]);
        write_i32(&mut globals, text_exports::SCRATCH_BASE as i32);
        globals.push(0x0b);
        globals.extend_bytes(&[I32, 0x00, 0x41]);
        write_i32(&mut globals, text_exports::SCRATCH_CAPACITY as i32);
        globals.push(0x0b);
        section(&mut module, 6, globals);
    }

    // String literal bytes live in one deterministic data segment so host
    // shims can materialize handles with `spx_string_new(ptr, len)`.

    let mut exports = crate::bounded_output::CappedVec::new();
    let legacy_export_count = if !has_public_profile {
        1 + owned_plans.len() as u32 + u32::from(!owned_plans.is_empty() || uses_strings)
    } else {
        0
    };
    write_u32(
        &mut exports,
        legacy_export_count
            + scalar_exports.len() as u32
            + text_exports.len() as u32
            + if text_exports.is_empty() { 0 } else { 4 },
    );
    if !has_public_profile {
        let main_index = program
            .functions
            .iter()
            .position(|function| function.id == program.entrypoint)
            .ok_or_else(|| Diagnostic::io("SPX-W101", "web target requires a main function"))?;
        let main = &program.functions[main_index];
        if !main.params.is_empty() || main.return_type != ResolvedType::I64 {
            return Err(Diagnostic::io(
                "SPX-W101",
                "resolved web entry point must have type `fn main() -> i64`",
            ));
        }
        write_name(&mut exports, "semaprax_main");
        exports.push(0x00);
        write_u32(&mut exports, import_count + main_index as u32);
        if !owned_plans.is_empty() || uses_strings {
            write_name(&mut exports, "memory");
            exports.push(0x02);
            write_u32(&mut exports, 0);
        }
    }
    let adapter_base = import_count + executable_functions.len() as u32;
    for (ordinal, plan) in owned_plans.iter().enumerate() {
        write_name(&mut exports, &plan.export);
        exports.push(0x00);
        write_u32(&mut exports, adapter_base + ordinal as u32);
    }
    let scalar_export_base = adapter_base + owned_plans.len() as u32;
    for (ordinal, plan) in scalar_exports.iter().enumerate() {
        write_name(&mut exports, &plan.wasm_export);
        exports.push(0x00);
        write_u32(&mut exports, scalar_export_base + ordinal as u32);
    }
    if !text_exports.is_empty() {
        for (name, kind, index) in [
            (text_exports::MEMORY_EXPORT, 0x02, 0),
            (text_exports::STATUS_GLOBAL_EXPORT, 0x03, 0),
            (text_exports::SCRATCH_BASE_EXPORT, 0x03, 1),
            (text_exports::SCRATCH_CAPACITY_EXPORT, 0x03, 2),
        ] {
            write_name(&mut exports, name);
            exports.push(kind);
            write_u32(&mut exports, index);
        }
        let text_export_base = text_helper_base + 3;
        for (ordinal, plan) in text_exports.iter().enumerate() {
            write_name(&mut exports, &plan.wasm_export);
            exports.push(0x00);
            write_u32(&mut exports, text_export_base + ordinal as u32);
        }
    }
    section(&mut module, 7, exports);

    let mut code = crate::bounded_output::CappedVec::new();
    write_u32(
        &mut code,
        (executable_functions.len()
            + owned_plans.len()
            + scalar_exports.len()
            + if text_exports.is_empty() { 0 } else { 3 }
            + text_exports.len()) as u32,
    );
    for (function, _) in &executable_functions {
        let mut body = crate::bounded_output::CappedVec::new();
        let result_local = function.params.len() as u32;
        let mut value_indexes: HashMap<_, _> = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| (param.id.clone(), index as u32))
            .collect();
        let mut layout = LocalLayout {
            declarations: vec![function.return_type.clone()],
            lets: HashMap::new(),
            wide_scratch: [0; 2],
            u8_scratch: None,
            usize_scratch: None,
            match_scratch: HashMap::new(),
            string_data: Some(&string_data),
            string_ops_v2_base,
            text_intrinsics,
        };
        for contract in &function.requires {
            collect_locals(contract, function.params.len() as u32, &mut layout)?;
        }
        collect_locals(&function.body, function.params.len() as u32, &mut layout)?;
        for contract in &function.ensures {
            collect_locals(contract, function.params.len() as u32, &mut layout)?;
        }
        if function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(needs_i32_wide_scratch)
        {
            layout.wide_scratch = [
                layout.declarations.len() as u32,
                layout.declarations.len() as u32 + 1,
            ];
            layout.declarations.push(ResolvedType::I64);
            layout.declarations.push(ResolvedType::I64);
        }
        if contains_u8_arithmetic(&function.body)
            || function
                .requires
                .iter()
                .chain(&function.ensures)
                .any(contains_u8_arithmetic)
        {
            let left_index = layout.declarations.len() as u32;
            layout.declarations.push(ResolvedType::U8);
            layout.declarations.push(ResolvedType::U8);
            layout.u8_scratch = Some((left_index, left_index + 1));
        }
        if contains_usize_arithmetic(&function.body)
            || function
                .requires
                .iter()
                .chain(&function.ensures)
                .any(contains_usize_arithmetic)
        {
            let left_index = layout.declarations.len() as u32;
            layout.declarations.push(ResolvedType::Usize);
            layout.declarations.push(ResolvedType::Usize);
            layout.usize_scratch = Some((left_index, left_index + 1));
        }
        value_indexes.extend(layout.lets.iter().map(|(id, index)| (id.clone(), *index)));
        value_indexes.insert(function.result_id.clone(), result_local);
        write_u32(&mut body, layout.declarations.len() as u32);
        for ty in &layout.declarations {
            write_u32(&mut body, 1);
            body.push(wasm_type(ty)?);
        }
        for contract in &function.requires {
            emit_expr(
                &mut body,
                contract,
                &value_indexes,
                &function_indexes,
                &layout,
                None,
            )?;
            emit_contract_guard(&mut body, has_public_profile.then_some(1));
        }
        emit_expr(
            &mut body,
            &function.body,
            &value_indexes,
            &function_indexes,
            &layout,
            None,
        )?;
        body.push(0x21);
        write_u32(&mut body, result_local);
        for contract in &function.ensures {
            emit_expr(
                &mut body,
                contract,
                &value_indexes,
                &function_indexes,
                &layout,
                None,
            )?;
            emit_contract_guard(&mut body, has_public_profile.then_some(2));
        }
        body.push(0x20);
        write_u32(&mut body, result_local);
        body.push(0x0b);
        write_u32(&mut code, body.len() as u32);
        code.extend_from_slice(&body);
    }
    for plan in &owned_plans {
        let mut body = crate::bounded_output::CappedVec::new();
        plan.emit_body_into(&mut body);
        write_u32(&mut code, body.len() as u32);
        code.extend_from_slice(&body);
    }
    for plan in scalar_exports {
        let mut body = crate::bounded_output::CappedVec::new();
        plan.emit_wrapper_body(&mut body, &function_indexes)?;
        write_u32(&mut code, body.len() as u32);
        code.extend_from_slice(&body);
    }
    if !text_exports.is_empty() {
        for emitter in [
            text_exports::emit_utf8_validator_body as fn(&mut crate::bounded_output::CappedVec),
            text_exports::emit_starts_with_body,
            text_exports::emit_contains_body,
        ] {
            let mut body = crate::bounded_output::CappedVec::new();
            emitter(&mut body);
            write_u32(&mut code, body.len() as u32);
            code.extend_from_slice(&body);
        }
        for plan in text_exports {
            let mut body = crate::bounded_output::CappedVec::new();
            plan.emit_wrapper_body(&mut body, &function_indexes, text_helper_base, 0)?;
            write_u32(&mut code, body.len() as u32);
            code.extend_from_slice(&body);
        }
    }
    section(&mut module, 10, code);
    // String literal bytes live in one deterministic data segment so host
    // shims can materialize handles with `spx_string_new(ptr, len)`.
    if uses_strings {
        let mut data = crate::bounded_output::CappedVec::new();
        write_u32(&mut data, 1);
        data.push(0x00); // active segment, memory 0
        data.push(0x41); // i32.const
        write_i32(&mut data, STRING_DATA_BASE as i32);
        data.push(0x0b); // end of init expression
        write_u32(&mut data, string_data.bytes.len() as u32);
        data.extend_bytes(&string_data.bytes);
        section(&mut module, 11, data);
    }
    Ok(module.into_vec())
}

pub fn build_web(program: &Program, output: &Path) -> Result<(), Diagnostic> {
    reject_native_rust_imports(program)?;
    let resolved = hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|item| item.severity.is_error())
            .unwrap_or_else(|| Diagnostic::io("SPX-W100", "HIR resolution failed"))
    })?;
    let owned_plans = owned::plan(&resolved)?;
    std::fs::create_dir_all(output).map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot create web output {}: {error}", output.display()),
        )
    })?;
    let wasm_bytes = emit_resolved_module(&resolved)?;
    std::fs::write(output.join("app.wasm"), &wasm_bytes).map_err(|error| {
        Diagnostic::io(
            "SPX-I302",
            format!("cannot write WebAssembly module: {error}"),
        )
    })?;
    let runtime_exports = owned_plans
        .iter()
        .map(owned::OwnedPlan::runtime_json)
        .collect::<Vec<_>>()
        .join(",");
    let runtime = browser_runtime()
        .replace(
            "__SEMAPRAX_OWNED_EXPORTS__",
            &format!("Object.freeze({{{runtime_exports}}})"),
        )
        .replace(
            "__SEMAPRAX_WASM_SHA256__",
            &format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(&wasm_bytes))
            ),
        );
    std::fs::write(output.join("semaprax.js"), runtime).map_err(|error| {
        Diagnostic::io("SPX-I303", format!("cannot write browser runtime: {error}"))
    })?;
    std::fs::write(output.join("index.html"), browser_html()).map_err(|error| {
        Diagnostic::io("SPX-I304", format!("cannot write web entry page: {error}"))
    })?;
    std::fs::write(
        output.join("package.json"),
        "{\"private\":true,\"type\":\"module\"}\n",
    )
    .map_err(|error| {
        Diagnostic::io(
            "SPX-I306",
            format!("cannot write web package metadata: {error}"),
        )
    })?;
    let owned_manifest = owned_plans
        .iter()
        .map(|plan| plan.manifest_json(&resolved.functions[plan.function_index]))
        .collect::<Vec<_>>()
        .join(",");
    let manifest = format!(
        "{{\"schema\":\"semaprax.web.v3\",\"module\":{},\"graph_revision\":{},\"wasm\":\"app.wasm\",\"entry\":\"semaprax_main\",\"capabilities\":{},\"owned_abi\":{{\"schema\":\"semaprax.wasm-owned.v1\",\"functions\":[{}]}}}}\n",
        quote_json(&program.module),
        quote_json(&graph::revision(program)),
        json_strings(&program.permits),
        owned_manifest,
    );
    std::fs::write(output.join("semaprax.manifest.json"), manifest).map_err(|error| {
        Diagnostic::io("SPX-I305", format!("cannot write web manifest: {error}"))
    })?;
    Ok(())
}

/// Build a fresh, exact-inventory Public Scalar Export Profile v1 package.
///
/// The destination must not exist. All profile admission and artifact
/// rendering completes before the destination directory is created.
pub fn build_web_with_scalar_exports(
    program: &Program,
    output: &Path,
    export_ids: &[String],
) -> Result<(), Diagnostic> {
    reject_native_rust_imports(program)?;
    let resolved = hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|item| item.severity.is_error())
            .unwrap_or_else(|| Diagnostic::io("SPX-W100", "HIR resolution failed"))
    })?;
    let plans = scalar_exports::prepare(&resolved, export_ids)?;
    let wasm_bytes = emit_resolved_module_internal(&resolved, &plans, &[])?;
    let wasm_sha256 = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(&wasm_bytes))
    );
    let runtime = scalar_profile_runtime(&wasm_sha256);
    let bindings = scalar_bindings(&plans, &wasm_sha256);
    let declarations = scalar_declarations(&plans);
    let package = "{\"private\":true,\"type\":\"module\",\"exports\":\"./semaprax.bindings.js\",\"types\":\"./semaprax.bindings.d.ts\"}\n";
    let index = scalar_browser_html();
    let manifest_artifacts: [(&str, &[u8]); 5] = [
        ("index.html", index.as_bytes()),
        ("package.json", package.as_bytes()),
        ("semaprax.bindings.d.ts", declarations.as_bytes()),
        ("semaprax.bindings.js", bindings.as_bytes()),
        ("semaprax.js", runtime.as_bytes()),
    ];
    let manifest = scalar_manifest(program, &plans, &wasm_sha256, &manifest_artifacts);

    let artifacts: [(&str, &[u8]); 7] = [
        ("app.wasm", &wasm_bytes),
        ("semaprax.js", runtime.as_bytes()),
        ("semaprax.bindings.js", bindings.as_bytes()),
        ("semaprax.bindings.d.ts", declarations.as_bytes()),
        ("semaprax.scalar-exports.json", manifest.as_bytes()),
        ("package.json", package.as_bytes()),
        ("index.html", index.as_bytes()),
    ];
    publish_scalar_package(output, &artifacts)
}

/// Build a Project-v1 scalar Web package from one already linked HIR closure.
///
/// This is deliberately a separate package schema from the frozen single-file
/// `semaprax.web.v4` contract. Both native and Web project targets borrow the
/// same linked program used by native-lowering equivalence evidence; this
/// function performs no parsing or HIR resolution.
pub(crate) struct PreparedProjectWeb {
    wasm_bytes: Vec<u8>,
    runtime: String,
    bindings: String,
    declarations: String,
    manifest: String,
}

/// The closed schema for one pathless, deterministic Project Web build.
pub const PROJECT_WEB_BUILD_SCHEMA: &str = "semaprax.project-web-build.v1";
const PROJECT_WEB_BUILD_DIGEST_DOMAIN: &[u8] = b"semaprax.project-web-build.payload.v1\0";

/// Hard ceiling for an inline Project Web build envelope. Callers may select
/// a smaller limit, but cannot widen this process-wide admission boundary.
pub const MAX_PROJECT_WEB_BUILD_BYTES: usize = 16 * 1024 * 1024;

const PROJECT_WEB_PACKAGE: &str = "{\"private\":true,\"type\":\"module\",\"exports\":\"./semaprax.bindings.js\",\"types\":\"./semaprax.bindings.d.ts\"}\n";
const PROJECT_WEB_ARTIFACT_PATHS: [&str; 7] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.scalar-exports.json",
    "package.json",
    "index.html",
];

#[derive(Clone, Copy)]
struct ProjectWebIdentity<'a> {
    project_name: &'a str,
    project_revision: &'a str,
    workspace_revision: &'a str,
    project_graph_digest: &'a str,
    entry_module: &'a str,
}

/// One self-contained Web package returned without filesystem or process
/// authority. The envelope is canonical JSON and every artifact is encoded as
/// lowercase hexadecimal bytes in the fixed publication order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectWebBuild {
    envelope: String,
    payload_digest: String,
    artifact_bytes: usize,
    max_bytes: usize,
}

impl ProjectWebBuild {
    pub fn envelope(&self) -> &str {
        &self.envelope
    }

    pub fn payload_digest(&self) -> &str {
        &self.payload_digest
    }

    pub fn artifact_bytes(&self) -> usize {
        self.artifact_bytes
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Independently decode and replay the closed carrier, including exact
    /// artifact order, byte counts, SHA-256 values, cumulative limits, payload
    /// digest, and canonical JSON rendering.
    pub fn verify(&self) -> Result<(), Diagnostic> {
        verify_project_web_build(self)
    }

    /// Admit an externally transported envelope only after the same exact
    /// replay used for compiler-produced carriers. The caller supplies the
    /// trusted upper bound; a serialized envelope cannot widen it.
    pub fn verify_envelope(envelope: &str, max_bytes: usize) -> Result<Self, Diagnostic> {
        if max_bytes == 0 || max_bytes > MAX_PROJECT_WEB_BUILD_BYTES || envelope.len() > max_bytes {
            return Err(project_web_build_error(
                "Project Web build exceeds the verifier's closed envelope limit",
            ));
        }
        let value: serde_json::Value = serde_json::from_str(envelope)
            .map_err(|_| project_web_build_error("Project Web build envelope is not valid JSON"))?;
        let object = value.as_object().ok_or_else(|| {
            project_web_build_error("Project Web build envelope must be one JSON object")
        })?;
        let payload_digest = object
            .get("payload_digest")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| project_web_build_error("Project Web build payload_digest is invalid"))?
            .to_owned();
        let artifact_bytes = object
            .get("artifact_bytes")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                project_web_build_error("Project Web build artifact_bytes is invalid")
            })?;
        let build = Self {
            envelope: envelope.to_owned(),
            payload_digest,
            artifact_bytes,
            max_bytes,
        };
        build.verify()?;
        Ok(build)
    }
}

impl PreparedProjectWeb {
    pub(crate) fn publish(self, output: &Path) -> Result<(), Diagnostic> {
        let index = scalar_browser_html();
        let artifacts: [(&str, &[u8]); 7] = [
            ("app.wasm", &self.wasm_bytes),
            ("semaprax.js", self.runtime.as_bytes()),
            ("semaprax.bindings.js", self.bindings.as_bytes()),
            ("semaprax.bindings.d.ts", self.declarations.as_bytes()),
            ("semaprax.scalar-exports.json", self.manifest.as_bytes()),
            ("package.json", PROJECT_WEB_PACKAGE.as_bytes()),
            ("index.html", index.as_bytes()),
        ];
        publish_scalar_package(output, &artifacts)
    }

    pub(crate) fn into_inline(
        self,
        project_name: &str,
        project_revision: &str,
        workspace_revision: &str,
        project_graph_digest: &str,
        entry_module: &str,
        max_bytes: usize,
    ) -> Result<ProjectWebBuild, Diagnostic> {
        let artifacts: [(&str, &[u8]); 7] = [
            ("app.wasm", &self.wasm_bytes),
            ("semaprax.js", self.runtime.as_bytes()),
            ("semaprax.bindings.js", self.bindings.as_bytes()),
            ("semaprax.bindings.d.ts", self.declarations.as_bytes()),
            ("semaprax.scalar-exports.json", self.manifest.as_bytes()),
            ("package.json", PROJECT_WEB_PACKAGE.as_bytes()),
            ("index.html", scalar_browser_html().as_bytes()),
        ];
        build_project_web_carrier(
            ProjectWebIdentity {
                project_name,
                project_revision,
                workspace_revision,
                project_graph_digest,
                entry_module,
            },
            max_bytes,
            &artifacts,
        )
    }
}

fn project_web_build_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W117", message)
}

fn build_project_web_carrier(
    identity: ProjectWebIdentity<'_>,
    max_bytes: usize,
    artifacts: &[(&str, &[u8])],
) -> Result<ProjectWebBuild, Diagnostic> {
    if max_bytes == 0 || max_bytes > MAX_PROJECT_WEB_BUILD_BYTES {
        return Err(project_web_build_error(format!(
            "Project Web build max_bytes must be between 1 and {MAX_PROJECT_WEB_BUILD_BYTES}"
        )));
    }
    if artifacts.len() != PROJECT_WEB_ARTIFACT_PATHS.len()
        || artifacts
            .iter()
            .zip(PROJECT_WEB_ARTIFACT_PATHS)
            .any(|((actual, _), expected)| *actual != expected)
    {
        return Err(project_web_build_error(
            "Project Web build artifact inventory is not the closed seven-artifact package",
        ));
    }

    let mut artifact_bytes = 0usize;
    for (_, bytes) in artifacts {
        artifact_bytes = artifact_bytes.checked_add(bytes.len()).ok_or_else(|| {
            project_web_build_error("Project Web build cumulative artifact bytes overflowed")
        })?;
        if artifact_bytes > max_bytes {
            return Err(project_web_build_error(format!(
                "Project Web build cumulative artifact bytes exceed max_bytes {max_bytes}"
            )));
        }
    }

    let payload = render_project_web_build_payload(identity, max_bytes, artifact_bytes, artifacts)?;
    let payload_digest = project_web_payload_digest(payload.as_bytes());
    let mut envelope = payload;
    debug_assert!(envelope.ends_with('}'));
    envelope.pop();
    write!(
        envelope,
        ",\"payload_digest\":{}}}",
        quote_json(&payload_digest)
    )
    .expect("writing canonical Project Web build JSON to String cannot fail");
    if envelope.len() > max_bytes {
        return Err(project_web_build_error(format!(
            "Project Web build envelope bytes exceed max_bytes {max_bytes}"
        )));
    }
    let build = ProjectWebBuild {
        envelope,
        payload_digest,
        artifact_bytes,
        max_bytes,
    };
    verify_project_web_build(&build)?;
    Ok(build)
}

fn render_project_web_build_payload(
    identity: ProjectWebIdentity<'_>,
    max_bytes: usize,
    artifact_bytes: usize,
    artifacts: &[(&str, &[u8])],
) -> Result<String, Diagnostic> {
    let mut payload = format!(
        "{{\"schema\":{},\"project_schema\":\"semaprax.project.v1\",\"project\":{},\"project_revision\":{},\"workspace_revision\":{},\"project_graph_digest\":{},\"entry_module\":{},\"encoding\":\"hex-lower\",\"limits\":{{\"max_bytes\":{max_bytes}}},\"artifact_count\":7,\"artifact_bytes\":{artifact_bytes},\"artifacts\":[",
        quote_json(PROJECT_WEB_BUILD_SCHEMA),
        quote_json(identity.project_name),
        quote_json(identity.project_revision),
        quote_json(identity.workspace_revision),
        quote_json(identity.project_graph_digest),
        quote_json(identity.entry_module),
    );
    if payload.len() > max_bytes {
        return Err(project_web_build_error(format!(
            "Project Web build envelope bytes exceed max_bytes {max_bytes}"
        )));
    }
    for (index, (path, bytes)) in artifacts.iter().enumerate() {
        if index != 0 {
            payload.push(',');
        }
        let sha256 = format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(bytes))
        );
        write!(
            payload,
            "{{\"path\":{},\"bytes\":{},\"sha256\":{},\"content_hex\":\"",
            quote_json(path),
            bytes.len(),
            quote_json(&sha256),
        )
        .expect("writing canonical Project Web artifact JSON to String cannot fail");
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in *bytes {
            payload.push(char::from(HEX[usize::from(byte >> 4)]));
            payload.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        payload.push_str("\"}");
        if payload.len() > max_bytes {
            return Err(project_web_build_error(format!(
                "Project Web build envelope bytes exceed max_bytes {max_bytes}"
            )));
        }
    }
    payload.push_str("],\"nonclaims\":[\"no_filesystem_authority\",\"no_process_authority\",\"no_publication_or_cache\",\"transport_integrity_not_compiler_provenance\"]}");
    if payload.len() > max_bytes {
        return Err(project_web_build_error(format!(
            "Project Web build envelope bytes exceed max_bytes {max_bytes}"
        )));
    }
    Ok(payload)
}

fn verify_project_web_build(build: &ProjectWebBuild) -> Result<(), Diagnostic> {
    if build.max_bytes == 0
        || build.max_bytes > MAX_PROJECT_WEB_BUILD_BYTES
        || build.envelope.len() > build.max_bytes
    {
        return Err(project_web_build_error(
            "Project Web build exceeds its closed envelope limit",
        ));
    }
    let value: serde_json::Value = serde_json::from_str(&build.envelope)
        .map_err(|_| project_web_build_error("Project Web build envelope is not valid JSON"))?;
    let object = value.as_object().ok_or_else(|| {
        project_web_build_error("Project Web build envelope must be one JSON object")
    })?;
    const KEYS: [&str; 14] = [
        "schema",
        "project_schema",
        "project",
        "project_revision",
        "workspace_revision",
        "project_graph_digest",
        "entry_module",
        "encoding",
        "limits",
        "artifact_count",
        "artifact_bytes",
        "artifacts",
        "nonclaims",
        "payload_digest",
    ];
    if object.len() != KEYS.len() || KEYS.iter().any(|key| !object.contains_key(*key)) {
        return Err(project_web_build_error(
            "Project Web build envelope has a foreign or missing field",
        ));
    }
    let string = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| project_web_build_error(format!("Project Web build {key} is invalid")))
    };
    if string("schema")? != PROJECT_WEB_BUILD_SCHEMA
        || string("project_schema")? != "semaprax.project.v1"
        || string("encoding")? != "hex-lower"
    {
        return Err(project_web_build_error(
            "Project Web build schema or encoding is invalid",
        ));
    }
    let limits = object
        .get("limits")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| project_web_build_error("Project Web build limits are invalid"))?;
    if limits.len() != 1 {
        return Err(project_web_build_error(
            "Project Web build limits are not closed",
        ));
    }
    let declared_max = limits
        .get("max_bytes")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| project_web_build_error("Project Web build max_bytes is invalid"))?;
    if declared_max != build.max_bytes {
        return Err(project_web_build_error(
            "Project Web build max_bytes disagrees with its carrier",
        ));
    }
    if object
        .get("artifact_count")
        .and_then(serde_json::Value::as_u64)
        != Some(7)
    {
        return Err(project_web_build_error(
            "Project Web build artifact_count is not seven",
        ));
    }
    let rows = object
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .filter(|rows| rows.len() == PROJECT_WEB_ARTIFACT_PATHS.len())
        .ok_or_else(|| {
            project_web_build_error("Project Web build artifact inventory is invalid")
        })?;
    let mut decoded = Vec::with_capacity(rows.len());
    let mut replayed_artifact_bytes = 0usize;
    for (row, expected_path) in rows.iter().zip(PROJECT_WEB_ARTIFACT_PATHS) {
        let row = row.as_object().ok_or_else(|| {
            project_web_build_error("Project Web build artifact must be one JSON object")
        })?;
        if row.len() != 4
            || ["path", "bytes", "sha256", "content_hex"]
                .iter()
                .any(|key| !row.contains_key(*key))
        {
            return Err(project_web_build_error(
                "Project Web build artifact has a foreign or missing field",
            ));
        }
        if row.get("path").and_then(serde_json::Value::as_str) != Some(expected_path) {
            return Err(project_web_build_error(
                "Project Web build artifact order or path is invalid",
            ));
        }
        let declared_bytes = row
            .get("bytes")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| project_web_build_error("Project Web build byte count is invalid"))?;
        let content = row
            .get("content_hex")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| project_web_build_error("Project Web build content_hex is invalid"))?;
        let bytes = decode_lower_hex(content)?;
        if bytes.len() != declared_bytes {
            return Err(project_web_build_error(
                "Project Web build artifact byte count disagrees with content",
            ));
        }
        let expected_sha256 = format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(&bytes))
        );
        if row.get("sha256").and_then(serde_json::Value::as_str) != Some(expected_sha256.as_str()) {
            return Err(project_web_build_error(
                "Project Web build artifact SHA-256 disagrees with content",
            ));
        }
        replayed_artifact_bytes = replayed_artifact_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| project_web_build_error("Project Web build byte count overflowed"))?;
        if replayed_artifact_bytes > build.max_bytes {
            return Err(project_web_build_error(
                "Project Web build cumulative artifact bytes exceed max_bytes",
            ));
        }
        decoded.push(bytes);
    }
    let declared_artifact_bytes = object
        .get("artifact_bytes")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| project_web_build_error("Project Web build artifact_bytes is invalid"))?;
    if declared_artifact_bytes != replayed_artifact_bytes
        || declared_artifact_bytes != build.artifact_bytes
    {
        return Err(project_web_build_error(
            "Project Web build cumulative artifact bytes disagree",
        ));
    }
    verify_embedded_project_web_manifest(
        &decoded,
        string("project")?,
        string("project_revision")?,
        string("workspace_revision")?,
        string("project_graph_digest")?,
        string("entry_module")?,
    )?;
    let nonclaims = object
        .get("nonclaims")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| project_web_build_error("Project Web build nonclaims are invalid"))?;
    let expected_nonclaims = [
        "no_filesystem_authority",
        "no_process_authority",
        "no_publication_or_cache",
        "transport_integrity_not_compiler_provenance",
    ];
    if nonclaims.len() != expected_nonclaims.len()
        || nonclaims
            .iter()
            .zip(expected_nonclaims)
            .any(|(actual, expected)| actual.as_str() != Some(expected))
    {
        return Err(project_web_build_error(
            "Project Web build nonclaims are not the closed vocabulary",
        ));
    }
    let artifact_refs = PROJECT_WEB_ARTIFACT_PATHS
        .iter()
        .copied()
        .zip(decoded.iter().map(Vec::as_slice))
        .collect::<Vec<_>>();
    let payload = render_project_web_build_payload(
        ProjectWebIdentity {
            project_name: string("project")?,
            project_revision: string("project_revision")?,
            workspace_revision: string("workspace_revision")?,
            project_graph_digest: string("project_graph_digest")?,
            entry_module: string("entry_module")?,
        },
        build.max_bytes,
        build.artifact_bytes,
        &artifact_refs,
    )?;
    let replayed_digest = project_web_payload_digest(payload.as_bytes());
    if string("payload_digest")? != replayed_digest || build.payload_digest != replayed_digest {
        return Err(project_web_build_error(
            "Project Web build payload digest disagrees with exact replay",
        ));
    }
    let mut canonical = payload;
    canonical.pop();
    write!(
        canonical,
        ",\"payload_digest\":{}}}",
        quote_json(&replayed_digest)
    )
    .expect("writing canonical Project Web build JSON to String cannot fail");
    if canonical != build.envelope {
        return Err(project_web_build_error(
            "Project Web build envelope is not canonical exact replay",
        ));
    }
    Ok(())
}

fn project_web_payload_digest(payload: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(PROJECT_WEB_BUILD_DIGEST_DOMAIN);
    digest.update((payload.len() as u64).to_le_bytes());
    digest.update(payload);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn verify_embedded_project_web_manifest(
    artifacts: &[Vec<u8>],
    project: &str,
    project_revision: &str,
    workspace_revision: &str,
    project_graph_digest: &str,
    entry_module: &str,
) -> Result<(), Diagnostic> {
    let manifest_bytes = artifacts
        .get(4)
        .ok_or_else(|| project_web_build_error("Project Web build embedded manifest is absent"))?;
    let manifest_source = std::str::from_utf8(manifest_bytes)
        .map_err(|_| project_web_build_error("Project Web build embedded manifest is not UTF-8"))?;
    if !manifest_source.ends_with('\n') {
        return Err(project_web_build_error(
            "Project Web build embedded manifest is not canonical newline-terminated JSON",
        ));
    }
    let manifest: serde_json::Value = serde_json::from_str(manifest_source).map_err(|_| {
        project_web_build_error("Project Web build embedded manifest is not valid JSON")
    })?;
    let object = manifest.as_object().ok_or_else(|| {
        project_web_build_error("Project Web build embedded manifest must be one JSON object")
    })?;
    const MANIFEST_KEYS: [&str; 10] = [
        "schema",
        "project_schema",
        "project",
        "project_revision",
        "workspace_revision",
        "project_graph_digest",
        "entry_module",
        "capabilities",
        "artifacts",
        "scalar_abi",
    ];
    if object.len() != MANIFEST_KEYS.len()
        || MANIFEST_KEYS.iter().any(|key| !object.contains_key(*key))
    {
        return Err(project_web_build_error(
            "Project Web build embedded manifest has a foreign or missing field",
        ));
    }
    let string = |key: &str| object.get(key).and_then(serde_json::Value::as_str);
    if string("schema") != Some("semaprax.web-project.v1")
        || string("project_schema") != Some("semaprax.project.v1")
        || string("project") != Some(project)
        || string("project_revision") != Some(project_revision)
        || string("workspace_revision") != Some(workspace_revision)
        || string("project_graph_digest") != Some(project_graph_digest)
        || string("entry_module") != Some(entry_module)
        || object
            .get("capabilities")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|capabilities| !capabilities.is_empty())
    {
        return Err(project_web_build_error(
            "Project Web build embedded manifest disagrees with carrier identity",
        ));
    }

    const MANIFEST_ARTIFACTS: [(&str, usize); 6] = [
        ("app.wasm", 0),
        ("index.html", 6),
        ("package.json", 5),
        ("semaprax.bindings.d.ts", 3),
        ("semaprax.bindings.js", 2),
        ("semaprax.js", 1),
    ];
    let manifest_artifacts = object
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .filter(|rows| rows.len() == MANIFEST_ARTIFACTS.len())
        .ok_or_else(|| {
            project_web_build_error("Project Web build embedded artifact inventory is invalid")
        })?;
    for (row, (expected_path, artifact_index)) in manifest_artifacts.iter().zip(MANIFEST_ARTIFACTS)
    {
        let row = row.as_object().ok_or_else(|| {
            project_web_build_error("Project Web build embedded artifact row is invalid")
        })?;
        if row.len() != 2
            || row.get("path").and_then(serde_json::Value::as_str) != Some(expected_path)
        {
            return Err(project_web_build_error(
                "Project Web build embedded artifact order or path is invalid",
            ));
        }
        let bytes = artifacts.get(artifact_index).ok_or_else(|| {
            project_web_build_error("Project Web build embedded artifact target is absent")
        })?;
        let digest = format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
        if row.get("sha256").and_then(serde_json::Value::as_str) != Some(digest.as_str()) {
            return Err(project_web_build_error(
                "Project Web build embedded artifact SHA-256 disagrees with decoded bytes",
            ));
        }
    }

    let scalar_abi = object
        .get("scalar_abi")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| project_web_build_error("Project Web build scalar ABI is invalid"))?;
    if scalar_abi.len() != 2
        || scalar_abi.get("schema").and_then(serde_json::Value::as_str)
            != Some("semaprax.wasm-scalar.v1")
    {
        return Err(project_web_build_error(
            "Project Web build scalar ABI schema is invalid",
        ));
    }
    let functions = scalar_abi
        .get("functions")
        .and_then(serde_json::Value::as_array)
        .filter(|functions| (1..=32).contains(&functions.len()))
        .ok_or_else(|| project_web_build_error("Project Web build scalar ABI functions invalid"))?;
    let mut previous_id: Option<&str> = None;
    for function in functions {
        let function = function.as_object().ok_or_else(|| {
            project_web_build_error("Project Web build scalar ABI function is invalid")
        })?;
        if function.len() != 4
            || ["stable_id", "wasm_export", "parameters", "result"]
                .iter()
                .any(|key| !function.contains_key(*key))
        {
            return Err(project_web_build_error(
                "Project Web build scalar ABI function is not closed",
            ));
        }
        let stable_id = function
            .get("stable_id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| {
                !id.is_empty()
                    && id.len() <= 128
                    && id.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
                    })
            })
            .ok_or_else(|| project_web_build_error("Project Web build scalar stable ID invalid"))?;
        if previous_id.is_some_and(|previous| previous.as_bytes() >= stable_id.as_bytes()) {
            return Err(project_web_build_error(
                "Project Web build scalar stable IDs are not canonical",
            ));
        }
        previous_id = Some(stable_id);
        let expected_symbol = scalar_exports::raw_symbol(stable_id);
        if function
            .get("wasm_export")
            .and_then(serde_json::Value::as_str)
            != Some(expected_symbol.as_str())
        {
            return Err(project_web_build_error(
                "Project Web build scalar export symbol disagrees with stable ID",
            ));
        }
        let parameters = function
            .get("parameters")
            .and_then(serde_json::Value::as_array)
            .filter(|parameters| parameters.len() <= 8)
            .ok_or_else(|| {
                project_web_build_error("Project Web build scalar parameters are invalid")
            })?;
        if parameters
            .iter()
            .any(|ty| !matches!(ty.as_str(), Some("i64" | "bool")))
            || !matches!(
                function.get("result").and_then(serde_json::Value::as_str),
                Some("i64" | "bool")
            )
        {
            return Err(project_web_build_error(
                "Project Web build scalar type is outside the closed ABI",
            ));
        }
    }
    let canonical_functions = functions
        .iter()
        .map(|function| {
            let function = function
                .as_object()
                .expect("scalar ABI function object was admitted above");
            let stable_id = function["stable_id"]
                .as_str()
                .expect("scalar stable ID was admitted above");
            let wasm_export = function["wasm_export"]
                .as_str()
                .expect("scalar export symbol was admitted above");
            let parameters = function["parameters"]
                .as_array()
                .expect("scalar parameters were admitted above")
                .iter()
                .map(|ty| quote_json(ty.as_str().expect("scalar type was admitted above")))
                .collect::<Vec<_>>()
                .join(",");
            let result = function["result"]
                .as_str()
                .expect("scalar result was admitted above");
            format!(
                "{{\"stable_id\":{},\"wasm_export\":{},\"parameters\":[{}],\"result\":{}}}",
                quote_json(stable_id),
                quote_json(wasm_export),
                parameters,
                quote_json(result),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let canonical_artifacts = MANIFEST_ARTIFACTS
        .iter()
        .map(|(path, artifact_index)| {
            format!(
                "{{\"path\":{},\"sha256\":\"{:x}\"}}",
                quote_json(path),
                crate::digest_hex::LowerHex(Sha256::digest(&artifacts[*artifact_index])),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let canonical_manifest = format!(
        "{{\"schema\":\"semaprax.web-project.v1\",\"project_schema\":\"semaprax.project.v1\",\"project\":{},\"project_revision\":{},\"workspace_revision\":{},\"project_graph_digest\":{},\"entry_module\":{},\"capabilities\":[],\"artifacts\":[{}],\"scalar_abi\":{{\"schema\":\"semaprax.wasm-scalar.v1\",\"functions\":[{}]}}}}\n",
        quote_json(project),
        quote_json(project_revision),
        quote_json(workspace_revision),
        quote_json(project_graph_digest),
        quote_json(entry_module),
        canonical_artifacts,
        canonical_functions,
    );
    if manifest_source != canonical_manifest {
        return Err(project_web_build_error(
            "Project Web build embedded manifest is not canonical exact replay",
        ));
    }
    Ok(())
}

fn decode_lower_hex(value: &str) -> Result<Vec<u8>, Diagnostic> {
    if value.len() & 1 == 1 {
        return Err(project_web_build_error(
            "Project Web build content_hex has odd length",
        ));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for index in (0..value.len()).step_by(2) {
        let pair = &value.as_bytes()[index..index + 2];
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        let high = digit(pair[0]).ok_or_else(|| {
            project_web_build_error("Project Web build content_hex is not lowercase hexadecimal")
        })?;
        let low = digit(pair[1]).ok_or_else(|| {
            project_web_build_error("Project Web build content_hex is not lowercase hexadecimal")
        })?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

pub(crate) fn prepare_project_web_with_scalar_exports(
    program: &ResolvedProgram,
    project_name: &str,
    project_revision: &str,
    workspace_revision: &str,
    project_graph_digest: &str,
    entry_module: &str,
    export_ids: &[String],
) -> Result<PreparedProjectWeb, Diagnostic> {
    let plans = scalar_exports::prepare(program, export_ids)?;
    let wasm_bytes = emit_resolved_module_internal(program, &plans, &[])?;
    let wasm_sha256 = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(&wasm_bytes))
    );
    let runtime = scalar_profile_runtime(&wasm_sha256);
    let bindings = scalar_bindings(&plans, &wasm_sha256);
    let declarations = scalar_declarations(&plans);
    let package = "{\"private\":true,\"type\":\"module\",\"exports\":\"./semaprax.bindings.js\",\"types\":\"./semaprax.bindings.d.ts\"}\n";
    let index = scalar_browser_html();
    let manifest_artifacts: [(&str, &[u8]); 5] = [
        ("index.html", index.as_bytes()),
        ("package.json", package.as_bytes()),
        ("semaprax.bindings.d.ts", declarations.as_bytes()),
        ("semaprax.bindings.js", bindings.as_bytes()),
        ("semaprax.js", runtime.as_bytes()),
    ];
    let manifest = scalar_project_manifest(
        ProjectWebIdentity {
            project_name,
            project_revision,
            workspace_revision,
            project_graph_digest,
            entry_module,
        },
        &plans,
        &wasm_sha256,
        &manifest_artifacts,
    );
    Ok(PreparedProjectWeb {
        wasm_bytes,
        runtime,
        bindings,
        declarations,
        manifest,
    })
}

fn publish_scalar_package(output: &Path, artifacts: &[(&str, &[u8])]) -> Result<(), Diagnostic> {
    output.file_name().ok_or_else(|| {
        Diagnostic::io(
            "SPX-I301",
            "Public Scalar Export package output must name one directory",
        )
    })?;
    let parent_path = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_metadata = fs::symlink_metadata(parent_path).map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!(
                "cannot inspect Public Scalar Export output parent {}: {error}",
                parent_path.display()
            ),
        )
    })?;
    if !is_plain_directory(&parent_metadata) {
        return Err(Diagnostic::io(
            "SPX-I301",
            "Public Scalar Export output parent must be a real non-reparse directory",
        ));
    }
    let parent_identity = Handle::from_path(parent_path).map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot identify Public Scalar Export output parent: {error}"),
        )
    })?;
    match fs::symlink_metadata(output) {
        Ok(_) => {
            return Err(Diagnostic::io(
                "SPX-I307",
                format!(
                    "Public Scalar Export package destination already exists: {}",
                    output.display()
                ),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(Diagnostic::io(
                "SPX-I301",
                format!(
                    "cannot inspect Public Scalar Export destination {}: {error}",
                    output.display()
                ),
            ));
        }
    }
    fs::create_dir(output).map_err(|error| {
        let code = if error.kind() == std::io::ErrorKind::AlreadyExists {
            "SPX-I307"
        } else {
            "SPX-I301"
        };
        Diagnostic::io(
            code,
            format!(
                "cannot create fresh Public Scalar Export destination {}: {error}",
                output.display()
            ),
        )
    })?;
    let output_metadata = fs::symlink_metadata(output).map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot inspect fresh Public Scalar Export destination: {error}"),
        )
    })?;
    if !is_plain_directory(&output_metadata) {
        return Err(Diagnostic::io(
            "SPX-I301",
            "fresh Public Scalar Export destination is not a real non-reparse directory",
        ));
    }
    let output_identity = Handle::from_path(output).map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot identify fresh Public Scalar Export destination: {error}"),
        )
    })?;
    if let Err(error) = write_and_authenticate_scalar_artifacts(output, artifacts) {
        cleanup_scalar_package(
            parent_path,
            &parent_identity,
            output,
            &output_identity,
            artifacts,
        );
        return Err(error);
    }
    if let Err(error) =
        authenticate_scalar_destination(parent_path, &parent_identity, output, &output_identity)
    {
        cleanup_scalar_package(
            parent_path,
            &parent_identity,
            output,
            &output_identity,
            artifacts,
        );
        return Err(error);
    }
    if let Err(error) = authenticate_scalar_artifacts(output, artifacts) {
        cleanup_scalar_package(
            parent_path,
            &parent_identity,
            output,
            &output_identity,
            artifacts,
        );
        return Err(error);
    }
    Ok(())
}

fn is_plain_directory(metadata: &fs::Metadata) -> bool {
    metadata.is_dir() && !metadata.file_type().is_symlink() && !metadata_is_reparse(metadata)
}

fn is_plain_regular_file(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && !metadata.file_type().is_symlink() && !metadata_is_reparse(metadata)
}

#[cfg(windows)]
fn metadata_is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse(_metadata: &fs::Metadata) -> bool {
    false
}

fn write_and_authenticate_scalar_artifacts(
    directory: &Path,
    artifacts: &[(&str, &[u8])],
) -> Result<(), Diagnostic> {
    for (name, bytes) in artifacts {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = options.open(directory.join(name)).map_err(|error| {
            Diagnostic::io(
                "SPX-I302",
                format!("cannot create Public Scalar Export artifact `{name}`: {error}"),
            )
        })?;
        file.write_all(bytes).map_err(|error| {
            Diagnostic::io(
                "SPX-I302",
                format!("cannot write Public Scalar Export artifact `{name}`: {error}"),
            )
        })?;
        file.sync_all().map_err(|error| {
            Diagnostic::io(
                "SPX-I302",
                format!("cannot sync Public Scalar Export artifact `{name}`: {error}"),
            )
        })?;
    }
    authenticate_scalar_artifacts(directory, artifacts)
}

fn authenticate_scalar_artifacts(
    directory: &Path,
    artifacts: &[(&str, &[u8])],
) -> Result<(), Diagnostic> {
    let mut observed = fs::read_dir(directory)
        .map_err(|error| {
            Diagnostic::io(
                "SPX-I302",
                format!("cannot enumerate Public Scalar Export package: {error}"),
            )
        })?
        .map(|entry| {
            let entry = entry.map_err(|error| {
                Diagnostic::io(
                    "SPX-I302",
                    format!("cannot inspect Public Scalar Export package entry: {error}"),
                )
            })?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(|error| {
                Diagnostic::io(
                    "SPX-I302",
                    format!("cannot inspect Public Scalar Export package entry type: {error}"),
                )
            })?;
            if !is_plain_regular_file(&metadata) {
                return Err(Diagnostic::io(
                    "SPX-I302",
                    "Public Scalar Export package contains a non-regular entry",
                ));
            }
            entry.file_name().into_string().map_err(|_| {
                Diagnostic::io(
                    "SPX-I302",
                    "Public Scalar Export package contains a non-Unicode entry",
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    observed.sort();
    let mut expected = artifacts
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect::<Vec<_>>();
    expected.sort();
    if observed != expected {
        return Err(Diagnostic::io(
            "SPX-I302",
            "Public Scalar Export package inventory changed during publication",
        ));
    }
    for (name, bytes) in artifacts {
        let path = directory.join(name);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            Diagnostic::io(
                "SPX-I302",
                format!("cannot authenticate Public Scalar Export artifact `{name}`: {error}"),
            )
        })?;
        if !is_plain_regular_file(&metadata) {
            return Err(Diagnostic::io(
                "SPX-I302",
                format!("Public Scalar Export artifact `{name}` is not a real regular file"),
            ));
        }
        if fs::read(&path).map_err(|error| {
            Diagnostic::io(
                "SPX-I302",
                format!("cannot authenticate Public Scalar Export artifact `{name}`: {error}"),
            )
        })? != *bytes
        {
            return Err(Diagnostic::io(
                "SPX-I302",
                format!("Public Scalar Export artifact `{name}` changed during publication"),
            ));
        }
    }
    Ok(())
}

fn authenticate_scalar_destination(
    parent_path: &Path,
    parent_identity: &Handle,
    output: &Path,
    output_identity: &Handle,
) -> Result<(), Diagnostic> {
    let parent_metadata = fs::symlink_metadata(parent_path).map_err(|error| {
        Diagnostic::io(
            "SPX-I302",
            format!("cannot recheck Public Scalar Export output parent: {error}"),
        )
    })?;
    let output_metadata = fs::symlink_metadata(output).map_err(|error| {
        Diagnostic::io(
            "SPX-I302",
            format!("cannot recheck Public Scalar Export destination: {error}"),
        )
    })?;
    if !is_plain_directory(&parent_metadata) || !is_plain_directory(&output_metadata) {
        return Err(Diagnostic::io(
            "SPX-I302",
            "Public Scalar Export parent or destination became a symlink/reparse object",
        ));
    }
    if *parent_identity
        != Handle::from_path(parent_path).map_err(|error| {
            Diagnostic::io(
                "SPX-I302",
                format!("cannot identify rebound Public Scalar Export parent: {error}"),
            )
        })?
        || *output_identity
            != Handle::from_path(output).map_err(|error| {
                Diagnostic::io(
                    "SPX-I302",
                    format!("cannot identify rebound Public Scalar Export destination: {error}"),
                )
            })?
    {
        return Err(Diagnostic::io(
            "SPX-I302",
            "Public Scalar Export destination identity changed during publication",
        ));
    }
    Ok(())
}

fn cleanup_scalar_package(
    parent_path: &Path,
    parent_identity: &Handle,
    output: &Path,
    output_identity: &Handle,
    artifacts: &[(&str, &[u8])],
) {
    let identities_match = Handle::from_path(parent_path)
        .ok()
        .is_some_and(|identity| identity == *parent_identity)
        && Handle::from_path(output)
            .ok()
            .is_some_and(|identity| identity == *output_identity);
    if !identities_match {
        return;
    }
    for (name, expected) in artifacts {
        let path = output.join(name);
        let removable = fs::symlink_metadata(&path)
            .ok()
            .is_some_and(|metadata| is_plain_regular_file(&metadata))
            && fs::read(&path)
                .ok()
                .is_some_and(|observed| observed == *expected);
        if removable {
            let _ = fs::remove_file(path);
        }
    }
    let _ = fs::remove_dir(output);
}

fn scalar_profile_runtime(wasm_sha256: &str) -> String {
    browser_runtime()
        .replace("__SEMAPRAX_OWNED_EXPORTS__", "Object.freeze({})")
        .replace("__SEMAPRAX_WASM_SHA256__", wasm_sha256)
        .replace(
            &format!("const SPX_WASM_SHA256 = \"{wasm_sha256}\";"),
            &format!(
                "export const wasmSha256 = \"{wasm_sha256}\";\nconst SPX_WASM_SHA256 = wasmSha256;"
            ),
        )
        .replace("export const imports =", "const imports =")
        .replace(
            "const SPX_RUNTIME_TAG_ALLOCATOR_KEY =",
            "class SpxSemanticFailure extends Error {\n  constructor(domainId, code) { super(\"SEMAPRAX semantic failure\"); this.domainId = domainId; this.code = code; }\n}\nexport function semanticStatus(error) {\n  return error instanceof SpxSemanticFailure\n    ? Object.freeze({ schema: \"semaprax.status.v1\", domain_id: error.domainId, code: error.code })\n    : null;\n}\nconst SPX_RUNTIME_TAG_ALLOCATOR_KEY =",
        )
        .replace(
            "throw new RangeError(`SEMAPRAX checked arithmetic failure: ${operation}`);",
            "throw new SpxSemanticFailure(\"semaprax.arithmetic.v1\", ({ \"addition overflow\": 1, \"subtraction overflow\": 2, \"multiplication overflow\": 3, \"negation overflow\": 8 })[operation]);",
        )
        .replace(
            "if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError(\"SEMAPRAX checked arithmetic failure: invalid division\");",
            "if (b === 0n) throw new SpxSemanticFailure(\"semaprax.arithmetic.v1\", 4);\n      if (a === SPX_MIN && b === -1n) throw new SpxSemanticFailure(\"semaprax.arithmetic.v1\", 5);",
        )
        .replace(
            "if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError(\"SEMAPRAX checked arithmetic failure: invalid remainder\");",
            "if (b === 0n) throw new SpxSemanticFailure(\"semaprax.arithmetic.v1\", 6);\n      if (a === SPX_MIN && b === -1n) throw new SpxSemanticFailure(\"semaprax.arithmetic.v1\", 7);",
        )
        .replace(
            "spx_contract_fail: () => { throw new Error(\"SEMAPRAX contract failure\"); },",
            "spx_contract_fail: code => { throw new SpxSemanticFailure(\"semaprax.contract.v1\", code); },",
        )
}

fn scalar_bindings(plans: &[scalar_exports::ScalarExportPlan], wasm_sha256: &str) -> String {
    let facts = plans
        .iter()
        .map(|plan| {
            format!(
                "[{},Object.freeze({{raw:{},params:Object.freeze([{}]),result:{}}})]",
                quote_json(&plan.stable_id),
                quote_json(&plan.wasm_export),
                plan.params
                    .iter()
                    .map(|ty| quote_json(ty.text()))
                    .collect::<Vec<_>>()
                    .join(","),
                quote_json(plan.result.text()),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"import {{ instantiateBytes as instantiateRuntimeBytes, semanticStatus, wasmSha256 as runtimeWasmSha256 }} from "./semaprax.js";
const SPX_MIN = -(1n << 63n);
const SPX_MAX = (1n << 63n) - 1n;
const EXPECTED_WASM_SHA256 = "{wasm_sha256}";
if (runtimeWasmSha256 !== EXPECTED_WASM_SHA256) throw new Error("SEMAPRAX scalar binding/runtime digest disagreement");
const ENTRIES = Object.freeze([{facts}]);
const EXPORT_IDS = Object.freeze(ENTRIES.map(([id]) => id));
const FACTS = Object.create(null);
for (const [id, fact] of ENTRIES) Object.defineProperty(FACTS, id, {{ value: fact, enumerable: true }});
Object.freeze(FACTS);
function argument(value, type, index) {{
  if (type === "i64") {{
    if (typeof value !== "bigint" || value < SPX_MIN || value > SPX_MAX) throw new TypeError(`argument ${{index}} must be a signed 64-bit bigint`);
    return value;
  }}
  if (typeof value !== "boolean") throw new TypeError(`argument ${{index}} must be boolean`);
  return value ? 1 : 0;
}}
function result(value, type) {{
  if (type === "i64") {{
    if (typeof value !== "bigint" || value < SPX_MIN || value > SPX_MAX) throw new TypeError("SEMAPRAX adapter returned invalid i64");
    return value;
  }}
  if (value !== 0 && value !== 1) throw new TypeError("SEMAPRAX adapter returned non-canonical bool");
  return value === 1;
}}
function invoke(instance, id, values) {{
  const fact = FACTS[id];
  if (fact === undefined) throw new RangeError(`unknown SEMAPRAX scalar export: ${{id}}`);
  if (values.length !== fact.params.length) throw new TypeError(`SEMAPRAX scalar export ${{id}} expects ${{fact.params.length}} arguments`);
  const raw = instance.exports[fact.raw];
  if (typeof raw !== "function") throw new Error(`SEMAPRAX scalar adapter missing: ${{fact.raw}}`);
  try {{ return Object.freeze({{ ok: true, value: result(raw(...values.map((value, index) => argument(value, fact.params[index], index))), fact.result) }}); }}
  catch (error) {{
    const status = semanticStatus(error);
    if (status !== null) return Object.freeze({{ ok: false, status }});
    throw error;
  }}
}}
function facade(instance) {{
  const functions = Object.create(null);
  for (const id of EXPORT_IDS) Object.defineProperty(functions, id, {{ value: (...values) => invoke(instance, id, values), enumerable: true }});
  return Object.freeze({{ functions: Object.freeze(functions), call: (id, ...values) => invoke(instance, id, values) }});
}}
export async function instantiateBytes(bytes) {{ const linked = await instantiateRuntimeBytes(bytes); return facade(linked.instance); }}
export async function instantiate(url = new URL("./app.wasm", import.meta.url)) {{ const response = await fetch(url); return instantiateBytes(await response.arrayBuffer()); }}
export const exportIds = EXPORT_IDS;
"#
    )
}

fn scalar_declarations(plans: &[scalar_exports::ScalarExportPlan]) -> String {
    let properties = plans
        .iter()
        .map(|plan| {
            let args = plan
                .params
                .iter()
                .enumerate()
                .map(|(index, ty)| format!("arg{index}: {}", ty.typescript_type()))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "  readonly {}: ({args}) => ScalarResult<{}>;",
                quote_json(&plan.stable_id),
                plan.result.typescript_type(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "export type ScalarStatus = Readonly<{{ schema: \"semaprax.status.v1\"; domain_id: \"semaprax.arithmetic.v1\"; code: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 }}> | Readonly<{{ schema: \"semaprax.status.v1\"; domain_id: \"semaprax.contract.v1\"; code: 1 | 2 }}>;\nexport type ScalarResult<T> = Readonly<{{ ok: true; value: T }}> | Readonly<{{ ok: false; status: ScalarStatus }}>;\nexport interface ScalarFunctions {{\n{properties}\n}}\nexport interface ScalarRuntime {{ readonly functions: Readonly<ScalarFunctions>; call<I extends keyof ScalarFunctions>(id: I, ...args: Parameters<ScalarFunctions[I]>): ReturnType<ScalarFunctions[I]>; }}\nexport declare function instantiateBytes(bytes: ArrayBuffer | ArrayBufferView): Promise<ScalarRuntime>;\nexport declare function instantiate(url?: URL | string): Promise<ScalarRuntime>;\nexport declare const exportIds: readonly (keyof ScalarFunctions)[];\n"
    )
}

fn scalar_manifest(
    program: &Program,
    plans: &[scalar_exports::ScalarExportPlan],
    wasm_sha256: &str,
    artifacts: &[(&str, &[u8])],
) -> String {
    let functions = plans
        .iter()
        .map(|plan| plan.manifest_json())
        .collect::<Vec<_>>()
        .join(",");
    let mut artifact_rows = vec![format!(
        "{{\"path\":\"app.wasm\",\"sha256\":\"{wasm_sha256}\"}}"
    )];
    artifact_rows.extend(artifacts.iter().map(|(path, bytes)| {
        format!(
            "{{\"path\":{},\"sha256\":\"{:x}\"}}",
            quote_json(path),
            crate::digest_hex::LowerHex(Sha256::digest(bytes))
        )
    }));
    format!(
        "{{\"schema\":\"semaprax.web.v4\",\"module\":{},\"graph_revision\":{},\"capabilities\":[],\"artifacts\":[{}],\"scalar_abi\":{{\"schema\":\"semaprax.wasm-scalar.v1\",\"functions\":[{}]}}}}\n",
        quote_json(&program.module),
        quote_json(&graph::revision(program)),
        artifact_rows.join(","),
        functions,
    )
}

fn scalar_project_manifest(
    identity: ProjectWebIdentity<'_>,
    plans: &[scalar_exports::ScalarExportPlan],
    wasm_sha256: &str,
    artifacts: &[(&str, &[u8])],
) -> String {
    let functions = plans
        .iter()
        .map(|plan| plan.manifest_json())
        .collect::<Vec<_>>()
        .join(",");
    let mut artifact_rows = vec![format!(
        "{{\"path\":\"app.wasm\",\"sha256\":\"{wasm_sha256}\"}}"
    )];
    artifact_rows.extend(artifacts.iter().map(|(path, bytes)| {
        format!(
            "{{\"path\":{},\"sha256\":\"{:x}\"}}",
            quote_json(path),
            crate::digest_hex::LowerHex(Sha256::digest(bytes))
        )
    }));
    format!(
        "{{\"schema\":\"semaprax.web-project.v1\",\"project_schema\":\"semaprax.project.v1\",\"project\":{},\"project_revision\":{},\"workspace_revision\":{},\"project_graph_digest\":{},\"entry_module\":{},\"capabilities\":[],\"artifacts\":[{}],\"scalar_abi\":{{\"schema\":\"semaprax.wasm-scalar.v1\",\"functions\":[{}]}}}}\n",
        quote_json(identity.project_name),
        quote_json(identity.project_revision),
        quote_json(identity.workspace_revision),
        quote_json(identity.project_graph_digest),
        quote_json(identity.entry_module),
        artifact_rows.join(","),
        functions,
    )
}

fn scalar_browser_html() -> &'static str {
    r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>SEMAPRAX scalar package</title></head><body><p>Import <code>./semaprax.bindings.js</code> to use this package.</p></body></html>
"#
}

fn reject_native_rust_imports(program: &Program) -> Result<(), Diagnostic> {
    if program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .any(|import| import.native_rust)
    {
        Err(Diagnostic::io(
            "SPX-W114",
            "Native Rust imports are unavailable for WebAssembly targets",
        ))
    } else {
        Ok(())
    }
}

fn collect_locals(
    expr: &ResolvedExpr,
    parameter_count: u32,
    layout: &mut LocalLayout,
) -> Result<(), Diagnostic> {
    match &expr.kind {
        ResolvedExprKind::Call { args, .. } => {
            for arg in args {
                collect_locals(arg, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for arg in &call.args {
                collect_locals(arg, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for arg in &call.args {
                collect_locals(arg, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            collect_locals(source, parameter_count, layout)?;
            collect_locals(start, parameter_count, layout)?;
            collect_locals(end, parameter_count, layout)?;
        }
        ResolvedExprKind::Unary { value, .. } => {
            collect_locals(value, parameter_count, layout)?;
        }
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
            collect_locals(operand, parameter_count, layout)?;
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_locals(left, parameter_count, layout)?;
            collect_locals(right, parameter_count, layout)?;
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { binding, value, .. } => {
                        collect_locals(value, parameter_count, layout)?;
                        let index = parameter_count + layout.declarations.len() as u32;
                        layout.declarations.push(binding.ty.clone());
                        if layout.lets.insert(binding.id.clone(), index).is_some() {
                            return Err(Diagnostic::io(
                                "SPX-W108",
                                format!("duplicate WebAssembly local identity `{}`", binding.id),
                            ));
                        }
                    }
                    // An assignment target reuses its `let` local and an
                    // unsafe boundary adds none; only their values contribute
                    // to the local walk.
                    ResolvedStatement::Assign { value, .. } => {
                        collect_locals(value, parameter_count, layout)?;
                    }
                    ResolvedStatement::Unsafe { body, .. } => {
                        collect_locals(body, parameter_count, layout)?;
                    }
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        collect_locals(condition, parameter_count, layout)?;
                        collect_locals(body, parameter_count, layout)?;
                    }
                }
            }
            collect_locals(tail, parameter_count, layout)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_locals(condition, parameter_count, layout)?;
            collect_locals(then_branch, parameter_count, layout)?;
            collect_locals(else_branch, parameter_count, layout)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for field in fields {
                collect_locals(&field.value, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                collect_locals(&field.value, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_locals(scrutinee, parameter_count, layout)?;
            if matches!(
                scrutinee.ty,
                ResolvedType::I64
                    | ResolvedType::I32
                    | ResolvedType::U8
                    | ResolvedType::Char
                    | ResolvedType::Bool
            ) {
                // Refutable Match v1: stage the scrutinee once in its own
                // dedicated local so every arm test re-reads exactly one
                // evaluation.
                let index = parameter_count + layout.declarations.len() as u32;
                layout.declarations.push(scrutinee.ty.clone());
                if layout
                    .match_scratch
                    .insert(expr.id.as_str().to_owned(), index)
                    .is_some()
                {
                    return Err(Diagnostic::io(
                        "SPX-W108",
                        format!(
                            "duplicate WebAssembly local identity for match `{}`",
                            expr.id
                        ),
                    ));
                }
                // Binding arms alias the staged scrutinee local: reading the
                // binding reads exactly one evaluation of the scrutinee.
                for arm in arms {
                    if let crate::hir::ResolvedMatchPattern::Binding(binding) = &arm.pattern {
                        layout.lets.insert(binding.id.clone(), index);
                    }
                }
            }
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_locals(guard.as_ref(), parameter_count, layout)?;
                }
                collect_locals(&arm.value, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::Project { base, .. } => {
            collect_locals(base, parameter_count, layout)?;
        }
        ResolvedExprKind::Upcast { source } => {
            collect_locals(source, parameter_count, layout)?;
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_locals(base, parameter_count, layout)?;
            for field in fields {
                collect_locals(&field.value, parameter_count, layout)?;
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

fn emit_expr(
    output: &mut impl ByteOutput,
    expr: &ResolvedExpr,
    value_indexes: &HashMap<ValueId, u32>,
    function_indexes: &HashMap<FunctionExecutionId, u32>,
    layout: &LocalLayout,
    result: Option<(u32, &str)>,
) -> Result<(), Diagnostic> {
    match &expr.kind {
        ResolvedExprKind::Int(value) => {
            output.push(0x42);
            write_i64(output, *value);
        }
        ResolvedExprKind::Int32(value) => {
            output.push(0x41);
            write_i64(output, i64::from(*value));
        }
        ResolvedExprKind::Char(value) => {
            // Chars ride the i32 valtype; scalar values are below 2^31 so the
            // signed LEB128 encoding is exact.
            output.push(0x41);
            write_i64(output, i64::from(*value));
        }
        ResolvedExprKind::Uint8(value) => {
            // u8 values ride the i32 valtype with the same exact encoding.
            output.push(0x41);
            write_i64(output, i64::from(*value));
        }
        ResolvedExprKind::Usize(value) => {
            output.push(0x42);
            write_i64(output, *value as i64);
        }
        ResolvedExprKind::Float32(bits) => {
            output.push(0x43);
            output.extend_bytes(&bits.to_le_bytes());
        }
        ResolvedExprKind::Float64(bits) => {
            output.push(0x44);
            output.extend_bytes(&bits.to_le_bytes());
        }
        ResolvedExprKind::Bool(value) => {
            output.push(0x41);
            write_i64(output, i64::from(*value));
        }
        ResolvedExprKind::String(value) => {
            // A string value is an abstract host handle riding the i64 lane;
            // the handle is freshly allocated for this evaluation.
            let Some(table) = layout.string_data else {
                return Err(Diagnostic::io(
                    "SPX-W116",
                    "string literal reached lowering without string admission",
                ));
            };
            let Some(&offset) = table.offsets.get(value) else {
                return Err(Diagnostic::io(
                    "SPX-W116",
                    "string literal has no data-segment offset",
                ));
            };
            output.push(0x41); // i32.const offset
            write_u32(output, offset);
            output.push(0x41); // i32.const len
            write_i64(output, value.len() as i64);
            call_import(output, STRING_IMPORT_BASE_NEW);
        }
        ResolvedExprKind::Place(place) => {
            if !place.projections.is_empty() {
                return Err(Diagnostic::io(
                    "SPX-W110",
                    "aggregate place projections are not supported by the Wasm core backend",
                ));
            }
            let index = value_indexes.get(&place.root).copied().or_else(|| {
                result.and_then(|(index, result_id)| {
                    (place.root.as_str() == result_id).then_some(index)
                })
            });
            let index = index.ok_or_else(|| {
                Diagnostic::io(
                    "SPX-W103",
                    format!("unknown value identity `{}`", place.root),
                )
            })?;
            output.push(0x20);
            write_u32(output, index);
            // Every read of an owned string place clones the handle so the
            // source place keeps its unique owner.
            if matches!(expr.ty, ResolvedType::String) {
                call_import(output, STRING_IMPORT_BASE_CLONE);
            }
        }
        ResolvedExprKind::Call {
            callee,
            instance,
            args,
            ..
        } => {
            if instance.is_none() {
                if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
                    let helpers = layout.text_intrinsics.ok_or_else(|| {
                        Diagnostic::io(
                            "SPX-W119",
                            "borrowed `str` operation reached a non-text Wasm profile",
                        )
                    })?;
                    for arg in args {
                        emit_expr(output, arg, value_indexes, function_indexes, layout, result)?;
                    }
                    match op {
                        crate::str_ops::StrOp::LenBytes => {
                            output.push(0x42); // i64.const 32
                            write_i64(output, 32);
                            output.push(0x88); // i64.shr_u
                        }
                        crate::str_ops::StrOp::IsEmpty => {
                            output.push(0x42);
                            write_i64(output, 32);
                            output.push(0x88); // i64.shr_u
                            output.push(0x50); // i64.eqz
                        }
                        crate::str_ops::StrOp::StartsWith => {
                            output.push(0x10);
                            write_u32(output, helpers.starts_with);
                        }
                        crate::str_ops::StrOp::Contains => {
                            output.push(0x10);
                            write_u32(output, helpers.contains);
                        }
                    }
                    return Ok(());
                }
                if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                    // Compiler-owned string operations lower through their
                    // dedicated host imports; borrowed reads leave the input
                    // handle owned by the caller and concatenation hands both
                    // input handles to the host shim.
                    for arg in args {
                        emit_expr(output, arg, value_indexes, function_indexes, layout, result)?;
                    }
                    match op {
                        crate::string_ops::StringOp::Len => {
                            call_import(output, STRING_OPS_IMPORT_BASE_LEN);
                        }
                        crate::string_ops::StringOp::IsEmpty => {
                            call_import(output, STRING_OPS_IMPORT_BASE_LEN);
                            output.push(0x50); // i64.eqz keeps bool results exact
                        }
                        crate::string_ops::StringOp::Concat => {
                            call_import(output, STRING_OPS_IMPORT_BASE_CONCAT);
                        }
                        crate::string_ops::StringOp::StartsWith => {
                            call_import(output, layout.string_ops_v2_base);
                        }
                        crate::string_ops::StringOp::Contains => {
                            call_import(output, layout.string_ops_v2_base + 1);
                        }
                        crate::string_ops::StringOp::LenChars => {
                            call_import(output, layout.string_ops_v2_base + 2);
                        }
                        crate::string_ops::StringOp::FromChar => {
                            call_import(output, layout.string_ops_v2_base + 3);
                        }
                    }
                    return Ok(());
                }
            }
            for arg in args {
                emit_expr(output, arg, value_indexes, function_indexes, layout, result)?;
            }
            output.push(0x10);
            let execution = instance.as_ref().map_or_else(
                || FunctionExecutionId::Monomorphic(callee.clone()),
                |instance| FunctionExecutionId::Generic(instance.clone()),
            );
            write_u32(
                output,
                *function_indexes.get(&execution).ok_or_else(|| {
                    Diagnostic::io("SPX-W104", format!("unknown function identity `{callee}`"))
                })?,
            );
        }
        ResolvedExprKind::NativeRustImportCall(_) => {
            return Err(Diagnostic::io(
                "SPX-W114",
                "Native Rust imports are unavailable for WebAssembly targets",
            ));
        }
        ResolvedExprKind::HostCommandCall(_) => {
            return Err(Diagnostic::io(
                "SPX-W114",
                "command I/O operations require the Language Command I/O v1 WebAssembly lane",
            ));
        }
        ResolvedExprKind::Unary { op, value } => match op {
            UnaryOp::Neg => {
                if value.ty == ResolvedType::I32 {
                    emit_expr(
                        output,
                        value,
                        value_indexes,
                        function_indexes,
                        layout,
                        result,
                    )?;
                    let [wide, _] = layout.wide_scratch;
                    output.push(0xac);
                    local_set(output, wide);
                    local_get(output, wide);
                    output.push(0xa7);
                    output.push(0x41);
                    write_i64(output, i32::MIN as i64);
                    output.push(0x46);
                    emit_unreachable_trap(output);
                    output.push(0x42);
                    write_i64(output, 0);
                    local_get(output, wide);
                    output.push(0x7d);
                    output.push(0xa7);
                    return Ok(());
                }
                emit_expr(
                    output,
                    value,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                )?;
                if matches!(value.ty, ResolvedType::F32 | ResolvedType::F64) {
                    // IEEE-754 negation is total; no failure import is used.
                    output.push(match value.ty {
                        ResolvedType::F32 => 0x8c,
                        _ => 0x9a,
                    });
                } else {
                    output.push(0x10);
                    write_u32(output, 5);
                }
            }
            UnaryOp::Not => {
                emit_expr(
                    output,
                    value,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                )?;
                output.push(0x45);
            }
        },
        ResolvedExprKind::Binary { op, left, right } => {
            if matches!(left.ty, ResolvedType::String) {
                // Owned strings compare by UTF-8 contents through the host
                // shim; every other operator over strings is ill-typed.
                if !matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
                    return Err(Diagnostic::io(
                        "SPX-W116",
                        "string operands only support equality comparison",
                    ));
                }
                emit_expr(
                    output,
                    left,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                )?;
                emit_expr(
                    output,
                    right,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                )?;
                call_import(output, STRING_IMPORT_BASE_EQ);
                if *op == BinaryOp::Ne {
                    output.push(0x45); // i32.eqz keeps bool results exact
                }
                return Ok(());
            }
            if matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            ) && left.ty == ResolvedType::I32
            {
                emit_i32_checked_binary(
                    output,
                    *op,
                    left,
                    right,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                )?;
                return Ok(());
            }
            emit_expr(
                output,
                left,
                value_indexes,
                function_indexes,
                layout,
                result,
            )?;
            if matches!(op, BinaryOp::And | BinaryOp::Or) {
                emit_short_circuit(
                    output,
                    *op,
                    right,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                )?;
                return Ok(());
            }
            emit_expr(
                output,
                right,
                value_indexes,
                function_indexes,
                layout,
                result,
            )?;
            if matches!(left.ty, ResolvedType::F32 | ResolvedType::F64) {
                // IEEE-754 arithmetic is total and never selects a failure
                // status, so floats use native opcodes instead of the checked
                // scalar imports.
                let wide = matches!(left.ty, ResolvedType::F64);
                output.push(match (op, wide) {
                    (BinaryOp::Add, true) => 0xa0,
                    (BinaryOp::Sub, true) => 0xa1,
                    (BinaryOp::Mul, true) => 0xa2,
                    (BinaryOp::Div, true) => 0xa3,
                    (BinaryOp::Add, false) => 0x92,
                    (BinaryOp::Sub, false) => 0x93,
                    (BinaryOp::Mul, false) => 0x94,
                    (BinaryOp::Div, false) => 0x95,
                    (BinaryOp::Eq, true) => 0x61,
                    (BinaryOp::Ne, true) => 0x62,
                    (BinaryOp::Lt, true) => 0x63,
                    (BinaryOp::Gt, true) => 0x64,
                    (BinaryOp::Le, true) => 0x65,
                    (BinaryOp::Ge, true) => 0x66,
                    (BinaryOp::Eq, false) => 0x5b,
                    (BinaryOp::Ne, false) => 0x5c,
                    (BinaryOp::Lt, false) => 0x5d,
                    (BinaryOp::Gt, false) => 0x5e,
                    (BinaryOp::Le, false) => 0x5f,
                    (BinaryOp::Ge, false) => 0x60,
                    (BinaryOp::Rem, _) => {
                        return Err(Diagnostic::io(
                            "SPX-W102",
                            "floating-point remainder has no admitted Wasm lowering",
                        ));
                    }
                    _ => unreachable!("float arithmetic operation was matched above"),
                });
                return Ok(());
            }
            if matches!(left.ty, ResolvedType::U8)
                && matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
                )
            {
                // Checked u8 arithmetic without new host imports: bounded
                // operands make the unsigned i32 opcodes exact, and one
                // unsigned range check traps on any out-of-range result. The
                // scratch locals keep live values off the stack while the
                // polymorphic trap block executes.
                let Some((left_scratch, right_scratch)) = layout.u8_scratch else {
                    return Err(Diagnostic::io(
                        "SPX-W108",
                        "missing WebAssembly local layout for checked u8 arithmetic",
                    ));
                };
                // The stack holds [left, right]; pop them in reverse so each
                // scratch local keeps its operand.
                output.push(0x21);
                write_u32(output, right_scratch);
                output.push(0x21);
                write_u32(output, left_scratch);
                if matches!(op, BinaryOp::Div | BinaryOp::Rem) {
                    output.push(0x20);
                    write_u32(output, right_scratch);
                    output.push(0x45);
                    emit_failure_trap(output);
                    output.push(0x20);
                    write_u32(output, left_scratch);
                    output.push(0x20);
                    write_u32(output, right_scratch);
                    output.push(if *op == BinaryOp::Div { 0x6e } else { 0x70 });
                    return Ok(());
                }
                output.push(0x20);
                write_u32(output, left_scratch);
                output.push(0x20);
                write_u32(output, right_scratch);
                output.push(match op {
                    BinaryOp::Add => 0x6a,
                    BinaryOp::Sub => 0x6b,
                    _ => 0x6c,
                });
                output.push(0x21);
                write_u32(output, left_scratch);
                output.push(0x20);
                write_u32(output, left_scratch);
                output.push(0x41);
                write_i64(output, 255);
                output.push(0x4b);
                emit_failure_trap(output);
                output.push(0x20);
                write_u32(output, left_scratch);
                return Ok(());
            }
            if matches!(left.ty, ResolvedType::U8) && matches!(op, BinaryOp::Rem) {
                return Err(Diagnostic::io(
                    "SPX-W102",
                    "u8 remainder has no admitted Wasm lowering",
                ));
            }
            if matches!(left.ty, ResolvedType::Usize)
                && matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                )
            {
                let Some((left_scratch, right_scratch)) = layout.usize_scratch else {
                    return Err(Diagnostic::io(
                        "SPX-W108",
                        "missing WebAssembly local layout for checked usize arithmetic",
                    ));
                };
                output.push(0x21);
                write_u32(output, right_scratch);
                output.push(0x21);
                write_u32(output, left_scratch);
                match op {
                    BinaryOp::Add => {
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x7c);
                        output.push(0x22);
                        write_u32(output, right_scratch);
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x54); // i64.lt_u(result, left)
                        emit_failure_trap(output);
                        output.push(0x20);
                        write_u32(output, right_scratch);
                    }
                    BinaryOp::Sub => {
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x54); // i64.lt_u(left, right)
                        emit_failure_trap(output);
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x7d);
                    }
                    BinaryOp::Mul => {
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x50); // i64.eqz
                        output.push(0x45); // i32.eqz => right != 0
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x42);
                        write_i64(output, -1); // UINT64_MAX bit pattern
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x80); // i64.div_u
                        output.push(0x56); // i64.gt_u
                        output.push(0x71); // i32.and
                        emit_failure_trap(output);
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x7e);
                    }
                    BinaryOp::Div | BinaryOp::Rem => {
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(0x50);
                        emit_failure_trap(output);
                        output.push(0x20);
                        write_u32(output, left_scratch);
                        output.push(0x20);
                        write_u32(output, right_scratch);
                        output.push(if *op == BinaryOp::Div { 0x80 } else { 0x82 });
                    }
                    _ => unreachable!("usize arithmetic operation was matched above"),
                }
                return Ok(());
            }
            if matches!(left.ty, ResolvedType::Char)
                && matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                )
            {
                return Err(Diagnostic::io(
                    "SPX-W102",
                    "char arithmetic has no admitted Wasm lowering",
                ));
            }
            match op {
                BinaryOp::Add => call_import(output, 0),
                BinaryOp::Sub => call_import(output, 1),
                BinaryOp::Mul => call_import(output, 2),
                BinaryOp::Div => call_import(output, 3),
                BinaryOp::Rem => call_import(output, 4),
                BinaryOp::Eq | BinaryOp::Ne => {
                    output.push(match (&left.ty, op) {
                        (ResolvedType::I64 | ResolvedType::Usize, BinaryOp::Eq) => 0x51,
                        (ResolvedType::I64 | ResolvedType::Usize, BinaryOp::Ne) => 0x52,
                        (_, BinaryOp::Eq) => 0x46,
                        (_, BinaryOp::Ne) => 0x47,
                        _ => unreachable!(),
                    });
                }
                BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
                    // Ordered comparison compares scalar values; chars ride
                    // the unsigned i32 opcodes while i64 keeps its lane.
                    output.push(match (&left.ty, op) {
                        (ResolvedType::I32, BinaryOp::Lt) => 0x48,
                        (ResolvedType::I32, BinaryOp::Gt) => 0x4a,
                        (ResolvedType::I32, BinaryOp::Le) => 0x4c,
                        (ResolvedType::I32, BinaryOp::Ge) => 0x4e,
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
                        (_, BinaryOp::Lt) => 0x53,
                        (_, BinaryOp::Gt) => 0x55,
                        (_, BinaryOp::Le) => 0x57,
                        (_, BinaryOp::Ge) => 0x59,
                        _ => unreachable!("ordered comparison was matched above"),
                    });
                }
                BinaryOp::And | BinaryOp::Or => unreachable!(),
            }
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { binding, value, .. } => {
                        emit_expr(
                            output,
                            value,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                        )?;
                        let index = layout.lets.get(&binding.id).ok_or_else(|| {
                            Diagnostic::io(
                                "SPX-W108",
                                format!("missing WebAssembly local layout for `{}`", binding.id),
                            )
                        })?;
                        output.push(0x21);
                        write_u32(output, *index);
                    }
                    // The assigned value is emitted fully, then stored into
                    // the target's local with `local.set`.
                    ResolvedStatement::Assign {
                        binding,
                        value: assigned,
                        ..
                    } => {
                        emit_expr(
                            output,
                            assigned,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                        )?;
                        let index = layout.lets.get(&binding.id).ok_or_else(|| {
                            Diagnostic::io(
                                "SPX-W108",
                                format!(
                                    "missing WebAssembly local layout for assignment target `{}`",
                                    binding.id
                                ),
                            )
                        })?;
                        output.push(0x21);
                        write_u32(output, *index);
                    }
                    // Unsafe boundaries are transparent: the ordinary body is
                    // emitted exactly as-is and its scalar Copy result drops.
                    ResolvedStatement::Unsafe { body, .. } => {
                        emit_expr(
                            output,
                            body,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                        )?;
                        output.push(0x1A);
                    }
                    // Bounded While-Loops v1 lowers to a core `block`/`loop`
                    // pair: the condition re-evaluates at the top, a false
                    // condition branches out of the enclosing block, and the
                    // discarded body value falls through to the back-edge
                    // branch. Checked-arithmetic failures inside the loop use
                    // the same host imports/traps as straight-line code.
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        debug_assert!(matches!(condition.ty, ResolvedType::Bool));
                        output.extend_bytes(&[0x02, 0x40]); // block (empty)
                        output.extend_bytes(&[0x03, 0x40]); // loop (empty) $top
                        emit_expr(
                            output,
                            condition,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                        )?;
                        output.push(0x45); // i32.eqz
                        output.extend_bytes(&[0x0d, 0x01]); // br_if 1 -> $exit on false
                        emit_expr(
                            output,
                            body,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                        )?;
                        output.push(0x1A); // drop body value
                        output.extend_bytes(&[0x0c, 0x00]); // br 0 -> $top
                        output.push(0x0b); // end loop
                        output.push(0x0b); // end block
                    }
                }
            }
            emit_expr(
                output,
                tail,
                value_indexes,
                function_indexes,
                layout,
                result,
            )?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            emit_expr(
                output,
                condition,
                value_indexes,
                function_indexes,
                layout,
                result,
            )?;
            output.push(0x04);
            output.push(wasm_type(&then_branch.ty)?);
            emit_expr(
                output,
                then_branch,
                value_indexes,
                function_indexes,
                layout,
                result,
            )?;
            output.push(0x05);
            emit_expr(
                output,
                else_branch,
                value_indexes,
                function_indexes,
                layout,
                result,
            )?;
            output.push(0x0b);
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            // Refutable Match v1: Copy-scalar scrutinees lower to the
            // literal/guard decision chain on the core lane; aggregates keep
            // the aggregate-lane rejection below.
            if matches!(
                scrutinee.ty,
                ResolvedType::I64
                    | ResolvedType::I32
                    | ResolvedType::U8
                    | ResolvedType::Char
                    | ResolvedType::Bool
            ) {
                return emit_scalar_refutable_match(
                    output,
                    expr,
                    scrutinee,
                    arms,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                );
            }
            return Err(Diagnostic::io(
                "SPX-W110",
                "aggregate expressions require WebAssembly aggregate lowering",
            ));
        }
        ResolvedExprKind::ConstructRecord { .. }
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::BorrowPlace { .. }
        | ResolvedExprKind::ByteRange { .. }
        | ResolvedExprKind::ConstructVariant { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::TryOption { .. }
        | ResolvedExprKind::Project { .. }
        | ResolvedExprKind::Upcast { .. }
        | ResolvedExprKind::UpdateRecord { .. } => {
            return Err(Diagnostic::io(
                "SPX-W110",
                "aggregate expressions require WebAssembly aggregate lowering",
            ));
        }
    }
    Ok(())
}

/// Refutable Match v1 core-lane lowering. The scrutinee evaluates once into
/// its dedicated staging local; every non-final arm nests one reject block
/// whose `br_if 0` falls through to the following arms, and a selected arm's
/// value branches out of the result-carrying `$done` block. A guard is an
/// ordinary emitted bool expression short-circuited after the pattern test,
/// so it evaluates exactly once per reached matching arm.
#[allow(clippy::too_many_arguments)]
fn emit_scalar_refutable_match(
    output: &mut impl ByteOutput,
    expr: &ResolvedExpr,
    scrutinee: &ResolvedExpr,
    arms: &[crate::hir::ResolvedMatchArm],
    value_indexes: &HashMap<ValueId, u32>,
    function_indexes: &HashMap<FunctionExecutionId, u32>,
    layout: &LocalLayout<'_>,
    result: Option<(u32, &str)>,
) -> Result<(), Diagnostic> {
    let scratch = layout
        .match_scratch
        .get(expr.id.as_str())
        .copied()
        .ok_or_else(|| {
            Diagnostic::io(
                "SPX-W108",
                format!("missing WebAssembly local layout for match `{}`", expr.id),
            )
        })?;
    // One evaluation: emit the scrutinee, then store it in the staging local.
    emit_expr(
        output,
        scrutinee,
        value_indexes,
        function_indexes,
        layout,
        result,
    )?;
    output.push(0x21);
    write_u32(output, scratch);

    let result_type = wasm_type(&expr.ty)?;
    // block $done (result T)
    output.push(0x02);
    output.push(result_type);
    for (index, arm) in arms.iter().enumerate() {
        let final_arm = index + 1 == arms.len();
        if !final_arm {
            // block $reject (void)
            output.extend_bytes(&[0x02, 0x40]);
            emit_pattern_test(output, scratch, &scrutinee.ty, &arm.pattern)?;
            output.push(0x45); // i32.eqz
            output.extend_bytes(&[0x0d, 0x00]); // br_if 0 -> next arm
        }
        if let Some(guard) = &arm.guard {
            debug_assert!(matches!(guard.ty, ResolvedType::Bool));
            emit_expr(
                output,
                guard.as_ref(),
                value_indexes,
                function_indexes,
                layout,
                result,
            )?;
            output.push(0x45); // i32.eqz: false guard falls through
            output.extend_bytes(&[0x0d, 0x00]); // br_if 0 -> next arm
        }
        emit_expr(
            output,
            &arm.value,
            value_indexes,
            function_indexes,
            layout,
            result,
        )?;
        if !final_arm {
            // Selected: exit $done carrying the arm value. Labels from here
            // are [reject_i(0), $done(1)] because earlier reject blocks were
            // closed; the trailing catch-all falls out of $done naturally.
            output.extend_bytes(&[0x0c, 0x01]); // br 1
            output.push(0x0b); // end reject block
        }
    }
    output.push(0x0b); // end $done
    Ok(())
}

/// Pushes an i32 truth value for one scalar pattern: `local.get` then an
/// equality chain over the literal alternatives joined with `i32.or`.
fn emit_pattern_test(
    output: &mut impl ByteOutput,
    scratch: u32,
    scrutinee_ty: &ResolvedType,
    pattern: &crate::hir::ResolvedMatchPattern,
) -> Result<(), Diagnostic> {
    let alternatives: &[crate::hir::PatternValue] = match pattern {
        crate::hir::ResolvedMatchPattern::Literal(value) => std::slice::from_ref(value),
        crate::hir::ResolvedMatchPattern::Or(alternatives) => {
            let mut flattened = Vec::with_capacity(alternatives.len());
            for alternative in alternatives {
                match alternative {
                    crate::hir::ResolvedMatchPattern::Literal(value) => {
                        flattened.push(*value);
                    }
                    _ => {
                        return Err(Diagnostic::io(
                            "SPX-M105",
                            "or-pattern alternatives must be literals",
                        ))
                    }
                }
            }
            return emit_alternative_tests(output, scratch, scrutinee_ty, &flattened);
        }
        // Refutable Match v1: an irrefutable pattern with a guard tests as
        // constant true; the guard decides. Unguarded irrefutable arms only
        // occur as the trailing catch-all, which emits no test.
        crate::hir::ResolvedMatchPattern::Wildcard
        | crate::hir::ResolvedMatchPattern::Binding(_) => {
            output.extend_bytes(&[0x41, 0x01]); // i32.const 1
            return Ok(());
        }
        crate::hir::ResolvedMatchPattern::Variant { .. }
        | crate::hir::ResolvedMatchPattern::Record { .. } => {
            return Err(Diagnostic::io(
                "SPX-W110",
                "aggregate arm reached scalar match lowering",
            ))
        }
    };
    emit_alternative_tests(output, scratch, scrutinee_ty, alternatives)
}

fn emit_alternative_tests(
    output: &mut impl ByteOutput,
    scratch: u32,
    scrutinee_ty: &ResolvedType,
    alternatives: &[crate::hir::PatternValue],
) -> Result<(), Diagnostic> {
    for (position, value) in alternatives.iter().enumerate() {
        output.push(0x20); // local.get scratch
        write_u32(output, scratch);
        match (scrutinee_ty, value) {
            (ResolvedType::I64, crate::hir::PatternValue::Int(inner)) => {
                output.push(0x42); // i64.const
                write_i64(output, *inner);
                output.push(0x51); // i64.eq -> i32
            }
            (ResolvedType::I32, crate::hir::PatternValue::Int32(inner)) => {
                output.push(0x41); // i32.const
                write_i64(output, i64::from(*inner));
                output.push(0x46); // i32.eq
            }
            (ResolvedType::U8, crate::hir::PatternValue::Uint8(inner)) => {
                output.push(0x41);
                write_i64(output, i64::from(*inner));
                output.push(0x46);
            }
            (ResolvedType::Char, crate::hir::PatternValue::Char(inner)) => {
                output.push(0x41);
                write_i64(output, i64::from(*inner));
                output.push(0x46);
            }
            (ResolvedType::Bool, crate::hir::PatternValue::Bool(inner)) => {
                output.push(0x41);
                write_i64(output, i64::from(*inner));
                output.push(0x46);
            }
            _ => {
                return Err(Diagnostic::io(
                    "SPX-T255",
                    format!(
                        "literal pattern disagrees with its `{}` scrutinee",
                        scrutinee_ty.identity_key()
                    ),
                ));
            }
        }
        // Both equality flags are now stacked; join them for alternatives
        // after the first.
        if position != 0 {
            output.push(0x72); // i32.or
        }
    }
    Ok(())
}

fn emit_short_circuit(
    output: &mut impl ByteOutput,
    op: BinaryOp,
    right: &ResolvedExpr,
    value_indexes: &HashMap<ValueId, u32>,
    function_indexes: &HashMap<FunctionExecutionId, u32>,
    layout: &LocalLayout,
    result: Option<(u32, &str)>,
) -> Result<(), Diagnostic> {
    output.push(0x04);
    output.push(I32);
    if op == BinaryOp::And {
        emit_expr(
            output,
            right,
            value_indexes,
            function_indexes,
            layout,
            result,
        )?;
        output.push(0x05);
        output.extend_bytes(&[0x41, 0x00]);
    } else {
        output.extend_bytes(&[0x41, 0x01]);
        output.push(0x05);
        emit_expr(
            output,
            right,
            value_indexes,
            function_indexes,
            layout,
            result,
        )?;
    }
    output.push(0x0b);
    Ok(())
}

fn emit_contract_guard(output: &mut impl ByteOutput, failure_code: Option<i64>) {
    output.push(0x45);
    output.extend_bytes(&[0x04, 0x40]);
    if let Some(code) = failure_code {
        output.push(0x41);
        write_i64(output, code);
    }
    output.push(0x10);
    write_u32(output, 6);
    output.push(0x00);
    output.push(0x0b);
}

fn call_import(output: &mut impl ByteOutput, index: u32) {
    output.push(0x10);
    write_u32(output, index);
}

fn local_get(output: &mut impl ByteOutput, index: u32) {
    output.push(0x20);
    write_u32(output, index);
}

fn local_set(output: &mut impl ByteOutput, index: u32) {
    output.push(0x21);
    write_u32(output, index);
}

/// An `if (condition) unreachable` block: the core-module failure channel for
/// detected i32 arithmetic overflow, which has no status local in this lane.
fn emit_unreachable_trap(output: &mut impl ByteOutput) {
    output.extend_bytes(&[0x04, 0x40, 0x00, 0x0b]);
}

/// Inline checked i32 arithmetic without new host imports. Operands widen to
/// i64 so add/sub/mul compute exactly; the wrapped result must re-extend to
/// the same wide value. Division guards divisor zero and INT32_MIN / -1.
#[allow(clippy::too_many_arguments)]
fn emit_i32_checked_binary(
    output: &mut impl ByteOutput,
    op: BinaryOp,
    left: &ResolvedExpr,
    right: &ResolvedExpr,
    value_indexes: &HashMap<ValueId, u32>,
    function_indexes: &HashMap<FunctionExecutionId, u32>,
    layout: &LocalLayout,
    result: Option<(u32, &str)>,
) -> Result<(), Diagnostic> {
    let [wide, other] = layout.wide_scratch;
    emit_expr(
        output,
        left,
        value_indexes,
        function_indexes,
        layout,
        result,
    )?;
    output.push(0xac);
    local_set(output, wide);
    emit_expr(
        output,
        right,
        value_indexes,
        function_indexes,
        layout,
        result,
    )?;
    output.push(0xac);
    local_set(output, other);
    match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
            local_get(output, wide);
            local_get(output, other);
            output.push(match op {
                BinaryOp::Add => 0x7c,
                BinaryOp::Sub => 0x7d,
                _ => 0x7e,
            });
            local_set(output, wide);
        }
        BinaryOp::Div | BinaryOp::Rem => {
            local_get(output, other);
            output.push(0x50);
            emit_unreachable_trap(output);
            if op == BinaryOp::Div {
                local_get(output, wide);
                output.push(0xa7);
                output.push(0x41);
                write_i64(output, i64::from(i32::MIN));
                output.push(0x46);
                local_get(output, other);
                output.push(0xa7);
                output.push(0x41);
                write_i64(output, -1);
                output.push(0x46);
                output.push(0x71);
                emit_unreachable_trap(output);
            }
            local_get(output, wide);
            local_get(output, other);
            output.push(if op == BinaryOp::Div { 0x7f } else { 0x81 });
            local_set(output, wide);
        }
        _ => return Err(Diagnostic::io("SPX-W102", "unsupported i32 arithmetic")),
    }
    if matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) {
        local_get(output, wide);
        output.push(0xa7);
        output.push(0xac);
        local_get(output, wide);
        output.push(0x51);
        output.push(0x45);
        emit_unreachable_trap(output);
    }
    local_get(output, wide);
    output.push(0xa7);
    Ok(())
}

/// Whether a function body or contract contains i32 arithmetic that needs the
/// reserved i64 scratch pair.
pub(crate) fn needs_i32_wide_scratch(expression: &ResolvedExpr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ResolvedExprKind::Unary { op, value } => {
                if *op == UnaryOp::Neg && value.ty == ResolvedType::I32 {
                    return true;
                }
                pending.push(value);
            }
            ResolvedExprKind::Binary { op, left, right } => {
                if matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                ) && left.ty == ResolvedType::I32
                {
                    return true;
                }
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Call { args, .. } => pending.extend(args.iter()),
            ResolvedExprKind::NativeRustImportCall(call) => pending.extend(call.args.iter()),
            ResolvedExprKind::HostCommandCall(call) => pending.extend(call.args.iter()),
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => pending.extend([source.as_ref(), start.as_ref(), end.as_ref()]),
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
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
            | ResolvedExprKind::ConstructVariant { fields, .. }
            | ResolvedExprKind::UpdateRecord { fields, .. } => {
                for field in fields {
                    pending.push(&field.value);
                }
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.push(scrutinee);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        pending.push(guard.as_ref());
                    }
                    pending.push(&arm.value);
                }
            }
            ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
                pending.push(operand);
            }
            ResolvedExprKind::Project { base, .. } => pending.push(base),
            ResolvedExprKind::Upcast { source } => pending.push(source),
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
    }
    false
}

/// One deterministic checked-arithmetic failure trap: an empty void `if`
/// block whose body is `unreachable`. Callers keep live values in scratch
/// locals because the polymorphic block taints the operand stack.
fn emit_failure_trap(output: &mut impl ByteOutput) {
    output.push(0x04);
    output.push(0x40);
    output.push(0x00);
    output.push(0x0b);
}

/// Whether an expression contains checked u8 arithmetic that needs the
/// function-level scratch locals.
fn contains_u8_arithmetic(expression: &ResolvedExpr) -> bool {
    contains_checked_arithmetic(expression, &ResolvedType::U8)
}

fn contains_usize_arithmetic(expression: &ResolvedExpr) -> bool {
    contains_checked_arithmetic(expression, &ResolvedType::Usize)
}

fn contains_checked_arithmetic(expression: &ResolvedExpr, target: &ResolvedType) -> bool {
    match &expression.kind {
        ResolvedExprKind::Binary { op, left, right: _ }
            if left.ty == *target
                && matches!(
                    *op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                ) =>
        {
            true
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            contains_checked_arithmetic(left, target) || contains_checked_arithmetic(right, target)
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => contains_checked_arithmetic(value, target),
        ResolvedExprKind::Call { args, .. } => args
            .iter()
            .any(|argument| contains_checked_arithmetic(argument, target)),
        ResolvedExprKind::NativeRustImportCall(call) => call
            .args
            .iter()
            .any(|argument| contains_checked_arithmetic(argument, target)),
        ResolvedExprKind::HostCommandCall(call) => call
            .args
            .iter()
            .any(|argument| contains_checked_arithmetic(argument, target)),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            contains_checked_arithmetic(source, target)
                || contains_checked_arithmetic(start, target)
                || contains_checked_arithmetic(end, target)
        }
        ResolvedExprKind::Block { statements, tail } => {
            contains_checked_arithmetic(tail, target)
                || statements.iter().any(|statement| {
                    (0..statement.child_count()).any(|index| {
                        statement
                            .child(index)
                            .is_some_and(|child| contains_checked_arithmetic(child, target))
                    })
                })
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            contains_checked_arithmetic(condition, target)
                || contains_checked_arithmetic(then_branch, target)
                || contains_checked_arithmetic(else_branch, target)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .any(|field| contains_checked_arithmetic(&field.value, target)),
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            contains_checked_arithmetic(scrutinee, target)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| contains_checked_arithmetic(guard, target))
                        || contains_checked_arithmetic(&arm.value, target)
                })
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            contains_checked_arithmetic(base, target)
                || fields
                    .iter()
                    .any(|field| contains_checked_arithmetic(&field.value, target))
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

fn wasm_type(ty: &ResolvedType) -> Result<u8, Diagnostic> {
    match ty {
        ResolvedType::Unit => Err(Diagnostic::io(
            "SPX-W101",
            "unit is not a WebAssembly value type",
        )),
        ResolvedType::I64 => Ok(I64),
        ResolvedType::I32 => Ok(I32),
        ResolvedType::Char => Ok(I32),
        ResolvedType::U8 => Ok(I32),
        ResolvedType::Usize => Ok(I64),
        ResolvedType::F32 => Ok(F32),
        ResolvedType::F64 => Ok(F64),
        ResolvedType::Bool | ResolvedType::Nominal { .. } => Ok(I32),
        // Owned strings lower to an abstract host handle riding the i64 lane.
        ResolvedType::String | ResolvedType::Str | ResolvedType::SliceU8 | ResolvedType::Bytes => {
            Ok(I64)
        }
        ResolvedType::ArrayU8(_) => Err(Diagnostic::io(
            "SPX-W101",
            "fixed byte arrays require the aggregate WebAssembly path",
        )),
        ResolvedType::TypeParameter { .. } => Err(Diagnostic::io(
            "SPX-W109",
            format!(
                "unresolved generic type `{}` cannot be lowered to WebAssembly",
                ty.identity_key()
            ),
        )),
    }
}

fn intern_type(
    signature: Signature,
    types: &mut Vec<Signature>,
    indexes: &mut HashMap<Signature, u32>,
) -> u32 {
    if let Some(index) = indexes.get(&signature) {
        return *index;
    }
    let index = types.len() as u32;
    types.push(signature.clone());
    indexes.insert(signature, index);
    index
}

fn function_import(output: &mut impl ByteOutput, module: &str, name: &str, type_index: u32) {
    write_name(output, module);
    write_name(output, name);
    output.push(0x00);
    write_u32(output, type_index);
}

fn section(module: &mut impl ByteOutput, id: u8, contents: impl std::ops::Deref<Target = [u8]>) {
    module.push(id);
    write_u32(module, contents.len() as u32);
    module.extend_bytes(&contents);
}

fn write_name(output: &mut impl ByteOutput, value: &str) {
    write_bytes(output, value.as_bytes());
}

fn write_bytes(output: &mut impl ByteOutput, value: &[u8]) {
    write_u32(output, value.len() as u32);
    output.extend_bytes(value);
}

fn write_u32(output: &mut impl ByteOutput, mut value: u32) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn write_i64(output: &mut impl ByteOutput, mut value: i64) {
    loop {
        let byte = (value as u8) & 0x7f;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        output.push(if done { byte } else { byte | 0x80 });
        if done {
            break;
        }
    }
}

fn write_i32(output: &mut impl ByteOutput, mut value: i32) {
    loop {
        let byte = (value as u8) & 0x7f;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        output.push(if done { byte } else { byte | 0x80 });
        if done {
            break;
        }
    }
}

fn json_strings(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| quote_json(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn browser_runtime() -> &'static str {
    r#"const SPX_MIN = -(1n << 63n);
const SPX_MAX = (1n << 63n) - 1n;
const SPX_POISON_I64 = 0x5a5a5a5a5a5a5a5an;
const SPX_POISON_HANDLE = 0x5a5a5a5a;
const SPX_MAX_RUNTIME_TAG = 0x7ff;
const SPX_MAX_SLOT = 0x3ff;
const SPX_MAX_GENERATION = 0x3ff;
const SPX_MAX_DYNAMIC_STATUS = 0x7ffffffe;
const SPX_EXHAUSTED_STATUS = 0x7fffffff;
const SPX_OWNED_EXPORTS = __SEMAPRAX_OWNED_EXPORTS__;
const SPX_WASM_SHA256 = "__SEMAPRAX_WASM_SHA256__";
const SPX_RUNTIME_TAG_ALLOCATOR_KEY = Symbol.for("semaprax.wasm-owned.runtime-tags.v1");
const spxLocalRuntimeTags = new Set();

function runtimeTagAllocator() {
  const installed = globalThis[SPX_RUNTIME_TAG_ALLOCATOR_KEY];
  if (installed !== undefined) {
    if (typeof installed !== "object" || installed === null || typeof installed.take !== "function") {
      throw new Error("SEMAPRAX runtime-tag allocator global is invalid");
    }
    return installed;
  }
  let next = 1;
  const allocator = Object.freeze({
    take() {
      if (next > SPX_MAX_RUNTIME_TAG) {
        throw new Error("SEMAPRAX owned runtime instance identity space exhausted");
      }
      return next++;
    },
  });
  Object.defineProperty(globalThis, SPX_RUNTIME_TAG_ALLOCATOR_KEY, {
    value: allocator,
    configurable: false,
    enumerable: false,
    writable: false,
  });
  return allocator;
}

async function authenticatedWasmBytes(bytes) {
  let source;
  if (bytes instanceof ArrayBuffer) {
    source = new Uint8Array(bytes);
  } else if (ArrayBuffer.isView(bytes)) {
    source = new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  } else {
    throw new TypeError("SEMAPRAX instantiateBytes requires an ArrayBuffer or typed-array view");
  }
  const ownedCopy = new Uint8Array(source);
  if (globalThis.crypto === undefined || globalThis.crypto.subtle === undefined) {
    throw new Error("SEMAPRAX Web Crypto SHA-256 support is required");
  }
  const digest = new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", ownedCopy));
  const actual = Array.from(digest, byte => byte.toString(16).padStart(2, "0")).join("");
  if (actual !== SPX_WASM_SHA256) {
    throw new Error("SEMAPRAX WebAssembly artifact authentication failed");
  }
  return ownedCopy;
}

function checked(value, operation) {
  if (value < SPX_MIN || value > SPX_MAX) {
    throw new RangeError(`SEMAPRAX checked arithmetic failure: ${operation}`);
  }
  return value;
}

function createByteDataRuntime(options = {}) {
  const entries = new Map();
  const maxLiveEntries = boundedLimit(options.maxOwnedByteEntries, 16, "owned-byte-entry");
  let nextToken = 1;
  let instance = null;
  const decode = carrier => {
    if (typeof carrier !== "bigint") throw new TypeError("SEMAPRAX byte carrier is not i64");
    const word = BigInt.asUintN(64, carrier);
    const length = Number(word & 0xffffffffn);
    const root = Number((word >> 32n) & 0xffffffffn);
    if (length > 65536) throw new Error("SEMAPRAX byte carrier length invariant");
    return { carrier: word, length, root, tagged: (root & 0x80000000) !== 0, token: root & 0x7fffffff };
  };
  const memory = () => {
    const candidate = instance?.exports.__spx_byte_memory;
    if (!(candidate instanceof WebAssembly.Memory) || candidate.buffer.byteLength !== 131072) {
      throw new Error("SEMAPRAX fixed byte memory invariant");
    }
    return new Uint8Array(candidate.buffer);
  };
  const resolve = decoded => {
    if (!decoded.tagged || decoded.token === 0) throw new Error("SEMAPRAX owned Bytes token invariant");
    const entry = entries.get(decoded.token);
    if (!(entry instanceof Uint8Array) || entry.byteLength !== decoded.length) {
      throw new Error("SEMAPRAX stale or malformed owned Bytes carrier");
    }
    return entry;
  };
  const read = decoded => {
    if (decoded.tagged) return resolve(decoded);
    if ((decoded.root & 0xc0000000) === 0x40000000) {
      // The guest validates the descriptor against its private binding globals
      // immediately before this synchronous import. The adapter independently
      // replays the carrier-to-memory identity, shape, and extent checks before
      // it reads either guest memory or an authenticated owned entry.
      const pointer = (decoded.root & 0xffff) * 8;
      if (pointer > 131072 - 32) throw new Error("SEMAPRAX byte range descriptor bounds invariant");
      const bytes = memory();
      const descriptor = new DataView(bytes.buffer, bytes.byteOffset + pointer, 32);
      const identity = descriptor.getUint32(0, true);
      const self = descriptor.getUint32(4, true);
      const carrierIdentity = (decoded.root >>> 16) & 0x1fff;
      if (identity === 0 || identity !== carrierIdentity || self !== pointer) {
        throw new Error("SEMAPRAX byte range descriptor identity invariant");
      }
      const ultimate = decode(descriptor.getBigInt64(8, true));
      const offset = descriptor.getBigUint64(16, true);
      const length = descriptor.getBigUint64(24, true);
      if (length !== BigInt(decoded.length)) throw new Error("SEMAPRAX byte range descriptor length invariant");
      if ((ultimate.root & 0xc0000000) === 0x40000000) {
        throw new Error("SEMAPRAX nested byte range descriptor invariant");
      }
      if (offset > BigInt(ultimate.length) || length > BigInt(ultimate.length) - offset) {
        throw new Error("SEMAPRAX byte range descriptor extent invariant");
      }
      const root = ultimate.tagged ? resolve(ultimate) : (() => {
        if (ultimate.root > 131072 - ultimate.length) throw new Error("SEMAPRAX fixed byte range invariant");
        return bytes.slice(ultimate.root, ultimate.root + ultimate.length);
      })();
      const start = Number(offset);
      return root.slice(start, start + Number(length));
    }
    if (decoded.root > 131072 - decoded.length) throw new Error("SEMAPRAX fixed byte range invariant");
    return memory().slice(decoded.root, decoded.root + decoded.length);
  };
  const allocate = bytes => {
    if (entries.size >= maxLiveEntries) throw new Error("SEMAPRAX owned Bytes live entry limit exceeded");
    if (nextToken > 0x7fffffff) throw new Error("SEMAPRAX owned Bytes token space exhausted");
    const token = nextToken++;
    const owned = new Uint8Array(bytes);
    entries.set(token, owned);
    const root = 0x80000000n | BigInt(token);
    return BigInt.asIntN(64, (root << 32n) | BigInt(owned.byteLength));
  };
  const byteImports = Object.freeze({
    spx_bytes_copy: carrier => allocate(read(decode(carrier))),
    spx_bytes_get: (carrier, index) => {
      const bytes = read(decode(carrier));
      const unsigned = BigInt.asUintN(64, index);
      return unsigned >= BigInt(bytes.byteLength) ? -1 : bytes[Number(unsigned)];
    },
    spx_bytes_drop: carrier => {
      const decoded = decode(carrier);
      resolve(decoded);
      entries.delete(decoded.token);
    },
    spx_bytes_as_slice: carrier => {
      const decoded = decode(carrier);
      if (decoded.tagged) resolve(decoded); else read(decoded);
      return BigInt.asIntN(64, decoded.carrier);
    },
  });
  return Object.freeze({
    imports: byteImports,
    bind(wasmInstance) {
      if (instance !== null) throw new Error("SEMAPRAX byte runtime already bound");
      instance = wasmInstance;
    },
  });
}

export const imports = {
  env: {
    spx_add: (a, b) => checked(a + b, "addition overflow"),
    spx_sub: (a, b) => checked(a - b, "subtraction overflow"),
    spx_mul: (a, b) => checked(a * b, "multiplication overflow"),
    spx_div: (a, b) => {
      if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("SEMAPRAX checked arithmetic failure: invalid division");
      return a / b;
    },
    spx_rem: (a, b) => {
      if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("SEMAPRAX checked arithmetic failure: invalid remainder");
      return a % b;
    },
    spx_neg: value => checked(-value, "negation overflow"),
    spx_contract_fail: () => { throw new Error("SEMAPRAX contract failure"); },
  },
};

function boundedLimit(value, maximum, name) {
  if (value === undefined) return maximum;
  if (!Number.isSafeInteger(value) || value < 1 || value > maximum) {
    throw new RangeError(`invalid SEMAPRAX ${name} limit`);
  }
  return value;
}

function createOwnedRuntime(options = {}) {
  const maxSlot = boundedLimit(options.maxOwnedSlots, SPX_MAX_SLOT, "owned-slot");
  const maxDynamicStatus = boundedLimit(options.maxStatusTokens, SPX_MAX_DYNAMIC_STATUS, "status-token");
  const runtimeTag = runtimeTagAllocator().take();
  if (!Number.isInteger(runtimeTag) || runtimeTag < 1 || runtimeTag > SPX_MAX_RUNTIME_TAG
      || spxLocalRuntimeTags.has(runtimeTag)) {
    throw new Error("SEMAPRAX runtime-tag allocator returned an invalid or repeated identity");
  }
  spxLocalRuntimeTags.add(runtimeTag);
  const context = ((runtimeTag << 20) | 0x5350) | 0;
  const slots = new Map();
  const generations = new Map();
  const freeSlots = [];
  const statuses = new Map();
  statuses.set(SPX_EXHAUSTED_STATUS, Object.freeze({
    schema: "semaprax.status.v1",
    domain_id: "semaprax.wasm-adapter.v1",
    code: 5,
    class: "adapter",
    retryable: false,
  }));
  const events = [];
  const adoptionTickets = new WeakMap();
  let nextSlot = 1;
  let nextStatus = 1;
  let staging = null;
  let activeResult = null;
  let activeStatus = null;
  let semanticInvocation = null;
  let instance = null;

  const recordStatus = (domain, code, classification) => {
    if (nextStatus > maxDynamicStatus) return SPX_EXHAUSTED_STATUS;
    const token = nextStatus++;
    statuses.set(token, Object.freeze({
      schema: "semaprax.status.v1",
      domain_id: domain,
      code,
      class: classification,
      retryable: false,
    }));
    return token;
  };
  const fillStatus = (status, domain, code, classification) => {
    status.domain_id = domain;
    status.code = code;
    status.class = classification;
    Object.freeze(status);
  };
  const adapterFailure = code => {
    if (staging !== null) {
      fillStatus(staging.status, "semaprax.wasm-adapter.v1", code, "adapter");
      staging.retainStatus = true;
      return staging.statusToken;
    }
    if (activeStatus !== null) {
      fillStatus(activeStatus.status, "semaprax.wasm-adapter.v1", code, "adapter");
      const token = activeStatus.token;
      activeStatus = null;
      return token;
    }
    return recordStatus("semaprax.wasm-adapter.v1", code, "adapter");
  };
  const requireContext = candidate => candidate === context;
  const reserveSlot = (value, state) => {
    let slot;
    let generation;
    while (freeSlots.length > 0) {
      slot = freeSlots.pop();
      generation = (generations.get(slot) ?? 0) + 1;
      if (generation <= SPX_MAX_GENERATION) break;
      slot = undefined;
    }
    if (slot === undefined) {
      if (nextSlot > maxSlot) throw new Error("SEMAPRAX owned handle table exhausted");
      slot = nextSlot++;
      generation = 1;
    }
    generations.set(slot, generation);
    const handle = ((runtimeTag << 20) | (generation << 10) | slot) | 0;
    if (handle === 0 || slots.has(handle)) throw new Error("SEMAPRAX handle allocation invariant");
    const entry = { slot, generation, value, state };
    slots.set(handle, entry);
    return { handle, entry };
  };
  const allocate = value => reserveSlot(value, "owned").handle;
  const release = (handle, expected) => {
    const entry = slots.get(handle);
    if (!entry || entry.state !== expected) throw new Error("SEMAPRAX owned runtime invariant");
    slots.delete(handle);
    freeSlots.push(entry.slot);
    return entry;
  };

  const ownedImports = {
    spx_owned_begin: candidate => {
      if (!requireContext(candidate)) return adapterFailure(1);
      if (staging !== null || activeStatus !== null || activeResult !== null) return adapterFailure(2);
      if (nextStatus > maxDynamicStatus) return SPX_EXHAUSTED_STATUS;
      const statusToken = nextStatus++;
      const status = {
        schema: "semaprax.status.v1",
        domain_id: null,
        code: 0,
        class: null,
        retryable: false,
      };
      statuses.set(statusToken, status);
      staging = { handles: [], result: null, statusToken, status, retainStatus: false };
      return 0;
    },
    spx_owned_stage: (candidate, handle) => {
      if (!requireContext(candidate)) return adapterFailure(1);
      if (staging === null) return adapterFailure(2);
      const entry = slots.get(handle);
      if (!entry || entry.state !== "owned") return adapterFailure(3);
      if (staging.handles.includes(handle)) return adapterFailure(4);
      staging.handles.push(handle);
      return 0;
    },
    spx_owned_abort: candidate => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned abort context invariant");
      if (staging !== null && staging.result !== null) release(staging.result, "reserved");
      if (staging !== null && !staging.retainStatus) statuses.delete(staging.statusToken);
      staging = null;
    },
    spx_owned_reserve_result: candidate => {
      if (!requireContext(candidate)) return adapterFailure(1);
      if (staging === null || staging.result !== null) return adapterFailure(2);
      try {
        staging.result = reserveSlot(undefined, "reserved").handle;
      } catch (error) {
        if (error instanceof Error && error.message === "SEMAPRAX owned handle table exhausted") {
          return adapterFailure(5);
        }
        throw error;
      }
      return 0;
    },
    spx_owned_commit: candidate => {
      if (!requireContext(candidate)) return adapterFailure(1);
      if (staging === null) return adapterFailure(2);
      for (const handle of staging.handles) {
        const entry = slots.get(handle);
        if (!entry || entry.state !== "owned") return adapterFailure(3);
      }
      for (const handle of staging.handles) slots.get(handle).state = "inflight";
      activeResult = staging.result;
      activeStatus = { token: staging.statusToken, status: staging.status };
      events.push(Object.freeze({ kind: "commit", handles: Object.freeze([...staging.handles]) }));
      staging = null;
      return 0;
    },
    spx_owned_drop: (candidate, handle) => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned drop context invariant");
      release(handle, "inflight");
      events.push(Object.freeze({ kind: "drop", handle }));
    },
    spx_owned_cancel_result: candidate => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned cancel context invariant");
      if (activeResult === null) throw new Error("SEMAPRAX result reservation invariant");
      release(activeResult, "reserved");
      activeResult = null;
    },
    spx_owned_publish: (candidate, handle) => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned publish context invariant");
      const entry = release(handle, "inflight");
      if (activeResult === null) throw new Error("SEMAPRAX result publication reservation invariant");
      const published = activeResult;
      const reserved = slots.get(published);
      if (!reserved || reserved.state !== "reserved") throw new Error("SEMAPRAX reserved result invariant");
      reserved.value = entry.value;
      reserved.state = "owned";
      activeResult = null;
      events.push(Object.freeze({ kind: "publish", from: handle, to: published }));
      return published;
    },
    spx_status_record: (candidate, classification, code) => {
      if (!requireContext(candidate)) return adapterFailure(1);
      const target = staging ?? activeStatus;
      if (target === null) throw new Error("SEMAPRAX status reservation invariant");
      let domain;
      let statusClass;
      if (classification === 1 || classification === 2) {
        domain = "semaprax.contract.v1";
        statusClass = "contract";
      } else if (classification === 3) {
        domain = "semaprax.arithmetic.v1";
        statusClass = "arithmetic";
      } else if (classification === 4) {
        domain = "semaprax.wasm-adapter.v1";
        statusClass = "adapter";
      } else {
        throw new Error("SEMAPRAX compiler status classification invariant");
      }
      fillStatus(target.status, domain, code, statusClass);
      events.push(Object.freeze({ kind: "status", domain_id: domain, code, class: statusClass }));
      const token = target.token ?? target.statusToken;
      if (staging !== null) staging.retainStatus = true;
      else activeStatus = null;
      return token;
    },
    spx_owned_success: candidate => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned success context invariant");
      if (activeStatus === null || activeResult !== null) throw new Error("SEMAPRAX success reservation invariant");
      statuses.delete(activeStatus.token);
      activeStatus = null;
    },
    spx_semantic_event: (candidate, functionOrdinal, eventOrdinal) => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX semantic event context invariant");
      if (semanticInvocation === null) throw new Error("SEMAPRAX semantic event outside invocation");
      const contract = semanticInvocation.contract;
      if (functionOrdinal !== contract.function_ordinal
          || !contract.valid_ordinals.includes(eventOrdinal)
          || eventOrdinal === 0) {
        throw new Error("SEMAPRAX semantic event dictionary invariant");
      }
      semanticInvocation.ordinals.push(eventOrdinal);
    },
  };

  const facade = Object.freeze({
    prepareTrustedAdoption(value) {
      const ticket = Object.freeze(Object.create(null));
      adoptionTickets.set(ticket, { consumed: false, value });
      return ticket;
    },
    adopt(ticket) {
      const adoption = adoptionTickets.get(ticket);
      if (adoption === undefined || adoption.consumed) {
        throw new TypeError("SEMAPRAX adoption ticket is invalid or already consumed");
      }
      const handle = allocate(adoption.value);
      adoption.consumed = true;
      adoption.value = undefined;
      return handle;
    },
    dispose(handle) {
      if (!Number.isInteger(handle) || handle === 0) {
        throw new TypeError("SEMAPRAX owned handle is invalid");
      }
      release(handle, "owned");
      events.push(Object.freeze({ kind: "drop", handle }));
    },
    invoke(exportName, args, resultKind) {
      if (instance === null) throw new Error("SEMAPRAX owned runtime is not bound");
      if (typeof exportName !== "string") {
        throw new TypeError("SEMAPRAX owned export name must be a string");
      }
      if (!Object.hasOwn(SPX_OWNED_EXPORTS, exportName)) {
        throw new TypeError(`unknown SEMAPRAX owned export: ${exportName}`);
      }
      const contract = SPX_OWNED_EXPORTS[exportName];
      if (resultKind !== contract.result) {
        throw new TypeError(`SEMAPRAX owned export ${exportName} requires result kind ${contract.result}`);
      }
      if (!Array.isArray(args) || args.length !== contract.parameters.length) {
        throw new TypeError(`SEMAPRAX owned export ${exportName} argument count mismatch`);
      }
      const canonicalArgs = [];
      for (let index = 0; index < contract.parameters.length; index += 1) {
        const kind = contract.parameters[index];
        const value = args[index];
        const valid = kind === "i64" ? typeof value === "bigint" && value >= SPX_MIN && value <= SPX_MAX
          : kind === "bool" ? Number.isInteger(value) && (value === 0 || value === 1)
          : kind === "resource" ? Number.isInteger(value) && value >= 1 && value <= 0x7fffffff
          : false;
        if (!valid) throw new TypeError(`SEMAPRAX owned export ${exportName} argument ${index} kind mismatch`);
        canonicalArgs.push(value);
      }
      const fn = instance.exports[exportName];
      if (typeof fn !== "function") throw new Error(`missing SEMAPRAX owned export: ${exportName}`);
      const memory = instance.exports.memory;
      if (!(memory instanceof WebAssembly.Memory)) throw new Error("SEMAPRAX owned memory export is absent");
      const view = new DataView(memory.buffer);
      if (resultKind === "i64") view.setBigInt64(0, SPX_POISON_I64, true);
      else view.setInt32(0, SPX_POISON_HANDLE, true);
      const callArgs = [context];
      for (let index = 0; index < canonicalArgs.length; index += 1) {
        callArgs.push(canonicalArgs[index]);
      }
      callArgs.push(0);
      semanticInvocation = { contract, ordinals: [] };
      let statusToken;
      try {
        statusToken = Reflect.apply(fn, undefined, callArgs);
      } catch (error) {
        semanticInvocation = null;
        throw error;
      }
      const semantic = Object.freeze({
        schema: contract.dictionary_schema,
        function: contract.function,
        dictionary_fingerprint: contract.dictionary_fingerprint,
        ordinals: Object.freeze([...semanticInvocation.ordinals]),
      });
      semanticInvocation = null;
      if (statusToken !== 0) {
        const preserved = resultKind === "i64"
          ? view.getBigInt64(0, true) === SPX_POISON_I64
          : view.getInt32(0, true) === SPX_POISON_HANDLE;
        if (!preserved) throw new Error("SEMAPRAX failure published a poisoned result slot");
        const status = statuses.get(statusToken);
        if (!status) throw new Error("SEMAPRAX returned an unknown status token");
        return Object.freeze({ ok: false, published: false, statusToken, status, semantic });
      }
      const value = resultKind === "i64" ? view.getBigInt64(0, true) : view.getInt32(0, true);
      return Object.freeze({ ok: true, published: true, value, semantic });
    },
    resolveStatus(token) {
      return statuses.get(token) ?? null;
    },
    trace() {
      return events.map(event => ({ ...event, handles: event.handles ? [...event.handles] : undefined }));
    },
    liveHandleCount() {
      return slots.size;
    },
  });

  return Object.freeze({
    linkImports: Object.freeze({ env: Object.freeze(ownedImports) }),
    bind(wasmInstance) {
      if (instance !== null) throw new Error("SEMAPRAX owned runtime already bound");
      instance = wasmInstance;
    },
    facade,
  });
}

export async function instantiateBytes(bytes, options = {}) {
  const authenticatedBytes = await authenticatedWasmBytes(bytes);
  const byteRuntime = createByteDataRuntime(options);
  if (Object.keys(SPX_OWNED_EXPORTS).length === 0) {
    const linkedImports = { env: { ...imports.env, ...byteRuntime.imports } };
    const result = await WebAssembly.instantiate(authenticatedBytes, linkedImports);
    byteRuntime.bind(result.instance);
    return Object.freeze(result);
  }
  const runtime = createOwnedRuntime(options);
  const linkedImports = { env: { ...imports.env, ...byteRuntime.imports, ...runtime.linkImports.env } };
  const result = await WebAssembly.instantiate(authenticatedBytes, linkedImports);
  byteRuntime.bind(result.instance);
  runtime.bind(result.instance);
  return Object.freeze({ ...result, owned: runtime.facade });
}

export async function instantiate(url = new URL("./app.wasm", import.meta.url)) {
  const response = await fetch(url);
  return instantiateBytes(await response.arrayBuffer());
}
"#
}

fn browser_html() -> &'static str {
    r##"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>SEMAPRAX</title>
  </head>
  <body>
    <main>
      <h1>SEMAPRAX</h1>
      <output id="result" aria-live="polite">Loading…</output>
    </main>
    <script type="module">
      import { instantiate } from "./semaprax.js";
      const { instance } = await instantiate();
      document.querySelector("#result").value = instance.exports.semaprax_main().toString();
    </script>
  </body>
</html>
"##
}

#[cfg(test)]
mod project_web_build_tests {
    use super::*;

    fn embedded_manifest(project: &str, artifacts: &[Vec<u8>]) -> String {
        let digest = |index: usize| {
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(&artifacts[index]))
            )
        };
        format!(
            "{{\"schema\":\"semaprax.web-project.v1\",\"project_schema\":\"semaprax.project.v1\",\"project\":{project:?},\"project_revision\":\"sha256:project\",\"workspace_revision\":\"sha256:workspace\",\"project_graph_digest\":\"sha256:graph\",\"entry_module\":\"calculator.app\",\"capabilities\":[],\"artifacts\":[{{\"path\":\"app.wasm\",\"sha256\":\"{}\"}},{{\"path\":\"index.html\",\"sha256\":\"{}\"}},{{\"path\":\"package.json\",\"sha256\":\"{}\"}},{{\"path\":\"semaprax.bindings.d.ts\",\"sha256\":\"{}\"}},{{\"path\":\"semaprax.bindings.js\",\"sha256\":\"{}\"}},{{\"path\":\"semaprax.js\",\"sha256\":\"{}\"}}],\"scalar_abi\":{{\"schema\":\"semaprax.wasm-scalar.v1\",\"functions\":[{{\"stable_id\":\"calculator.add\",\"wasm_export\":{},\"parameters\":[\"i64\",\"i64\"],\"result\":\"i64\"}}]}}}}\n",
            digest(0),
            digest(6),
            digest(5),
            digest(3),
            digest(2),
            digest(1),
            quote_json(&scalar_exports::raw_symbol("calculator.add")),
        )
    }

    #[test]
    fn independently_replayed_inner_manifest_rejects_self_resigned_identity_forgery() {
        let mut bytes = vec![
            b"wasm".to_vec(),
            b"runtime".to_vec(),
            b"bindings".to_vec(),
            b"declarations".to_vec(),
            Vec::new(),
            b"package".to_vec(),
            b"index".to_vec(),
        ];
        bytes[4] = embedded_manifest("calculator", &bytes).into_bytes();
        let refs = PROJECT_WEB_ARTIFACT_PATHS
            .iter()
            .copied()
            .zip(bytes.iter().map(Vec::as_slice))
            .collect::<Vec<_>>();
        build_project_web_carrier(
            ProjectWebIdentity {
                project_name: "calculator",
                project_revision: "sha256:project",
                workspace_revision: "sha256:workspace",
                project_graph_digest: "sha256:graph",
                entry_module: "calculator.app",
            },
            64 * 1024,
            &refs,
        )
        .unwrap();

        bytes[4] = embedded_manifest("calculat0r", &bytes).into_bytes();
        let forged_refs = PROJECT_WEB_ARTIFACT_PATHS
            .iter()
            .copied()
            .zip(bytes.iter().map(Vec::as_slice))
            .collect::<Vec<_>>();
        let error = build_project_web_carrier(
            ProjectWebIdentity {
                project_name: "calculator",
                project_revision: "sha256:project",
                workspace_revision: "sha256:workspace",
                project_graph_digest: "sha256:graph",
                entry_module: "calculator.app",
            },
            64 * 1024,
            &forged_refs,
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-W117");
        assert!(error.message.contains("embedded manifest disagrees"));

        bytes[4] = embedded_manifest("calculator", &bytes)
            .replacen('{', "{ ", 1)
            .into_bytes();
        let noncanonical_refs = PROJECT_WEB_ARTIFACT_PATHS
            .iter()
            .copied()
            .zip(bytes.iter().map(Vec::as_slice))
            .collect::<Vec<_>>();
        let error = build_project_web_carrier(
            ProjectWebIdentity {
                project_name: "calculator",
                project_revision: "sha256:project",
                workspace_revision: "sha256:workspace",
                project_graph_digest: "sha256:graph",
                entry_module: "calculator.app",
            },
            64 * 1024,
            &noncanonical_refs,
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-W117");
        assert!(error.message.contains("not canonical exact replay"));
    }
}

#[cfg(test)]
mod stdout_profile_authority_tests {
    use super::*;

    const SOURCE: &str = r#"
module test.stdout_wasm_authority;
permit { process.stdout.write }
@id("app.main")
fn main() -> i64 uses { process.stdout.write } {
    let data = [65u8];
    let view = array_as_slice(data);
    let written = stdout_write(view);
    if written == 1usize { 0 } else { 1 }
}
"#;

    fn resolved(source: &str) -> ResolvedProgram {
        let ast = crate::parse(source, Path::new("stdout-wasm-authority.spx")).unwrap();
        crate::hir::resolve(&ast).unwrap()
    }

    fn assert_rejected(program: &ResolvedProgram) {
        let diagnostic = emit_resolved_module_with_stdout_transcript(program).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-T269");
        assert!(diagnostic.message.contains("authority mismatch"));
    }

    fn source_with_unused_inventory(declaration: &str) -> String {
        SOURCE.replace(
            "@id(\"app.main\")",
            &format!("{declaration}\n\n@id(\"app.main\")"),
        )
    }

    #[test]
    fn raw_test_projection_rejects_permit_effect_and_interface_supersets() {
        let ast = crate::parse(SOURCE, Path::new("stdout-wasm-valid.spx")).unwrap();
        assert!(!emit_module_with_stdout_transcript(&ast).unwrap().is_empty());

        let mut widened_permit = resolved(SOURCE);
        widened_permit.permits.push("process.network".to_owned());
        assert_rejected(&widened_permit);

        let mut widened_effect = resolved(SOURCE);
        widened_effect.permits.push("process.network".to_owned());
        widened_effect.functions[0]
            .effects
            .push("process.network".to_owned());
        assert_rejected(&widened_effect);

        let with_interface = resolved(&SOURCE.replace(
            "@id(\"app.main\")",
            r#"@id("host.interface")
interface Host permits {} {
    @id("host.echo")
    import rust fn echo(value: i64) -> i64
        effects {}
        failure status "host.echo.v1";
}
@id("app.main")"#,
        ));
        assert_rejected(&with_interface);

        let generic = resolved(&source_with_unused_inventory(
            r#"@id("unused.identity")
fn identity<T>(value: T) -> T { value }"#,
        ));
        assert_rejected(&generic);

        let authored_type_inventories = [
            r#"@id("unused.record")
record UnusedRecord { @id("unused.record.value") value: i64, }"#,
            r#"@id("unused.resource")
resource UnusedResource { @id("unused.resource.drop") drop trivial; }"#,
            r#"@id("unused.variant")
variant UnusedVariant { @id("unused.variant.none") None, }"#,
            r#"@id("unused.class")
class UnusedClass { @id("unused.class.value") value: i64, }"#,
        ];
        for declaration in authored_type_inventories {
            assert_rejected(&resolved(&source_with_unused_inventory(declaration)));
        }
    }
}
