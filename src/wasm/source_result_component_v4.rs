//! Closed source-language `Result` core for private Component Model v4 evidence.

use std::collections::{BTreeSet, HashMap};

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::graph;
use crate::hir::{
    self, DeclarationId, IdentityOrigin, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedProgram, ResolvedType,
};
use crate::prelude;
use crate::variant_layout::{VariantLayoutCache, VariantTarget};

use super::{
    aggregate, intern_type, section, write_bytes, write_i64, write_name, write_u32, Signature, I32,
    I64,
};

pub(crate) const FUNCTION_ID: &str = "component.evaluate";
const SOURCE_ID: &str = "component.source";
pub(crate) const STATUS_OUT_EXPORT: &str = "semaprax_evaluate_source_result_status_out";
pub(crate) const CANONICAL_EXPORT: &str = "cabi_evaluate_source_result";
const TEST_SELECTED_EXPORT: &str = "__spx_test_source_result_selected_v4";
const TEST_VALIDATE_EXPORT: &str = "__spx_test_validate_source_result_v4";
const CONTRACT_DOMAIN: &str = "semaprax.contract.v1";
const ARITHMETIC_DOMAIN: &str = "semaprax.arithmetic.v1";
const INTERNAL_RESULT_AREA: i32 = 128;
pub(crate) const RESULT_AREA: i32 = 256;
const POISON_I64: i64 = 0xa5a5_a5a5_a5a5_a5a5_u64 as i64;
const CUSTOM_SECTION: &str = "semaprax.component-source-result-v4";

const CONTRACT_REQUIRES: i32 = status_word(1, 1);
const CONTRACT_ENSURES: i32 = status_word(1, 2);
const ARITHMETIC_BASE: i32 = status_word(2, 0);

const fn status_word(class: i32, code: i32) -> i32 {
    (class << 24) | code
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateSourceResultCoreArtifactV4 {
    pub(crate) bytes: Vec<u8>,
    pub(crate) source_revision: String,
    pub(crate) result_i64_bool_layout_digest: [u8; 32],
    pub(crate) result_bool_bool_layout_digest: [u8; 32],
    pub(crate) prelude_digest: [u8; 32],
}

pub(crate) fn emit_private_source_result_core_v4(
    program: &Program,
) -> Result<PrivateSourceResultCoreArtifactV4, Diagnostic> {
    emit_profile(program, false)
}

#[cfg(test)]
fn emit_test_profile(program: &Program) -> Result<PrivateSourceResultCoreArtifactV4, Diagnostic> {
    emit_profile(program, true)
}

fn emit_profile(
    program: &Program,
    test_exports: bool,
) -> Result<PrivateSourceResultCoreArtifactV4, Diagnostic> {
    let resolved = hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|diagnostic| diagnostic.severity.is_error())
            .unwrap_or_else(|| Diagnostic::io("SPX-WIT108", "source-result HIR resolution failed"))
    })?;
    hir::validate(&resolved)?;
    let ordered_closure = validate_profile(&resolved)?;
    let result_i64_bool = result_type(ResolvedType::I64, ResolvedType::Bool);
    let result_bool_bool = result_type(ResolvedType::Bool, ResolvedType::Bool);
    let layouts = VariantLayoutCache::build(&resolved, VariantTarget::Wasm32)?;
    let result_i64_bool_layout = layouts.layout(&result_i64_bool)?;
    let result_bool_bool_layout = layouts.layout(&result_bool_bool)?;
    require_layout(result_i64_bool_layout, &result_i64_bool, 16, 8, 8)?;
    require_layout(result_bool_bool_layout, &result_bool_bool, 8, 4, 4)?;
    let result_i64_bool_layout_digest = result_i64_bool_layout.digest();
    let result_bool_bool_layout_digest = result_bool_bool_layout.digest();
    let prelude_digest = prelude::digest_v1();
    let source_revision = graph::revision(program);
    let selected = DeclarationId::new(FUNCTION_ID);
    let lowering = aggregate::lower_selected_functions(&resolved, &ordered_closure, &selected)?;
    let bytes = compose(
        lowering,
        &source_revision,
        prelude_digest,
        result_i64_bool_layout_digest,
        result_bool_bool_layout_digest,
        test_exports,
    )?;
    Ok(PrivateSourceResultCoreArtifactV4 {
        bytes,
        source_revision,
        result_i64_bool_layout_digest,
        result_bool_bool_layout_digest,
        prelude_digest,
    })
}

