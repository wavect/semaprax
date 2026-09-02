//! Closed source-language `Option` propagation core for private Component Model v10 evidence.

use std::collections::HashMap;

use sha2::{Digest, Sha256};

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

pub(crate) const FUNCTION_ID: &str = "component.option-propagation.evaluate";
pub(crate) const STATUS_OUT_EXPORT: &str = "semaprax_evaluate_option_propagation_status_out";
pub(crate) const CANONICAL_EXPORT: &str = "cabi_evaluate_option_propagation_v10";
const TEST_SELECTED_EXPORT: &str = "__spx_test_option_propagation_selected_v10";
const TEST_VALIDATE_EXPORT: &str = "__spx_test_validate_option_bool_v10";
const CONTRACT_DOMAIN: &str = "semaprax.contract.v1";
const ARITHMETIC_DOMAIN: &str = "semaprax.arithmetic.v1";
const INTERNAL_RESULT_AREA: i32 = 128;
const INTERNAL_INPUT_AREA: i32 = 64;
pub(crate) const RESULT_AREA: i32 = 256;
const POISON_I64: i64 = 0xa5a5_a5a5_a5a5_a5a5_u64 as i64;
const CUSTOM_SECTION: &str = "semaprax.component-option-propagation-v10";
const PLAN_DOMAIN: &[u8] = b"semaprax.component-option-propagation-plan.v10\0";

pub(crate) const SOURCE_V10: &str = include_str!("../../platform-tests/component-runtime/v10.spx");

const CONTRACT_REQUIRES: i32 = status_word(1, 1);
const CONTRACT_ENSURES: i32 = status_word(1, 2);
const ARITHMETIC_BASE: i32 = status_word(2, 0);

const fn status_word(class: i32, code: i32) -> i32 {
    (class << 24) | code
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateOptionPropagationCoreArtifactV10 {
    pub(crate) bytes: Vec<u8>,
    pub(crate) source_revision: String,
    pub(crate) graph_digest: [u8; 32],
    pub(crate) option_i64_layout_digest: [u8; 32],
    pub(crate) option_bool_layout_digest: [u8; 32],
    pub(crate) plan_digest: [u8; 32],
    pub(crate) prelude_digest: [u8; 32],
}

struct ProfileRoots<'a> {
    source_revision: &'a str,
    graph_digest: [u8; 32],
    prelude_digest: [u8; 32],
    option_i64_layout_digest: [u8; 32],
    option_bool_layout_digest: [u8; 32],
    plan_digest: [u8; 32],
}

pub(crate) fn emit_private_option_propagation_core_v10(
    program: &Program,
) -> Result<PrivateOptionPropagationCoreArtifactV10, Diagnostic> {
    require_exact_source(program)?;
    emit_profile(program, false)
}

#[cfg(test)]
fn emit_test_profile(
    program: &Program,
) -> Result<PrivateOptionPropagationCoreArtifactV10, Diagnostic> {
    emit_profile(program, true)
}

fn emit_profile(
    program: &Program,
    test_exports: bool,
) -> Result<PrivateOptionPropagationCoreArtifactV10, Diagnostic> {
    let resolved = hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|diagnostic| diagnostic.severity.is_error())
            .unwrap_or_else(|| {
                Diagnostic::io("SPX-WIT108", "option-propagation HIR resolution failed")
            })
    })?;
    hir::validate(&resolved)?;
    let ordered_closure = validate_profile(&resolved)?;
    let option_i64 = option_type(ResolvedType::I64);
    let option_bool = option_type(ResolvedType::Bool);
    let layouts = VariantLayoutCache::build(&resolved, VariantTarget::Wasm32)?;
    let option_i64_layout = layouts.layout(&option_i64)?;
    let option_bool_layout = layouts.layout(&option_bool)?;
    require_layout(option_i64_layout, &option_i64, 16, 8, 8)?;
    require_layout(option_bool_layout, &option_bool, 8, 4, 4)?;
    let option_i64_layout_digest = option_i64_layout.digest();
    let option_bool_layout_digest = option_bool_layout.digest();
    let prelude_digest = prelude::digest_v1();
    let source_revision = graph::revision(program);
    let graph_json = graph::to_json(program).map_err(first_error)?;
    if !graph_json.starts_with("{\"schema\":\"semaprax.graph.v11\",") {
        return Err(profile_error(
            "option-propagation v10 requires exact Graph v11",
        ));
    }
    let graph_digest = Sha256::digest(graph_json.as_bytes()).into();
    let selected_function = function(&resolved, &DeclarationId::new(FUNCTION_ID))?;
    let plan_json = crate::graph_cleanup::cleanup_plan_json(&selected_function.cleanup_plan);
    let plan_digest = plan_digest(&plan_json);
    let selected = DeclarationId::new(FUNCTION_ID);
    let lowering = aggregate::lower_selected_functions(&resolved, &ordered_closure, &selected)?;
    let bytes = compose(
        lowering,
        ProfileRoots {
            source_revision: &source_revision,
            graph_digest,
            prelude_digest,
            option_i64_layout_digest,
            option_bool_layout_digest,
            plan_digest,
        },
        test_exports,
    )?;
    Ok(PrivateOptionPropagationCoreArtifactV10 {
        bytes,
        source_revision,
        graph_digest,
        option_i64_layout_digest,
        option_bool_layout_digest,
        plan_digest,
        prelude_digest,
    })
}

