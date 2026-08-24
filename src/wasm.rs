use std::collections::HashMap;
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
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod generic_function_component_v9;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod generic_record_component_v7;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod nested_record_component_v6;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod option_propagation_component_v10;
mod owned;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod record_pattern_component_v8;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod result_component_v3;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod scalar_algebra_component_v5;
mod scalar_exports;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod source_result_component_v4;

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
            ResolvedExprKind::Match { scrutinee, arms } => {
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
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_) => {}
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
            ResolvedExprKind::Match { scrutinee, arms } => {
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
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_) => {}
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
            ResolvedExprKind::Match { scrutinee, arms } => {
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
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_) => {}
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
    /// Refutable Match v1: one staging local per scalar match expression,
    /// keyed by expression identity. The scrutinee evaluates once here and
    /// every arm test re-reads it.
    match_scratch: HashMap<String, u32>,
    /// Interned string literal offsets for the whole program, when strings
    /// are admitted at all.
    string_data: Option<&'a StringData>,
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

/// Emit a WebAssembly core module from verified, identity-resolved HIR.
///
/// Most callers should use [`emit_module`], which resolves and verifies parsed
/// source first. This entry point exists for semantic consumers that already
/// hold HIR and keeps all backend lowering independent of source-level names.
pub fn emit_resolved_module(program: &ResolvedProgram) -> Result<Vec<u8>, Diagnostic> {
    emit_resolved_module_internal(program, &[])
}

/// Emit the bounded Public Scalar Export Profile v1 from resolved HIR.
pub fn emit_resolved_module_with_scalar_exports(
    program: &ResolvedProgram,
    export_ids: &[String],
) -> Result<Vec<u8>, Diagnostic> {
    let plans = scalar_exports::prepare(program, export_ids)?;
    emit_resolved_module_internal(program, &plans)
}