fn result_type(ok: ResolvedType, err: ResolvedType) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new("core.result"),
        arguments: vec![ok, err],
    }
}

fn require_layout(
    layout: &crate::variant_layout::VariantLayout,
    expected_type: &ResolvedType,
    size: u32,
    align: u32,
    payload_offset: u32,
) -> Result<(), Diagnostic> {
    if layout.instance != *expected_type
        || layout.variant != DeclarationId::new("core.result")
        || layout.size != size
        || layout.align != align
        || layout.tag_size != 4
        || layout.payload_offset != payload_offset
        || layout.cases.len() != 2
        || layout.cases[0].case != DeclarationId::new("core.result.ok")
        || layout.cases[0].tag != 0
        || layout.cases[1].case != DeclarationId::new("core.result.err")
        || layout.cases[1].tag != 1
    {
        return Err(profile_error(
            "source-result Wasm32 layout-v2 binding changed",
        ));
    }
    Ok(())
}

fn validate_profile(program: &ResolvedProgram) -> Result<Vec<DeclarationId>, Diagnostic> {
    if !program.interfaces.is_empty()
        || program.types.iter().any(|declaration| {
            !program
                .declarations
                .declaration(&declaration.id)
                .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned)
        })
    {
        return Err(profile_error(
            "source-result v4 admits no interfaces, resources, or user nominal types",
        ));
    }
    let source_id = DeclarationId::new(SOURCE_ID);
    let selected_id = DeclarationId::new(FUNCTION_ID);
    let source = function(program, &source_id)?;
    let selected = function(program, &selected_id)?;
    require_function_signature(
        source,
        &[ResolvedType::I64, ResolvedType::Bool],
        &result_type(ResolvedType::I64, ResolvedType::Bool),
        "source helper",
    )?;
    require_function_signature(
        selected,
        &[ResolvedType::I64, ResolvedType::Bool, ResolvedType::I64],
        &result_type(ResolvedType::Bool, ResolvedType::Bool),
        "selected evaluate function",
    )?;
    let mut visiting = BTreeSet::new();
    let mut reachable = BTreeSet::new();
    collect_reachable(program, &selected_id, &mut visiting, &mut reachable)?;
    let expected = BTreeSet::from([source_id.clone(), selected_id.clone()]);
    if reachable != expected {
        return Err(profile_error(
            "source-result v4 requires the exact source/evaluate reachable closure",
        ));
    }
    for id in &reachable {
        let function = function(program, id)?;
        if !function.effects.is_empty() {
            return Err(profile_error(
                "source-result v4 reachable functions must be effect-free",
            ));
        }
        for contract in &function.requires {
            validate_expr(program, contract)?;
        }
        validate_expr(program, &function.body)?;
        for contract in &function.ensures {
            validate_expr(program, contract)?;
        }
    }
    let ordered = program
        .functions
        .iter()
        .filter(|function| reachable.contains(&function.id))
        .map(|function| function.id.clone())
        .collect::<Vec<_>>();
    if ordered.len() != 2 || ordered[0] != source_id || ordered[1] != selected_id {
        return Err(profile_error(
            "source-result v4 requires source before evaluate in declaration order",
        ));
    }
    Ok(ordered)
}

fn function<'a>(
    program: &'a ResolvedProgram,
    id: &DeclarationId,
) -> Result<&'a ResolvedFunction, Diagnostic> {
    program
        .functions
        .iter()
        .find(|function| function.id == *id)
        .ok_or_else(|| profile_error(format!("source-result v4 requires `{id}`")))
}

fn require_function_signature(
    function: &ResolvedFunction,
    params: &[ResolvedType],
    result: &ResolvedType,
    context: &str,
) -> Result<(), Diagnostic> {
    if function.params.len() != params.len()
        || function
            .params
            .iter()
            .zip(params)
            .any(|(actual, expected)| actual.ty != *expected)
        || function.return_type != *result
    {
        return Err(profile_error(format!(
            "{context} has an incompatible signature"
        )));
    }
    Ok(())
}

