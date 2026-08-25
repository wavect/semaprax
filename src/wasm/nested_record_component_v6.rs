//! Closed nested direct-scalar record core for private Component Model v6 evidence.

use std::collections::HashMap;

use crate::aggregate_layout::{AggregateLayout, AggregateTarget};
use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::graph;
use crate::hir::{
    self, DeclarationId, IdentityOrigin, ResolvedExpr, ResolvedExprKind, ResolvedProgram,
    ResolvedType, ResolvedTypeDeclarationKind,
};

use super::{
    aggregate, intern_type, section, write_bytes, write_i64, write_name, write_u32, Signature, I32,
    I64,
};

const FUNCTION_ID: &str = "component.transform";
const INNER_ID: &str = "component.inner";
const OUTER_ID: &str = "component.outer";
pub(crate) const CANONICAL_EXPORT: &str = "cabi_transform_nested_record_v6";
const CONTRACT_DOMAIN: &str = "semaprax.contract.v1";
const ARITHMETIC_DOMAIN: &str = "semaprax.arithmetic.v1";
const INPUT_AREA: i32 = 128;
const INTERNAL_RESULT_AREA: i32 = 192;
pub(crate) const RESULT_AREA: i32 = 256;
const POISON_I64: i64 = 0xa5a5_a5a5_a5a5_a5a5_u64 as i64;
const CUSTOM_SECTION: &str = "semaprax.component-nested-record-v6";

const CONTRACT_REQUIRES: i32 = status_word(1, 1);
const CONTRACT_ENSURES: i32 = status_word(1, 2);
const ARITHMETIC_BASE: i32 = status_word(2, 0);

const fn status_word(class: i32, code: i32) -> i32 {
    (class << 24) | code
}

pub(crate) const SOURCE_V6: &str = r#"module test.component_nested_record_v6;

@id("component.inner")
record Inner {
    @id("component.inner.value") value: i64,
    @id("component.inner.flag") flag: bool,
}

@id("component.outer")
record Outer {
    @id("component.outer.inner") inner: Inner,
    @id("component.outer.other") other: i64,
}