fn emit_resolved_module_internal(
    program: &ResolvedProgram,
    scalar_exports: &[scalar_exports::ScalarExportPlan],
) -> Result<Vec<u8>, Diagnostic> {
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
    if has_authored_aggregate || !concrete_variants.is_empty() {
        if !scalar_exports.is_empty() {
            return Err(Diagnostic::io(
                "SPX-W115",
                "Public Scalar Export Profile v1 does not admit aggregate or variant lowering",
            ));
        }
        return aggregate::emit(program);
    }
    let owned_plans = owned::plan(program)?;
    if !scalar_exports.is_empty() && !owned_plans.is_empty() {
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
        };
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
            params: if scalar_exports.is_empty() {
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
    if let Some(type_indexes) = owned_import_types {
        for (name, type_index) in owned::IMPORT_NAMES.into_iter().zip(type_indexes) {
            function_import(&mut imports, "env", name, type_index);
        }
    }
    section(&mut module, 2, imports);

    let mut functions = crate::bounded_output::CappedVec::new();
    write_u32(
        &mut functions,
        (function_types.len() + owned_function_types.len() + scalar_export_types.len()) as u32,
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
    section(&mut module, 3, functions);

    if !owned_plans.is_empty() || uses_strings {
        let mut memories = crate::bounded_output::CappedVec::new();
        write_u32(&mut memories, 1);
        memories.extend([0x00, 0x01]); // one-page, unbounded memory
        section(&mut module, 5, memories);
    }

    // String literal bytes live in one deterministic data segment so host
    // shims can materialize handles with `spx_string_new(ptr, len)`.

    let mut exports = crate::bounded_output::CappedVec::new();
    let legacy_export_count = if scalar_exports.is_empty() {
        1 + owned_plans.len() as u32 + u32::from(!owned_plans.is_empty() || uses_strings)
    } else {
        0
    };
    write_u32(
        &mut exports,
        legacy_export_count + scalar_exports.len() as u32,
    );
    if scalar_exports.is_empty() {
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
    section(&mut module, 7, exports);

    let mut code = crate::bounded_output::CappedVec::new();
    write_u32(
        &mut code,
        (executable_functions.len() + owned_plans.len() + scalar_exports.len()) as u32,
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
            match_scratch: HashMap::new(),
            string_data: Some(&string_data),
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
            emit_contract_guard(&mut body, (!scalar_exports.is_empty()).then_some(1));
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
            emit_contract_guard(&mut body, (!scalar_exports.is_empty()).then_some(2));
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
    let wasm_bytes = emit_resolved_module_internal(&resolved, &plans)?;
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

impl PreparedProjectWeb {
    pub(crate) fn publish(self, output: &Path) -> Result<(), Diagnostic> {
        let package = "{\"private\":true,\"type\":\"module\",\"exports\":\"./semaprax.bindings.js\",\"types\":\"./semaprax.bindings.d.ts\"}\n";
        let index = scalar_browser_html();
        let artifacts: [(&str, &[u8]); 7] = [
            ("app.wasm", &self.wasm_bytes),
            ("semaprax.js", self.runtime.as_bytes()),
            ("semaprax.bindings.js", self.bindings.as_bytes()),
            ("semaprax.bindings.d.ts", self.declarations.as_bytes()),
            ("semaprax.scalar-exports.json", self.manifest.as_bytes()),
            ("package.json", package.as_bytes()),
            ("index.html", index.as_bytes()),
        ];
        publish_scalar_package(output, &artifacts)
    }
}

pub(crate) fn prepare_project_web_with_scalar_exports(
    program: &ResolvedProgram,
    project_name: &str,
    project_revision: &str,
    workspace_revision: &str,
    entry_module: &str,
    export_ids: &[String],
) -> Result<PreparedProjectWeb, Diagnostic> {
    let plans = scalar_exports::prepare(program, export_ids)?;
    let wasm_bytes = emit_resolved_module_internal(program, &plans)?;
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
        project_name,
        project_revision,
        workspace_revision,
        entry_module,
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
    project_name: &str,
    project_revision: &str,
    workspace_revision: &str,
    entry_module: &str,
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
        "{{\"schema\":\"semaprax.web-project.v1\",\"project_schema\":\"semaprax.project.v1\",\"project\":{},\"project_revision\":{},\"workspace_revision\":{},\"entry_module\":{},\"capabilities\":[],\"artifacts\":[{}],\"scalar_abi\":{{\"schema\":\"semaprax.wasm-scalar.v1\",\"functions\":[{}]}}}}\n",
        quote_json(project_name),
        quote_json(project_revision),
        quote_json(workspace_revision),
        quote_json(entry_module),
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
        ResolvedExprKind::Match { scrutinee, arms } => {
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
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_) => {}
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
                        (ResolvedType::I64, BinaryOp::Eq) => 0x51,
                        (ResolvedType::I64, BinaryOp::Ne) => 0x52,
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
        ResolvedExprKind::Match { scrutinee, arms } => {
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
        crate::hir::ResolvedMatchPattern::Wildcard | crate::hir::ResolvedMatchPattern::Binding(_) => {
            output.extend_bytes(&[0x41, 0x01]); // i32.const 1
            return Ok(());
        }
        crate::hir::ResolvedMatchPattern::Variant { .. } | crate::hir::ResolvedMatchPattern::Record { .. } => {
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
            ResolvedExprKind::Match { scrutinee, arms } => {
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
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_) => {}
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
    match &expression.kind {
        ResolvedExprKind::Binary { op, left, right: _ }
            if matches!(left.ty, ResolvedType::U8)
                && matches!(
                    *op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                ) =>
        {
            true
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            contains_u8_arithmetic(left) || contains_u8_arithmetic(right)
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => contains_u8_arithmetic(value),
        ResolvedExprKind::Call { args, .. } => args.iter().any(contains_u8_arithmetic),
        ResolvedExprKind::NativeRustImportCall(call) => {
            call.args.iter().any(contains_u8_arithmetic)
        }
        ResolvedExprKind::Block { statements, tail } => {
            contains_u8_arithmetic(tail)
                || statements.iter().any(|statement| {
                    (0..statement.child_count())
                        .any(|index| statement.child(index).is_some_and(contains_u8_arithmetic))
                })
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            contains_u8_arithmetic(condition)
                || contains_u8_arithmetic(then_branch)
                || contains_u8_arithmetic(else_branch)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .any(|field| contains_u8_arithmetic(&field.value)),
        ResolvedExprKind::Match { scrutinee, arms } => {
            contains_u8_arithmetic(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| contains_u8_arithmetic(guard))
                        || contains_u8_arithmetic(&arm.value)
                })
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            contains_u8_arithmetic(base)
                || fields
                    .iter()
                    .any(|field| contains_u8_arithmetic(&field.value))
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_) => false,
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
        ResolvedType::F32 => Ok(F32),
        ResolvedType::F64 => Ok(F64),
        ResolvedType::Bool | ResolvedType::Nominal { .. } => Ok(I32),
        // Owned strings lower to an abstract host handle riding the i64 lane.
        ResolvedType::String => Ok(I64),
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
  if (Object.keys(SPX_OWNED_EXPORTS).length === 0) {
    return Object.freeze(await WebAssembly.instantiate(authenticatedBytes, imports));
  }
  const runtime = createOwnedRuntime(options);
  const linkedImports = { env: { ...imports.env, ...runtime.linkImports.env } };
  const result = await WebAssembly.instantiate(authenticatedBytes, linkedImports);
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