fn collect_reachable(
    program: &ResolvedProgram,
    id: &DeclarationId,
    visiting: &mut BTreeSet<DeclarationId>,
    reachable: &mut BTreeSet<DeclarationId>,
) -> Result<(), Diagnostic> {
    if reachable.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.clone()) {
        return Err(profile_error(
            "source-result v4 reachable closure must be acyclic",
        ));
    }
    let function = function(program, id)?;
    let mut calls = Vec::new();
    for contract in &function.requires {
        collect_calls(contract, &mut calls);
    }
    collect_calls(&function.body, &mut calls);
    for contract in &function.ensures {
        collect_calls(contract, &mut calls);
    }
    for callee in calls {
        collect_reachable(program, &callee, visiting, reachable)?;
    }
    visiting.remove(id);
    reachable.insert(id.clone());
    Ok(())
}

fn collect_calls(expr: &ResolvedExpr, output: &mut Vec<DeclarationId>) {
    walk_children(expr, |child| collect_calls(child, output));
    if let ResolvedExprKind::Call { callee, .. } = &expr.kind {
        output.push(callee.clone());
    }
}

fn validate_expr(program: &ResolvedProgram, expr: &ResolvedExpr) -> Result<(), Diagnostic> {
    validate_type(program, &expr.ty)?;
    match &expr.kind {
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::Unary { .. }
        | ResolvedExprKind::Binary { .. }
        | ResolvedExprKind::Block { .. }
        | ResolvedExprKind::If { .. }
        | ResolvedExprKind::Call { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::ConstructVariant { .. } => {}
        _ => {
            return Err(profile_error(
                "source-result v4 admits only direct-copy Result/? expressions",
            ));
        }
    }
    let mut result = Ok(());
    walk_children(expr, |child| {
        if result.is_ok() {
            result = validate_expr(program, child);
        }
    });
    result
}

fn validate_type(program: &ResolvedProgram, ty: &ResolvedType) -> Result<(), Diagnostic> {
    match ty {
        ResolvedType::I64 | ResolvedType::Bool => Ok(()),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if declaration == &DeclarationId::new("core.result")
            && arguments.len() == 2
            && arguments
                .iter()
                .all(|argument| matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
            && program
                .declarations
                .declaration(declaration)
                .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned) =>
        {
            Ok(())
        }
        _ => Err(profile_error(format!(
            "source-result v4 excludes type `{}`",
            ty.identity_key()
        ))),
    }
}

fn walk_children(expr: &ResolvedExpr, mut visit: impl FnMut(&ResolvedExpr)) {
    match &expr.kind {
        ResolvedExprKind::Call { args, .. } => args.iter().for_each(&mut visit),
        ResolvedExprKind::Unary { value, .. } => visit(value),
        ResolvedExprKind::Try { operand, .. } => visit(operand),
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
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visit(condition);
            visit(then_branch);
            visit(else_branch);
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                visit(&field.value);
            }
        }
        _ => {}
    }
}