fn option_type(value: ResolvedType) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(prelude::OPTION_ID),
        arguments: vec![value],
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
        || layout.variant != DeclarationId::new(prelude::OPTION_ID)
        || layout.size != size
        || layout.align != align
        || layout.tag_size != 4
        || layout.payload_offset != payload_offset
        || layout.cases.len() != 2
        || layout.cases[0].case != DeclarationId::new(prelude::OPTION_NONE_ID)
        || layout.cases[0].tag != 0
        || layout.cases[1].case != DeclarationId::new(prelude::OPTION_SOME_ID)
        || layout.cases[1].tag != 1
    {
        return Err(profile_error(
            "option-propagation Wasm32 layout-v2 binding changed",
        ));
    }
    Ok(())
}

fn validate_profile(program: &ResolvedProgram) -> Result<Vec<DeclarationId>, Diagnostic> {
    if !program.permits.is_empty()
        || !program.interfaces.is_empty()
        || !program.function_templates.is_empty()
        || !program.function_instances.is_empty()
        || program.functions.len() != 2
        || program.types.iter().any(|declaration| {
            !program
                .declarations
                .declaration(&declaration.id)
                .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned)
        })
    {
        return Err(profile_error(
            "option-propagation v10 admits no interfaces, resources, or user nominal types",
        ));
    }
    let selected_id = DeclarationId::new(FUNCTION_ID);
    let selected = function(program, &selected_id)?;
    require_function_signature(
        selected,
        &[option_type(ResolvedType::I64), ResolvedType::I64],
        &option_type(ResolvedType::Bool),
        "selected evaluate function",
    )?;
    let main = function(program, &DeclarationId::new("app.main"))?;
    require_function_signature(main, &[], &ResolvedType::I64, "app.main")?;
    if program.entrypoint != DeclarationId::new("app.main") {
        return Err(profile_error(
            "option-propagation v10 requires exact app.main entrypoint",
        ));
    }
    let expected_program = crate::parse(
        SOURCE_V10,
        std::path::Path::new("option-propagation-v10-hir-profile.spx"),
    )?;
    let expected = hir::resolve(&expected_program).map_err(first_error)?;
    let expected_selected = function(&expected, &selected_id)?;
    let expected_main = function(&expected, &DeclarationId::new("app.main"))?;
    if selected.id != expected_selected.id
        || selected.name != expected_selected.name
        || selected.params != expected_selected.params
        || selected.result_id != expected_selected.result_id
        || selected.return_type != expected_selected.return_type
        || selected.effects != expected_selected.effects
        || selected.requires != expected_selected.requires
        || selected.ensures != expected_selected.ensures
        || selected.body != expected_selected.body
        || selected.cleanup_plan != expected_selected.cleanup_plan
        || main.id != expected_main.id
        || main.params != expected_main.params
        || main.return_type != expected_main.return_type
        || main.body != expected_main.body
        || main.cleanup_plan != expected_main.cleanup_plan
    {
        return Err(profile_error(
            "option-propagation v10 resolved HIR shape changed",
        ));
    }
    if selected.cleanup_plan.schema != crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V3
        || main.cleanup_plan.schema != crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V2
    {
        return Err(profile_error(
            "option-propagation v10 requires CleanupPlan v3 only for evaluate",
        ));
    }
    for function in &program.functions {
        if !function.effects.is_empty() {
            return Err(profile_error(
                "option-propagation v10 reachable functions must be effect-free",
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
    if program.functions[0].id != selected_id
        || program.functions[1].id != DeclarationId::new("app.main")
    {
        return Err(profile_error(
            "option-propagation v10 requires evaluate index 0 and app.main index 1",
        ));
    }
    Ok(vec![selected_id])
}

fn require_exact_source(program: &Program) -> Result<(), Diagnostic> {
    let expected = crate::parse(
        SOURCE_V10,
        std::path::Path::new("option-propagation-v10-profile.spx"),
    )?;
    if crate::format::canonical(program) != crate::format::canonical(&expected) {
        return Err(profile_error(
            "option-propagation v10 requires exact frozen source",
        ));
    }
    Ok(())
}

fn plan_digest(plan_json: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PLAN_DOMAIN);
    hash.update((plan_json.len() as u64).to_le_bytes());
    hash.update(plan_json.as_bytes());
    hash.finalize().into()
}

fn function<'a>(
    program: &'a ResolvedProgram,
    id: &DeclarationId,
) -> Result<&'a ResolvedFunction, Diagnostic> {
    program
        .functions
        .iter()
        .find(|function| function.id == *id)
        .ok_or_else(|| profile_error(format!("option-propagation v10 requires `{id}`")))
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
        | ResolvedExprKind::TryOption { .. }
        | ResolvedExprKind::ConstructVariant { .. } => {}
        _ => {
            return Err(profile_error(
                "option-propagation v10 admits only the exact direct-copy Option/? expressions",
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
        } if declaration == &DeclarationId::new(prelude::OPTION_ID)
            && arguments.len() == 1
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
            "option-propagation v10 excludes type `{}`",
            ty.identity_key()
        ))),
    }
}