@id("component.transform")
fn transform(input: Outer, delta: i64) -> Outer
    requires delta != -99
    ensures delta != 13
{
    input with {
        inner: input.inner with { value: input.inner.value + delta },
        other: input.other / (delta - 1),
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateNestedRecordCoreArtifactV6 {
    pub(crate) bytes: Vec<u8>,
    pub(crate) source_revision: String,
    pub(crate) inner_layout_digest: [u8; 32],
    pub(crate) outer_layout_digest: [u8; 32],
}

pub(crate) fn emit_private_nested_record_core_v6(
    program: &Program,
) -> Result<PrivateNestedRecordCoreArtifactV6, Diagnostic> {
    require_exact_source(program)?;
    let resolved = hir::resolve(program).map_err(first_error)?;
    hir::validate(&resolved)?;
    validate_profile(&resolved)?;
    let inner = AggregateLayout::for_record(
        &resolved,
        AggregateTarget::Wasm32,
        &DeclarationId::new(INNER_ID),
    )?;
    let outer = AggregateLayout::for_record(
        &resolved,
        AggregateTarget::Wasm32,
        &DeclarationId::new(OUTER_ID),
    )?;
    require_layouts(&inner, &outer)?;
    let inner_layout_digest = inner.digest();
    let outer_layout_digest = outer.digest();
    let selected = DeclarationId::new(FUNCTION_ID);
    let lowering =
        aggregate::lower_selected_functions(&resolved, std::slice::from_ref(&selected), &selected)?;
    let source_revision = graph::revision(program);
    let bytes = compose(
        lowering,
        &source_revision,
        inner_layout_digest,
        outer_layout_digest,
    )?;
    Ok(PrivateNestedRecordCoreArtifactV6 {
        bytes,
        source_revision,
        inner_layout_digest,
        outer_layout_digest,
    })
}

fn require_exact_source(program: &Program) -> Result<(), Diagnostic> {
    let expected = crate::parse(
        SOURCE_V6,
        std::path::Path::new("nested-record-v6-profile.spx"),
    )?;
    if crate::format::canonical(program) != crate::format::canonical(&expected) {
        return Err(profile_error(
            "nested-record v6 requires the exact frozen source semantics",
        ));
    }
    Ok(())
}

fn validate_profile(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    if !program.permits.is_empty() || !program.interfaces.is_empty() || program.functions.len() != 2
    {
        return Err(profile_error(
            "nested-record v6 requires exactly transform and app.main without authority",
        ));
    }
    let authored = program
        .types
        .iter()
        .filter(|declaration| {
            !program
                .declarations
                .declaration(&declaration.id)
                .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned)
        })
        .collect::<Vec<_>>();
    if authored.len() != 2
        || authored[0].id != DeclarationId::new(INNER_ID)
        || authored[1].id != DeclarationId::new(OUTER_ID)
        || !matches!(authored[0].kind, ResolvedTypeDeclarationKind::Record { .. })
        || !matches!(authored[1].kind, ResolvedTypeDeclarationKind::Record { .. })
    {
        return Err(profile_error(
            "nested-record v6 requires the exact Inner/Outer record table",
        ));
    }
    let transform = &program.functions[0];
    if transform.id != DeclarationId::new(FUNCTION_ID)
        || transform.params.len() != 2
        || transform.params[0].ty
            != (ResolvedType::Nominal {
                declaration: DeclarationId::new(OUTER_ID),
                arguments: Vec::new(),
            })
        || transform.params[1].ty != ResolvedType::I64
        || transform.return_type != transform.params[0].ty
        || !transform.effects.is_empty()
        || transform.requires.len() != 1
        || transform.ensures.len() != 1
    {
        return Err(profile_error(
            "nested-record v6 transform signature, contracts, or effects changed",
        ));
    }
    for expression in transform
        .requires
        .iter()
        .chain(std::iter::once(&transform.body))
        .chain(transform.ensures.iter())
    {
        validate_expr(expression)?;
    }
    let main = &program.functions[1];
    if main.id != DeclarationId::new("app.main")
        || !main.params.is_empty()
        || main.return_type != ResolvedType::I64
        || !main.effects.is_empty()
        || !main.requires.is_empty()
        || !main.ensures.is_empty()
    {
        return Err(profile_error("nested-record v6 app.main shape changed"));
    }
    validate_expr(&main.body)
}

fn validate_expr(expression: &ResolvedExpr) -> Result<(), Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::Unary { .. }
        | ResolvedExprKind::Binary { .. }
        | ResolvedExprKind::Block { .. }
        | ResolvedExprKind::UpdateRecord { .. }
        | ResolvedExprKind::Project { .. } => {}
        _ => {
            return Err(profile_error(
                "nested-record v6 admits only closed scalar record-update expressions",
            ));
        }
    }
    let mut result = Ok(());
    walk_children(expression, |child| {
        if result.is_ok() {
            result = validate_expr(child);
        }
    });
    result
}

fn walk_children(expression: &ResolvedExpr, mut visit: impl FnMut(&ResolvedExpr)) {
    match &expression.kind {
        ResolvedExprKind::Unary { value, .. } => visit(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            visit(left);
            visit(right);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                for index in 0..statement.child_count() {
                    visit(
                        statement
                            .child(index)
                            .expect("resolved statement child count is canonical"),
                    );
                }
            }
            visit(tail);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            visit(base);
            for field in fields {
                visit(&field.value);
            }
        }
        ResolvedExprKind::Project { base, .. } => visit(base),
        _ => {}
    }
}