fn compose(
    lowering: aggregate::SelectedAggregateLowering,
    source_revision: &str,
    prelude_digest: [u8; 32],
    result_i64_bool_layout_digest: [u8; 32],
    result_bool_bool_layout_digest: [u8; 32],
    test_exports: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let aggregate::SelectedAggregateLowering {
        mut types,
        function_type_indexes,
        mut bodies,
        selected_index,
    } = lowering;
    let mut type_indexes = types
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, signature)| (signature, u32::try_from(index).unwrap_or(u32::MAX)))
        .collect::<HashMap<_, _>>();
    let validate_type = intern_type(
        Signature {
            params: vec![I32, I32],
            results: vec![I32],
        },
        &mut types,
        &mut type_indexes,
    );
    let status_out_type = intern_type(
        Signature {
            params: vec![I64, I32, I64, I32],
            results: vec![I32],
        },
        &mut types,
        &mut type_indexes,
    );
    let canonical_type = intern_type(
        Signature {
            params: vec![I64, I32, I64],
            results: vec![I32],
        },
        &mut types,
        &mut type_indexes,
    );
    let validate_index = u32::try_from(bodies.len())
        .map_err(|_| profile_error("source-result function index overflows u32"))?;
    let status_out_index = validate_index
        .checked_add(1)
        .ok_or_else(|| profile_error("source-result function index overflows u32"))?;
    let canonical_index = status_out_index
        .checked_add(1)
        .ok_or_else(|| profile_error("source-result function index overflows u32"))?;
    bodies.push(validate_copy_body());
    bodies.push(status_out_body(selected_index, validate_index));
    bodies.push(canonical_adapter_body(status_out_index));

    let mut module = b"\0asm\x01\0\0\0".to_vec();
    let mut type_section = Vec::new();
    write_u32(&mut type_section, types.len() as u32);
    for signature in &types {
        type_section.push(0x60);
        write_bytes(&mut type_section, &signature.params);
        write_bytes(&mut type_section, &signature.results);
    }
    section(&mut module, 1, type_section);

    let mut function_section = Vec::new();
    write_u32(
        &mut function_section,
        u32::try_from(function_type_indexes.len() + 3)
            .map_err(|_| profile_error("source-result function count overflows u32"))?,
    );
    for index in function_type_indexes {
        write_u32(&mut function_section, index);
    }
    for index in [validate_type, status_out_type, canonical_type] {
        write_u32(&mut function_section, index);
    }
    section(&mut module, 3, function_section);

    section(&mut module, 5, vec![0x01, 0x00, 0x01]);
    let mut globals = vec![0x01, I32, 0x01, 0x41];
    write_i64(&mut globals, i64::from(aggregate::SHADOW_STACK_TOP));
    globals.push(0x0b);
    section(&mut module, 6, globals);

    let mut exports = Vec::new();
    write_u32(&mut exports, if test_exports { 5 } else { 3 });
    write_name(&mut exports, "memory");
    exports.extend([0x02, 0x00]);
    write_name(&mut exports, STATUS_OUT_EXPORT);
    exports.push(0x00);
    write_u32(&mut exports, status_out_index);
    write_name(&mut exports, CANONICAL_EXPORT);
    exports.push(0x00);
    write_u32(&mut exports, canonical_index);
    if test_exports {
        write_name(&mut exports, TEST_SELECTED_EXPORT);
        exports.push(0x00);
        write_u32(&mut exports, selected_index);
        write_name(&mut exports, TEST_VALIDATE_EXPORT);
        exports.push(0x00);
        write_u32(&mut exports, validate_index);
    }
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
    custom.extend(prelude_digest);
    custom.extend(result_i64_bool_layout_digest);
    custom.extend(result_bool_bool_layout_digest);
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