fn walk_children(expr: &ResolvedExpr, mut visit: impl FnMut(&ResolvedExpr)) {
    match &expr.kind {
        ResolvedExprKind::Unary { value, .. } => visit(value),
        ResolvedExprKind::TryOption { operand, .. } => visit(operand),
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
    roots: ProfileRoots<'_>,
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
            params: vec![I32, I64, I32],
            results: vec![I32],
        },
        &mut types,
        &mut type_indexes,
    );
    let canonical_type = intern_type(
        Signature {
            params: vec![I32, I64, I64],
            results: vec![I32],
        },
        &mut types,
        &mut type_indexes,
    );
    let validate_index = u32::try_from(bodies.len())
        .map_err(|_| profile_error("option-propagation function index overflows u32"))?;
    let status_out_index = validate_index
        .checked_add(1)
        .ok_or_else(|| profile_error("option-propagation function index overflows u32"))?;
    let canonical_index = status_out_index
        .checked_add(1)
        .ok_or_else(|| profile_error("option-propagation function index overflows u32"))?;
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
            .map_err(|_| profile_error("option-propagation function count overflows u32"))?,
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
    write_name(&mut custom, roots.source_revision);
    custom.extend(roots.graph_digest);
    custom.extend(roots.prelude_digest);
    custom.extend(roots.option_i64_layout_digest);
    custom.extend(roots.option_bool_layout_digest);
    custom.extend(roots.plan_digest);
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
    body.extend([0x41, 0x01, 0x4b, 0x04, 0x40, 0x41]);
    write_i64(&mut body, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
    body.extend([0x0f, 0x0b, 0x20]);
    write_u32(&mut body, tag);
    body.extend([
        0x41, 0x01, 0x46, 0x04, 0x40, 0x20, 0x00, 0x28, 0x02, 0x04, 0x22,
    ]);
    write_u32(&mut body, payload);
    body.extend([0x41, 0x01, 0x4b, 0x04, 0x40, 0x41]);
    write_i64(&mut body, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
    body.extend([0x0f, 0x0b, 0x0b]);
    body.extend([0x20]);
    write_u32(&mut body, tag);
    body.extend([
        0x41, 0x01, 0x46, 0x04, 0x40, 0x20, 0x01, 0x41, 0x04, 0x6a, 0x20,
    ]);
    write_u32(&mut body, payload);
    body.extend([0x36, 0x02, 0x00, 0x0b, 0x20, 0x01, 0x20]);
    write_u32(&mut body, tag);
    body.extend([0x36, 0x02, 0x00, 0x41, 0x00, 0x0b]);
    body
}

fn status_out_body(selected_index: u32, validate_index: u32) -> Vec<u8> {
    let status = 3_u32;
    let mut body = vec![0x01, 0x01, I32];
    body.extend([0x20, 0x00, 0x20, 0x01, 0x41]);
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
    write_u32(&mut body, 2);
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
    // Reject a raw noncanonical input option tag before staging any authority.
    body.extend([0x20, 0x00, 0x41, 0x01, 0x4b, 0x04, 0x40, 0x00, 0x0b]);
    body.extend([0x20, 0x00, 0x41, 0x01, 0x46, 0x04, 0x40, 0x41]);
    write_i64(&mut body, i64::from(INTERNAL_INPUT_AREA + 8));
    body.extend([0x20, 0x01, 0x37, 0x03, 0x00, 0x0b]);
    // Publish the staged internal Option tag last.
    body.push(0x41);
    write_i64(&mut body, i64::from(INTERNAL_INPUT_AREA));
    body.extend([0x20, 0x00, 0x36, 0x02, 0x00]);
    body.push(0x41);
    write_i64(&mut body, i64::from(INTERNAL_INPUT_AREA));
    body.extend([0x20, 0x02, 0x41]);
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
    write_i64(&mut body, i64::from(INTERNAL_RESULT_AREA));
    body.extend([0x28, 0x02, 0x00, 0x41, 0x01, 0x46, 0x04, 0x40, 0x41]);
    write_i64(&mut body, i64::from(RESULT_AREA + 5));
    body.push(0x41);
    write_i64(&mut body, i64::from(INTERNAL_RESULT_AREA + 4));
    body.extend([0x28, 0x02, 0x00, 0x3a, 0x00, 0x00, 0x0b]);
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

fn first_error(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.severity.is_error())
        .unwrap_or_else(|| profile_error("option-propagation graph generation failed"))
}

#[cfg(test)]
#[path = "option_propagation_component_v10/tests.rs"]
mod tests;