fn require_layouts(inner: &AggregateLayout, outer: &AggregateLayout) -> Result<(), Diagnostic> {
    let inner_fields = [
        ("component.inner.value", 0_u32, 8_u32, 8_u32),
        ("component.inner.flag", 8, 4, 4),
    ];
    let outer_fields = [
        ("component.outer.inner", 0_u32, 16_u32, 8_u32),
        ("component.outer.other", 16, 8, 8),
    ];
    if inner.record != DeclarationId::new(INNER_ID)
        || inner.size != 16
        || inner.align != 8
        || inner.fields.len() != 2
        || inner.fields[0].ty != ResolvedType::I64
        || inner.fields[1].ty != ResolvedType::Bool
        || inner
            .fields
            .iter()
            .zip(inner_fields)
            .any(|(field, expected)| {
                (field.field.as_str(), field.offset, field.size, field.align) != expected
            })
        || outer.record != DeclarationId::new(OUTER_ID)
        || outer.size != 24
        || outer.align != 8
        || outer.fields.len() != 2
        || outer.fields[0].ty
            != (ResolvedType::Nominal {
                declaration: DeclarationId::new(INNER_ID),
                arguments: Vec::new(),
            })
        || outer.fields[1].ty != ResolvedType::I64
        || outer
            .fields
            .iter()
            .zip(outer_fields)
            .any(|(field, expected)| {
                (field.field.as_str(), field.offset, field.size, field.align) != expected
            })
    {
        return Err(profile_error(
            "nested-record v6 Wasm32 layout binding changed",
        ));
    }
    Ok(())
}

fn compose(
    lowering: aggregate::SelectedAggregateLowering,
    source_revision: &str,
    inner_layout_digest: [u8; 32],
    outer_layout_digest: [u8; 32],
) -> Result<Vec<u8>, Diagnostic> {
    let aggregate::SelectedAggregateLowering {
        mut types,
        function_type_indexes,
        mut bodies,
        selected_index,
    } = lowering;
    if bodies.len() != 1 || selected_index != 0 || function_type_indexes.len() != 1 {
        return Err(profile_error("nested-record v6 selected closure changed"));
    }
    let mut type_indexes = types
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, signature)| (signature, index as u32))
        .collect::<HashMap<_, _>>();
    let canonical_type = intern_type(
        Signature {
            params: vec![I64, I32, I64, I64],
            results: vec![I32],
        },
        &mut types,
        &mut type_indexes,
    );
    let canonical_index = u32::try_from(bodies.len())
        .map_err(|_| profile_error("nested-record v6 function index overflows u32"))?;
    bodies.push(canonical_adapter_body(selected_index));

    let mut module = b"\0asm\x01\0\0\0".to_vec();
    let mut type_section = Vec::new();
    write_u32(&mut type_section, types.len() as u32);
    for signature in &types {
        type_section.push(0x60);
        write_bytes(&mut type_section, &signature.params);
        write_bytes(&mut type_section, &signature.results);
    }
    section(&mut module, 1, type_section);
    let mut functions = Vec::new();
    write_u32(&mut functions, 2);
    write_u32(&mut functions, function_type_indexes[0]);
    write_u32(&mut functions, canonical_type);
    section(&mut module, 3, functions);
    section(&mut module, 5, vec![0x01, 0x00, 0x01]);
    let mut globals = vec![0x01, I32, 0x01, 0x41];
    write_i64(&mut globals, i64::from(aggregate::SHADOW_STACK_TOP));
    globals.push(0x0b);
    section(&mut module, 6, globals);
    let mut exports = Vec::new();
    write_u32(&mut exports, 2);
    write_name(&mut exports, "memory");
    exports.extend([0x02, 0x00]);
    write_name(&mut exports, CANONICAL_EXPORT);
    exports.push(0x00);
    write_u32(&mut exports, canonical_index);
    section(&mut module, 7, exports);
    let mut code = Vec::new();
    write_u32(&mut code, bodies.len() as u32);
    for body in bodies {
        write_bytes(&mut code, &body);
    }
    section(&mut module, 10, code);
    let mut data = Vec::new();
    write_u32(&mut data, 2);
    active_data(&mut data, 0, CONTRACT_DOMAIN.as_bytes());
    active_data(&mut data, 32, ARITHMETIC_DOMAIN.as_bytes());
    section(&mut module, 11, data);
    let mut custom = Vec::new();
    write_name(&mut custom, CUSTOM_SECTION);
    write_name(&mut custom, source_revision);
    custom.extend(inner_layout_digest);
    custom.extend(outer_layout_digest);
    section(&mut module, 0, custom);
    Ok(module)
}