fn validate_copy_body() -> Vec<u8> {
    let tag = 2_u32;
    let payload = 3_u32;
    let mut body = vec![0x01, 0x02, I32];
    body.extend([0x20, 0x00, 0x28, 0x02, 0x00, 0x22]);
    write_u32(&mut body, tag);
    body.extend([0x41, 0x02, 0x4f, 0x04, 0x40, 0x41]);
    write_i64(&mut body, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
    body.extend([0x0f, 0x0b, 0x20, 0x00, 0x28, 0x02, 0x04, 0x22]);
    write_u32(&mut body, payload);
    body.extend([0x41, 0x01, 0x4b, 0x04, 0x40, 0x41]);
    write_i64(&mut body, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
    body.extend([0x0f, 0x0b]);
    body.extend([0x20, 0x01, 0x41, 0x04, 0x6a, 0x20]);
    write_u32(&mut body, payload);
    body.extend([0x36, 0x02, 0x00, 0x20, 0x01, 0x20]);
    write_u32(&mut body, tag);
    body.extend([0x36, 0x02, 0x00, 0x41, 0x00, 0x0b]);
    body
}

fn status_out_body(selected_index: u32, validate_index: u32) -> Vec<u8> {
    let status = 4_u32;
    let mut body = vec![0x01, 0x01, I32];
    body.extend([0x20, 0x01, 0x41, 0x01, 0x4b, 0x04, 0x40, 0x41]);
    write_i64(&mut body, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
    body.extend([0x0f, 0x0b]);
    body.extend([0x20, 0x00, 0x20, 0x01, 0x20, 0x02, 0x41]);
    write_i64(&mut body, i64::from(INTERNAL_RESULT_AREA));
    body.push(0x10);
    write_u32(&mut body, selected_index);
    body.push(0x21);
    write_u32(&mut body, status);
    body.push(0x20);
    write_u32(&mut body, status);
    body.extend([0x45, 0x04, I32, 0x41]);
    write_i64(&mut body, i64::from(INTERNAL_RESULT_AREA));
    body.push(0x20);
    write_u32(&mut body, 3);
    body.push(0x10);
    write_u32(&mut body, validate_index);
    body.push(0x05);
    emit_normalized_status(&mut body, status);
    body.extend([0x0b, 0x0b]);
    body
}

fn emit_normalized_status(output: &mut Vec<u8>, status: u32) {
    output.push(0x20);
    write_u32(output, status);
    output.push(0x41);
    write_i64(output, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
    output.extend([0x46, 0x04, I32, 0x41]);
    write_i64(output, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
    output.extend([0x05, 0x20]);
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
    output.extend([0x0b, 0x0b, 0x0b, 0x0b]);
}

fn canonical_adapter_body(status_out_index: u32) -> Vec<u8> {
    let status = 3_u32;
    let mut body = vec![0x01, 0x01, I32];
    for offset in [RESULT_AREA, RESULT_AREA + 8, RESULT_AREA + 16] {
        body.push(0x41);
        write_i64(&mut body, i64::from(offset));
        body.push(0x42);
        write_i64(&mut body, POISON_I64);
        body.extend([0x37, 0x03, 0x00]);
    }
    body.extend([0x20, 0x00, 0x20, 0x01, 0x20, 0x02, 0x41]);
    write_i64(&mut body, i64::from(INTERNAL_RESULT_AREA));
    body.push(0x10);
    write_u32(&mut body, status_out_index);
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
    // Outer Ok: publish payload, inner tag, then outer tag.
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA + 5));
    body.push(0x41);
    write_i64(&mut body, i64::from(INTERNAL_RESULT_AREA + 4));
    body.extend([0x28, 0x02, 0x00, 0x3a, 0x00, 0x00]);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA + 4));
    body.push(0x41);
    write_i64(&mut body, i64::from(INTERNAL_RESULT_AREA));
    body.extend([0x28, 0x02, 0x00, 0x3a, 0x00, 0x00]);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA));
    body.extend([0x41, 0x00, 0x3a, 0x00, 0x00, 0x05]);
    // Outer Err: publish normalized status fields, then the outer tag.
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA + 4));
    body.push(0x20);
    write_u32(&mut body, status);
    body.extend([
        0x41, 0x18, 0x76, 0x41, 0x01, 0x46, 0x04, I32, 0x41, 0x00, 0x05, 0x41,
    ]);
    write_i64(&mut body, 32);
    body.extend([0x0b, 0x36, 0x02, 0x00]);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA + 8));
    body.push(0x20);
    write_u32(&mut body, status);
    body.extend([0x41, 0x18, 0x76, 0x41, 0x01, 0x46, 0x04, I32, 0x41]);
    write_i64(&mut body, CONTRACT_DOMAIN.len() as i64);
    body.extend([0x05, 0x41]);
    write_i64(&mut body, ARITHMETIC_DOMAIN.len() as i64);
    body.extend([0x0b, 0x36, 0x02, 0x00]);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA + 12));
    body.push(0x20);
    write_u32(&mut body, status);
    body.push(0x41);
    write_i64(&mut body, 0x00ff_ffff);
    body.extend([0x71, 0x36, 0x02, 0x00]);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA + 16));
    body.push(0x20);
    write_u32(&mut body, status);
    body.extend([0x41, 0x18, 0x76, 0x3a, 0x00, 0x00]);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA + 17));
    body.extend([0x41, 0x01, 0x3a, 0x00, 0x00]);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA + 18));
    body.extend([0x41, 0x00, 0x3a, 0x00, 0x00]);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA));
    body.extend([0x41, 0x01, 0x3a, 0x00, 0x00, 0x0b]);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA));
    body.push(0x0b);
    body
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-WIT108", message)
}

#[cfg(test)]
#[path = "source_result_component_v4/tests.rs"]
mod tests;