fn active_data(output: &mut Vec<u8>, offset: i32, bytes: &[u8]) {
    output.push(0x00);
    output.push(0x41);
    write_u32(output, offset as u32);
    output.push(0x0b);
    write_bytes(output, bytes);
}

fn canonical_adapter_body(selected_index: u32) -> Vec<u8> {
    let status = 4_u32;
    let flag = 5_u32;
    let mut body = vec![0x01, 0x02, I32];
    for offset in [
        RESULT_AREA,
        RESULT_AREA + 8,
        RESULT_AREA + 16,
        RESULT_AREA + 24,
    ] {
        body.push(0x41);
        write_i64(&mut body, i64::from(offset));
        body.push(0x42);
        write_i64(&mut body, POISON_I64);
        body.extend([0x37, 0x03, 0x00]);
    }
    body.extend([0x20, 0x01, 0x41, 0x01, 0x4b, 0x04, 0x40, 0x00, 0x0b]);
    store_i64_param(&mut body, INPUT_AREA, 0);
    store_i32_param(&mut body, INPUT_AREA + 8, 1);
    store_i64_param(&mut body, INPUT_AREA + 16, 2);
    body.push(0x41);
    write_i64(&mut body, i64::from(INPUT_AREA));
    body.push(0x20);
    write_u32(&mut body, 3);
    body.push(0x41);
    write_i64(&mut body, i64::from(INTERNAL_RESULT_AREA));
    body.push(0x10);
    write_u32(&mut body, selected_index);
    body.push(0x21);
    write_u32(&mut body, status);
    body.push(0x20);
    write_u32(&mut body, status);
    body.push(0x41);
    write_i64(&mut body, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
    body.extend([0x46, 0x04, 0x40, 0x00, 0x0b]);
    body.push(0x20);
    write_u32(&mut body, status);
    body.extend([0x45, 0x04, 0x40]);
    body.push(0x41);
    write_i64(&mut body, i64::from(INTERNAL_RESULT_AREA + 8));
    body.extend([0x28, 0x02, 0x00, 0x22]);
    write_u32(&mut body, flag);
    body.extend([0x41, 0x01, 0x4b, 0x04, 0x40, 0x00, 0x0b]);
    copy_i64(&mut body, INTERNAL_RESULT_AREA, RESULT_AREA + 8);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA + 16));
    body.push(0x20);
    write_u32(&mut body, flag);
    body.extend([0x3a, 0x00, 0x00]);
    copy_i64(&mut body, INTERNAL_RESULT_AREA + 16, RESULT_AREA + 24);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA));
    body.extend([0x41, 0x00, 0x3a, 0x00, 0x00, 0x05]);
    emit_normalized_status(&mut body, status);
    body.push(0x21);
    write_u32(&mut body, status);
    body.push(0x20);
    write_u32(&mut body, status);
    body.push(0x41);
    write_i64(&mut body, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
    body.extend([0x46, 0x04, 0x40, 0x00, 0x0b]);
    emit_status_fields(&mut body, status, RESULT_AREA + 8);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA));
    body.extend([0x41, 0x01, 0x3a, 0x00, 0x00, 0x0b]);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA));
    body.push(0x0b);
    body
}

fn store_i64_param(output: &mut Vec<u8>, address: i32, parameter: u32) {
    output.push(0x41);
    write_i64(output, i64::from(address));
    output.push(0x20);
    write_u32(output, parameter);
    output.extend([0x37, 0x03, 0x00]);
}

fn store_i32_param(output: &mut Vec<u8>, address: i32, parameter: u32) {
    output.push(0x41);
    write_i64(output, i64::from(address));
    output.push(0x20);
    write_u32(output, parameter);
    output.extend([0x36, 0x02, 0x00]);
}

fn copy_i64(output: &mut Vec<u8>, source: i32, target: i32) {
    output.push(0x41);
    write_i64(output, i64::from(target));
    output.push(0x41);
    write_i64(output, i64::from(source));
    output.extend([0x29, 0x03, 0x00, 0x37, 0x03, 0x00]);
}

fn emit_normalized_status(output: &mut Vec<u8>, status: u32) {
    output.push(0x20);
    write_u32(output, status);
    output.extend([0x41, 0x01, 0x4e, 0x20]);
    write_u32(output, status);
    output.push(0x41);
    write_i64(output, i64::from(aggregate::STATUS_NEG_OVERFLOW));
    output.extend([0x4c, 0x71, 0x04, I32, 0x41]);
    write_i64(output, i64::from(ARITHMETIC_BASE));
    output.push(0x20);
    write_u32(output, status);
    output.extend([0x72, 0x05, 0x20]);
    write_u32(output, status);
    output.push(0x41);
    write_i64(output, i64::from(aggregate::STATUS_REQUIRES_FALSE));
    output.extend([0x46, 0x04, I32, 0x41]);
    write_i64(output, i64::from(CONTRACT_REQUIRES));
    output.extend([0x05, 0x20]);
    write_u32(output, status);
    output.push(0x41);
    write_i64(output, i64::from(aggregate::STATUS_ENSURES_FALSE));
    output.extend([0x46, 0x04, I32, 0x41]);
    write_i64(output, i64::from(CONTRACT_ENSURES));
    output.extend([0x05, 0x41]);
    write_i64(output, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
    output.extend([0x0b, 0x0b, 0x0b]);
}

fn emit_status_fields(output: &mut Vec<u8>, status: u32, base: i32) {
    output.push(0x41);
    write_i64(output, i64::from(base));
    output.push(0x20);
    write_u32(output, status);
    output.extend([
        0x41, 0x18, 0x76, 0x41, 0x01, 0x46, 0x04, I32, 0x41, 0x00, 0x05, 0x41,
    ]);
    write_i64(output, 32);
    output.extend([0x0b, 0x36, 0x02, 0x00]);
    output.push(0x41);
    write_i64(output, i64::from(base + 4));
    output.push(0x20);
    write_u32(output, status);
    output.extend([0x41, 0x18, 0x76, 0x41, 0x01, 0x46, 0x04, I32, 0x41]);
    write_i64(output, CONTRACT_DOMAIN.len() as i64);
    output.extend([0x05, 0x41]);
    write_i64(output, ARITHMETIC_DOMAIN.len() as i64);
    output.extend([0x0b, 0x36, 0x02, 0x00]);
    output.push(0x41);
    write_i64(output, i64::from(base + 8));
    output.push(0x20);
    write_u32(output, status);
    output.push(0x41);
    write_i64(output, 0x00ff_ffff);
    output.extend([0x71, 0x36, 0x02, 0x00]);
    output.push(0x41);
    write_i64(output, i64::from(base + 12));
    output.push(0x20);
    write_u32(output, status);
    output.extend([0x41, 0x18, 0x76, 0x3a, 0x00, 0x00]);
    output.push(0x41);
    write_i64(output, i64::from(base + 13));
    output.extend([0x41, 0x01, 0x3a, 0x00, 0x00]);
    output.push(0x41);
    write_i64(output, i64::from(base + 14));
    output.extend([0x41, 0x00, 0x3a, 0x00, 0x00]);
}

fn first_error(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.severity.is_error())
        .unwrap_or_else(|| profile_error("nested-record v6 HIR resolution failed"))
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-WIT108", message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use sha2::{Digest, Sha256};

    use super::*;

    fn artifact() -> PrivateNestedRecordCoreArtifactV6 {
        let program = crate::parse(SOURCE_V6, Path::new("component-nested-record-v6.spx")).unwrap();
        emit_private_nested_record_core_v6(&program).unwrap()
    }

    #[test]
    fn deterministic_core_is_upstream_valid_and_layout_bound() {
        let first = artifact();
        assert_eq!(
            first.source_revision,
            "sha256:d1fcbc45b3d86fa1d7910378578828df3c557dba92f90ed9459f928c5bf2fe8a"
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&first.bytes)),
            [
                0x42, 0x83, 0x5d, 0xcb, 0xf9, 0x80, 0x78, 0xac, 0x24, 0xbf, 0xd3, 0x65, 0x68, 0xf1,
                0xb6, 0x91, 0x7b, 0x5b, 0x64, 0xca, 0x2d, 0x82, 0x65, 0xef, 0x4d, 0xed, 0x16, 0x1d,
                0x26, 0x43, 0x8d, 0xa1,
            ]
        );
        assert_eq!(
            first.inner_layout_digest,
            [
                0x18, 0x6a, 0x97, 0xe6, 0x59, 0xee, 0x80, 0xb6, 0x41, 0xbd, 0xe5, 0x66, 0xc9, 0x87,
                0x51, 0x22, 0xf8, 0xee, 0xa4, 0xea, 0x26, 0x5c, 0x3a, 0x1a, 0xf9, 0x7c, 0xfb, 0x11,
                0xbe, 0xf9, 0x8a, 0x87,
            ]
        );
        assert_eq!(
            first.outer_layout_digest,
            [
                0x48, 0x85, 0xc0, 0x35, 0x3c, 0xb0, 0x59, 0x28, 0x01, 0x8d, 0x35, 0x27, 0xa1, 0x3e,
                0x36, 0x3c, 0xbe, 0x10, 0xf3, 0xa0, 0xb9, 0xc4, 0xf5, 0xb0, 0xba, 0x15, 0x79, 0x20,
                0x97, 0xfb, 0xbe, 0x6f,
            ]
        );
        assert_eq!(first, artifact());
        assert_ne!(first.inner_layout_digest, first.outer_layout_digest);
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&first.bytes)
            .expect("pinned upstream validator rejected nested-record v6 core");
        assert_ne!(<[u8; 32]>::from(Sha256::digest(&first.bytes)), [0; 32]);
    }

    #[test]
    fn exact_source_semantic_mutations_reject() {
        for hostile in [
            SOURCE_V6.replacen("delta != -99", "delta != -98", 1),
            SOURCE_V6.replacen("delta != 13", "delta != 12", 1),
            SOURCE_V6.replacen("value + delta", "value - delta", 1),
            SOURCE_V6.replacen("delta - 1", "delta - 2", 1),
            SOURCE_V6.replacen("inner: input.inner", "other: input.other", 1),
        ] {
            let parsed = crate::parse(&hostile, Path::new("hostile-v6.spx"));
            match parsed {
                Ok(program) => assert!(emit_private_nested_record_core_v6(&program).is_err()),
                Err(error) => assert!(!error.code.is_empty()),
            }
        }
    }

    #[test]
    fn node_executes_success_status_precedence_poison_and_invalid_bool() {
        let artifact = artifact();
        let stem = format!("semaprax-nested-record-v6-{}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, artifact.bytes).unwrap();
        std::fs::write(
            &script_path,
            format!(
                "import fs from 'node:fs';\nconst {{instance}}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]));\nconst f=instance.exports.{CANONICAL_EXPORT};const m=new DataView(instance.exports.memory.buffer);\nconst call=(value,flag,other,delta)=>f(BigInt(value),flag,BigInt(other),BigInt(delta));\nlet p=call(18,1,22,2);if(m.getUint8(p)!==0||m.getBigInt64(p+8,true)!==20n||m.getUint8(p+16)!==1||m.getBigInt64(p+24,true)!==22n)throw new Error('success');\np=call(18,0,22,2);if(m.getUint8(p+16)!==0)throw new Error('false flag');\np=call(9223372036854775807n,1,22,1);if(m.getUint8(p)!==1||m.getUint32(p+16,true)!==1)throw new Error('sticky add');\np=call(18,1,22,1);if(m.getUint8(p)!==1||m.getUint32(p+16,true)!==4)throw new Error('divzero');\nlet trapped=false;try{{call(18,2,22,2);}}catch(_e){{trapped=true;}}if(!trapped)throw new Error('invalid bool');\nconsole.log('nested-record-v6-core-ok');\n"
            ),
        )
        .unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .expect("Node is required by the existing Wasm quality gate");
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        assert!(
            output.status.success(),
            "Node nested-record v6 gate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
