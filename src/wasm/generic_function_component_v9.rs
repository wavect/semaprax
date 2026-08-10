//! Closed exact-instance generic-function core for private Component Model v9 evidence.

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::graph;
use crate::hir::{
    self, DeclarationId, FunctionInstanceId, IdentityOrigin, ResolvedExprKind, ResolvedProgram,
    ResolvedStatement, ResolvedType,
};

use super::{
    aggregate, intern_type, section, write_bytes, write_i64, write_name, write_u32, Signature, I32,
    I64,
};

const TEMPLATE_IDS: [&str; 3] = [
    "component.generic-function.preserve",
    "component.generic-function.invert",
    "component.generic-function.ordered",
];
const MATERIALIZE_ID: &str = "component.generic-function.materialize";
pub(crate) const CANONICAL_EXPORTS: [&str; 6] = [
    "cabi_preserve_i64_v9",
    "cabi_invert_i64_v9",
    "cabi_preserve_bool_v9",
    "cabi_invert_bool_v9",
    "cabi_ordered_i64_bool_v9",
    "cabi_ordered_bool_i64_v9",
];

const CONTRACT_DOMAIN: &str = "semaprax.contract.v1";
const POISON_I32: i32 = 0xa5a5_a5a5_u32 as i32;
const CUSTOM_SECTION: &str = "semaprax.component-generic-function-v9";
const PLAN_DOMAIN: &[u8] = b"semaprax.component-generic-function-plan.v9\0";
const CONTRACT_REQUIRES: i32 = status_word(1, 1);
const CONTRACT_ENSURES: i32 = status_word(1, 2);

const fn status_word(class: i32, code: i32) -> i32 {
    (class << 24) | code
}

pub(crate) const SOURCE_V9: &str = r#"module test.component_generic_function_v9;

@id("component.generic-function.preserve")
fn preserve<T>(marker: bool, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    marker
}

@id("component.generic-function.invert")
fn invert<T>(marker: bool, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    !marker
}

@id("component.generic-function.ordered")
fn ordered<T, U>(marker: bool, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    marker
}

@id("component.generic-function.materialize")
fn materialize() -> bool {
    let preserve_i64 = preserve<i64>(true, 0);
    let invert_i64 = invert<i64>(false, 0);
    let preserve_bool = preserve<bool>(true, 0);
    let invert_bool = invert<bool>(false, 0);
    let ordered_i64_bool = ordered<i64, bool>(true, 0);
    let ordered_bool_i64 = ordered<bool, i64>(true, 0);
    preserve_i64 && invert_i64 && preserve_bool && invert_bool && ordered_i64_bool && ordered_bool_i64
}

@id("app.main")
fn main() -> i64 { if materialize() { 0 } else { 1 } }
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Shape {
    PreserveI64,
    InvertI64,
    PreserveBool,
    InvertBool,
    OrderedI64Bool,
    OrderedBoolI64,
}

impl Shape {
    const ALL: [Self; 6] = [
        Self::PreserveI64,
        Self::InvertI64,
        Self::PreserveBool,
        Self::InvertBool,
        Self::OrderedI64Bool,
        Self::OrderedBoolI64,
    ];

    const fn index(self) -> usize {
        match self {
            Self::PreserveI64 => 0,
            Self::InvertI64 => 1,
            Self::PreserveBool => 2,
            Self::InvertBool => 3,
            Self::OrderedI64Bool => 4,
            Self::OrderedBoolI64 => 5,
        }
    }

    const fn template(self) -> &'static str {
        match self {
            Self::PreserveI64 | Self::PreserveBool => TEMPLATE_IDS[0],
            Self::InvertI64 | Self::InvertBool => TEMPLATE_IDS[1],
            Self::OrderedI64Bool | Self::OrderedBoolI64 => TEMPLATE_IDS[2],
        }
    }

    fn type_arguments(self) -> Vec<ResolvedType> {
        match self {
            Self::PreserveI64 | Self::InvertI64 => vec![ResolvedType::I64],
            Self::PreserveBool | Self::InvertBool => vec![ResolvedType::Bool],
            Self::OrderedI64Bool => vec![ResolvedType::I64, ResolvedType::Bool],
            Self::OrderedBoolI64 => vec![ResolvedType::Bool, ResolvedType::I64],
        }
    }

    fn instance(self) -> FunctionInstanceId {
        FunctionInstanceId::derive(&DeclarationId::new(self.template()), &self.type_arguments())
    }

    const fn invert(self) -> bool {
        matches!(self, Self::InvertI64 | Self::InvertBool)
    }

    const fn internal(self) -> i32 {
        self.result() - 16
    }

    const fn result(self) -> i32 {
        160 + (self.index() as i32 * 64)
    }

    fn canonical_signature(self) -> Signature {
        Signature {
            params: vec![I32, I64],
            results: vec![I32],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateGenericFunctionCoreArtifactV9 {
    pub(crate) bytes: Vec<u8>,
    pub(crate) source_revision: String,
    pub(crate) graph_digest: [u8; 32],
    pub(crate) plan_digest: [u8; 32],
}

pub(crate) fn emit_private_generic_function_core_v9(
    program: &Program,
) -> Result<PrivateGenericFunctionCoreArtifactV9, Diagnostic> {
    require_exact_source(program)?;
    let resolved = hir::resolve(program).map_err(first_error)?;
    hir::validate(&resolved)?;
    let ordered = validate_profile(&resolved)?;
    let lowering = aggregate::lower_selected_function_instances(
        &resolved,
        &ordered,
        ordered
            .first()
            .ok_or_else(|| profile_error("generic-function v9 has no exact instances"))?,
    )?;
    let source_revision = graph::revision(program);
    let graph_json = graph::to_json(program).map_err(first_error)?;
    if !graph_json.starts_with("{\"schema\":\"semaprax.graph.v14\",") {
        return Err(profile_error(
            "generic-function v9 requires exact Graph v14",
        ));
    }
    let graph_digest = Sha256::digest(graph_json.as_bytes()).into();
    let plan_digest = plan_digest();
    let bytes = compose(lowering, &source_revision, graph_digest, plan_digest)?;
    Ok(PrivateGenericFunctionCoreArtifactV9 {
        bytes,
        source_revision,
        graph_digest,
        plan_digest,
    })
}

fn require_exact_source(program: &Program) -> Result<(), Diagnostic> {
    let expected = crate::parse(
        SOURCE_V9,
        std::path::Path::new("generic-function-v9-profile.spx"),
    )?;
    if crate::format::canonical(program) != crate::format::canonical(&expected) {
        return Err(profile_error(
            "generic-function v9 requires exact frozen source",
        ));
    }
    Ok(())
}

fn validate_profile(program: &ResolvedProgram) -> Result<Vec<FunctionInstanceId>, Diagnostic> {
    if !program.permits.is_empty()
        || !program.interfaces.is_empty()
        || program.function_templates.len() != 3
        || program.function_instances.len() != 6
        || program.functions.len() != 2
    {
        return Err(profile_error(
            "generic-function v9 requires three templates, six exact instances, materialize, and app.main without authority",
        ));
    }
    if program.types.iter().any(|declaration| {
        !program
            .declarations
            .declaration(&declaration.id)
            .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned)
    }) {
        return Err(profile_error(
            "generic-function v9 admits no authored type or layout roots",
        ));
    }
    for (index, template) in program.function_templates.iter().enumerate() {
        let expected_parameter_count = if index == 2 { 2 } else { 1 };
        let expected_type_parameters = if index == 2 {
            &[("T", 0_u32), ("U", 1_u32)][..]
        } else {
            &[("T", 0_u32)][..]
        };
        if template.id != DeclarationId::new(TEMPLATE_IDS[index])
            || template.type_parameters.len() != expected_parameter_count
            || !template
                .type_parameters
                .iter()
                .zip(expected_type_parameters)
                .all(|(actual, expected)| actual.name == expected.0 && actual.index == expected.1)
            || template.params.len() != 2
            || template.params[0].ty != ResolvedType::Bool
            || template.params[1].ty != ResolvedType::I64
            || template.return_type != ResolvedType::Bool
            || !template.effects.is_empty()
            || template.requires.len() != 1
            || template.ensures.len() != 1
            || !is_exact_control_contract(&template.requires[0], &template.params[1].id, -99)
            || !is_exact_control_contract(&template.ensures[0], &template.params[1].id, 13)
            || !is_exact_body(&template.body, &template.params[0].id, index == 1)
        {
            return Err(profile_error(
                "generic-function v9 template identity, signature, contract, or body changed",
            ));
        }
    }

    let expected = Shape::ALL.map(Shape::instance);
    for (index, (instance, shape)) in program
        .function_instances
        .iter()
        .zip(Shape::ALL)
        .enumerate()
    {
        if instance.id != expected[index]
            || instance.template != DeclarationId::new(shape.template())
            || instance.type_arguments != shape.type_arguments()
            || instance.function.params.len() != 2
            || instance.function.params[0].ty != ResolvedType::Bool
            || instance.function.params[1].ty != ResolvedType::I64
            || instance.function.return_type != ResolvedType::Bool
            || !instance.function.effects.is_empty()
            || instance.function.requires.len() != 1
            || instance.function.ensures.len() != 1
            || !is_exact_control_contract(
                &instance.function.requires[0],
                &instance.function.params[1].id,
                -99,
            )
            || !is_exact_control_contract(
                &instance.function.ensures[0],
                &instance.function.params[1].id,
                13,
            )
            || !is_exact_body(
                &instance.function.body,
                &instance.function.params[0].id,
                shape.invert(),
            )
        {
            return Err(profile_error(
                "generic-function v9 exact instance identity or specialization changed",
            ));
        }
    }
    if program.functions[0].id != DeclarationId::new(MATERIALIZE_ID)
        || program.functions[1].id != DeclarationId::new("app.main")
        || program.entrypoint != DeclarationId::new("app.main")
    {
        return Err(profile_error(
            "generic-function v9 monomorphic function set changed",
        ));
    }
    validate_materialize(&program.functions[0])?;
    validate_main(&program.functions[1])?;
    Ok(expected.to_vec())
}

fn validate_materialize(function: &crate::hir::ResolvedFunction) -> Result<(), Diagnostic> {
    if !function.params.is_empty()
        || function.return_type != ResolvedType::Bool
        || !function.effects.is_empty()
        || !function.requires.is_empty()
        || !function.ensures.is_empty()
    {
        return Err(profile_error(
            "generic-function v9 materialize signature or contract changed",
        ));
    }
    let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
        return Err(profile_error(
            "generic-function v9 materialize is not an exact block",
        ));
    };
    if statements.len() != Shape::ALL.len() {
        return Err(profile_error(
            "generic-function v9 materialize call count changed",
        ));
    }
    let expected_markers = [true, false, true, false, true, true];
    let mut bindings = Vec::with_capacity(statements.len());
    for (index, ((statement, shape), expected_marker)) in statements
        .iter()
        .zip(Shape::ALL)
        .zip(expected_markers)
        .enumerate()
    {
        let ResolvedStatement::Let { binding, value, .. } = statement;
        let ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance: Some(instance),
            args,
        } = &value.kind
        else {
            return Err(profile_error(
                "generic-function v9 materialize lost an exact instance call",
            ));
        };
        if binding.ty != ResolvedType::Bool
            || value.ty != ResolvedType::Bool
            || callee != &DeclarationId::new(shape.template())
            || type_arguments != &shape.type_arguments()
            || instance != &shape.instance()
            || args.len() != 2
            || !matches!(&args[0].kind, ResolvedExprKind::Bool(value) if *value == expected_marker)
            || !matches!(&args[1].kind, ResolvedExprKind::Int(0))
        {
            return Err(profile_error(format!(
                "generic-function v9 materialize call {index} changed identity or arguments"
            )));
        }
        bindings.push(binding.id.clone());
    }
    let mut cursor = tail.as_ref();
    for binding in bindings.iter().skip(1).rev() {
        let ResolvedExprKind::Binary {
            op: crate::ast::BinaryOp::And,
            left,
            right,
        } = &cursor.kind
        else {
            return Err(profile_error(
                "generic-function v9 materialize all-true chain changed",
            ));
        };
        if !is_exact_place(right, binding) {
            return Err(profile_error(
                "generic-function v9 materialize all-true order changed",
            ));
        }
        cursor = left;
    }
    if !is_exact_place(cursor, &bindings[0]) {
        return Err(profile_error(
            "generic-function v9 materialize all-true root changed",
        ));
    }
    Ok(())
}

fn validate_main(function: &crate::hir::ResolvedFunction) -> Result<(), Diagnostic> {
    if !function.params.is_empty()
        || function.return_type != ResolvedType::I64
        || !function.effects.is_empty()
        || !function.requires.is_empty()
        || !function.ensures.is_empty()
    {
        return Err(profile_error(
            "generic-function v9 app.main signature or contract changed",
        ));
    }
    let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
        return Err(profile_error("generic-function v9 app.main is not a block"));
    };
    let ResolvedExprKind::If {
        condition,
        then_branch,
        else_branch,
    } = &tail.kind
    else {
        return Err(profile_error(
            "generic-function v9 app.main lost its exact result check",
        ));
    };
    if !statements.is_empty()
        || !matches!(
            &condition.kind,
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance: None,
                args,
            } if callee == &DeclarationId::new(MATERIALIZE_ID)
                && type_arguments.is_empty()
                && args.is_empty()
        )
        || !is_exact_block_int(then_branch, 0)
        || !is_exact_block_int(else_branch, 1)
    {
        return Err(profile_error(
            "generic-function v9 app.main call or 0/1 result changed",
        ));
    }
    Ok(())
}

fn is_exact_place(expression: &crate::hir::ResolvedExpr, value: &crate::hir::ValueId) -> bool {
    expression.ty == ResolvedType::Bool
        && matches!(
            &expression.kind,
            ResolvedExprKind::Place(place)
                if &place.root == value && place.projections.is_empty()
        )
}

fn is_exact_block_int(expression: &crate::hir::ResolvedExpr, expected: i64) -> bool {
    matches!(
        &expression.kind,
        ResolvedExprKind::Block { statements, tail }
            if statements.is_empty()
                && matches!(&tail.kind, ResolvedExprKind::Int(value) if *value == expected)
    )
}

fn is_exact_body(
    body: &crate::hir::ResolvedExpr,
    marker: &crate::hir::ValueId,
    invert: bool,
) -> bool {
    let ResolvedExprKind::Block { statements, tail } = &body.kind else {
        return false;
    };
    if !statements.is_empty() {
        return false;
    }
    let exact_marker = |candidate: &crate::hir::ResolvedExpr| {
        candidate.ty == ResolvedType::Bool
            && matches!(
                &candidate.kind,
                ResolvedExprKind::Place(place)
                    if &place.root == marker && place.projections.is_empty()
            )
    };
    match (&tail.kind, invert) {
        (ResolvedExprKind::Place(_), false) => exact_marker(tail),
        (
            ResolvedExprKind::Unary {
                op: crate::ast::UnaryOp::Not,
                value,
            },
            true,
        ) => exact_marker(value),
        _ => false,
    }
}

fn is_exact_control_contract(
    expression: &crate::hir::ResolvedExpr,
    control: &crate::hir::ValueId,
    expected: i64,
) -> bool {
    matches!(
        &expression.kind,
        ResolvedExprKind::Binary {
            op: crate::ast::BinaryOp::Ne,
            left,
            right,
        } if matches!(
            &left.kind,
            ResolvedExprKind::Place(place)
                if &place.root == control && place.projections.is_empty()
        ) && is_exact_int(right, expected)
    )
}

fn is_exact_int(expression: &crate::hir::ResolvedExpr, expected: i64) -> bool {
    match (&expression.kind, expected) {
        (ResolvedExprKind::Int(value), expected) => *value == expected,
        (
            ResolvedExprKind::Unary {
                op: crate::ast::UnaryOp::Neg,
                value,
            },
            expected,
        ) if expected < 0 => {
            matches!(&value.kind, ResolvedExprKind::Int(value) if *value == -expected)
        }
        _ => false,
    }
}

fn plan_digest() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PLAN_DOMAIN);
    for shape in Shape::ALL {
        for field in [
            shape.template().as_bytes(),
            shape.instance().as_str().as_bytes(),
            CANONICAL_EXPORTS[shape.index()].as_bytes(),
        ] {
            hash.update((field.len() as u64).to_le_bytes());
            hash.update(field);
        }
        for argument in shape.type_arguments() {
            let key = argument.identity_key();
            hash.update((key.len() as u64).to_le_bytes());
            hash.update(key.as_bytes());
        }
        hash.update(shape.internal().to_le_bytes());
        hash.update(shape.result().to_le_bytes());
        hash.update([u8::from(shape.invert())]);
        hash.update([shape.index() as u8]);
    }
    hash.finalize().into()
}

fn compose(
    lowering: aggregate::SelectedAggregateLowering,
    source_revision: &str,
    graph_digest: [u8; 32],
    plan_digest: [u8; 32],
) -> Result<Vec<u8>, Diagnostic> {
    let aggregate::SelectedAggregateLowering {
        mut types,
        function_type_indexes,
        bodies: mut source_bodies,
        selected_index,
    } = lowering;
    if selected_index != 0 || source_bodies.len() != 6 || function_type_indexes.len() != 6 {
        return Err(profile_error(
            "generic-function v9 selected exact-instance map changed",
        ));
    }
    let mut type_indexes = types
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, signature)| (signature, index as u32))
        .collect::<HashMap<_, _>>();
    let canonical_type = intern_type(
        Shape::PreserveI64.canonical_signature(),
        &mut types,
        &mut type_indexes,
    );
    let canonical_start = source_bodies.len() as u32;
    for shape in Shape::ALL {
        source_bodies.push(canonical_adapter_body(shape, shape.index() as u32));
    }

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
    write_u32(&mut functions, 12);
    for index in function_type_indexes {
        write_u32(&mut functions, index);
    }
    for _ in Shape::ALL {
        write_u32(&mut functions, canonical_type);
    }
    section(&mut module, 3, functions);
    section(&mut module, 5, vec![0x01, 0x00, 0x01]);
    let mut globals = vec![0x01, I32, 0x01, 0x41];
    write_i64(&mut globals, i64::from(aggregate::SHADOW_STACK_TOP));
    globals.push(0x0b);
    section(&mut module, 6, globals);
    let mut exports = Vec::new();
    write_u32(&mut exports, 7);
    write_name(&mut exports, "memory");
    exports.extend([0x02, 0x00]);
    for (offset, name) in CANONICAL_EXPORTS.into_iter().enumerate() {
        write_name(&mut exports, name);
        exports.push(0x00);
        write_u32(&mut exports, canonical_start + offset as u32);
    }
    section(&mut module, 7, exports);
    let mut code = Vec::new();
    write_u32(&mut code, source_bodies.len() as u32);
    for body in source_bodies {
        write_bytes(&mut code, &body);
    }
    section(&mut module, 10, code);
    let mut data = Vec::new();
    write_u32(&mut data, 1);
    active_data(&mut data, 0, CONTRACT_DOMAIN.as_bytes());
    section(&mut module, 11, data);
    let mut custom = Vec::new();
    write_name(&mut custom, CUSTOM_SECTION);
    write_name(&mut custom, source_revision);
    custom.extend(graph_digest);
    custom.extend(plan_digest);
    section(&mut module, 0, custom);
    Ok(module)
}

fn active_data(output: &mut Vec<u8>, offset: i32, bytes: &[u8]) {
    output.push(0x00);
    output.push(0x41);
    write_i64(output, i64::from(offset));
    output.push(0x0b);
    write_bytes(output, bytes);
}

fn canonical_adapter_body(shape: Shape, selected_index: u32) -> Vec<u8> {
    let status = 2_u32;
    let flag = 3_u32;
    let mut body = vec![0x01, 0x02, I32];
    for offset in [0, 4, 8, 12, 16] {
        store_i32_const(&mut body, shape.result() + offset, POISON_I32);
    }
    body.extend([0x20, 0x00, 0x41, 0x01, 0x4b, 0x04, 0x40, 0x00, 0x0b]);
    body.extend([0x20, 0x00, 0x20, 0x01, 0x41]);
    write_i64(&mut body, i64::from(shape.internal()));
    body.push(0x10);
    write_u32(&mut body, selected_index);
    body.push(0x21);
    write_u32(&mut body, status);
    trap_invalid_status(&mut body, status);
    body.push(0x20);
    write_u32(&mut body, status);
    body.extend([0x45, 0x04, 0x40, 0x41]);
    write_i64(&mut body, i64::from(shape.internal()));
    body.extend([0x28, 0x02, 0x00, 0x22]);
    write_u32(&mut body, flag);
    body.extend([0x41, 0x01, 0x4b, 0x04, 0x40, 0x00, 0x0b]);
    store_i32_byte_local(&mut body, shape.result() + 4, flag);
    store_tag(&mut body, shape.result(), 0);
    body.push(0x05);
    emit_normalized_status(&mut body, status);
    body.push(0x21);
    write_u32(&mut body, status);
    trap_invalid_status(&mut body, status);
    emit_status_fields(&mut body, status, shape.result() + 4);
    store_tag(&mut body, shape.result(), 1);
    body.push(0x0b);
    body.push(0x41);
    write_i64(&mut body, i64::from(shape.result()));
    body.push(0x0b);
    body
}

fn trap_invalid_status(output: &mut Vec<u8>, status: u32) {
    output.push(0x20);
    write_u32(output, status);
    output.push(0x41);
    write_i64(output, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
    output.extend([0x46, 0x04, 0x40, 0x00, 0x0b]);
}

fn store_i32_const(output: &mut Vec<u8>, address: i32, value: i32) {
    output.push(0x41);
    write_i64(output, i64::from(address));
    output.push(0x41);
    write_i64(output, i64::from(value));
    output.extend([0x36, 0x02, 0x00]);
}

fn store_i32_byte_local(output: &mut Vec<u8>, address: i32, local: u32) {
    output.push(0x41);
    write_i64(output, i64::from(address));
    output.push(0x20);
    write_u32(output, local);
    output.extend([0x3a, 0x00, 0x00]);
}

fn store_tag(output: &mut Vec<u8>, address: i32, tag: i32) {
    output.push(0x41);
    write_i64(output, i64::from(address));
    output.push(0x41);
    write_i64(output, i64::from(tag));
    output.extend([0x3a, 0x00, 0x00]);
}

fn emit_normalized_status(output: &mut Vec<u8>, status: u32) {
    output.push(0x20);
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
    output.extend([0x0b, 0x0b]);
}

fn emit_status_fields(output: &mut Vec<u8>, status: u32, base: i32) {
    store_i32_const(output, base, 0);
    store_i32_const(output, base + 4, CONTRACT_DOMAIN.len() as i32);
    output.push(0x41);
    write_i64(output, i64::from(base + 8));
    output.push(0x20);
    write_u32(output, status);
    output.extend([0x41]);
    write_i64(output, 0x00ff_ffff);
    output.extend([0x71, 0x36, 0x02, 0x00]);
    output.push(0x41);
    write_i64(output, i64::from(base + 12));
    output.push(0x20);
    write_u32(output, status);
    output.extend([0x41, 0x18, 0x76, 0x3a, 0x00, 0x00]);
    store_i32_byte_const(output, base + 13, 1);
    store_i32_byte_const(output, base + 14, 0);
}

fn store_i32_byte_const(output: &mut Vec<u8>, address: i32, value: i32) {
    output.push(0x41);
    write_i64(output, i64::from(address));
    output.push(0x41);
    write_i64(output, i64::from(value));
    output.extend([0x3a, 0x00, 0x00]);
}

fn first_error(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.severity.is_error())
        .unwrap_or_else(|| profile_error("generic-function v9 HIR resolution failed"))
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W113", message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::*;

    fn artifact() -> PrivateGenericFunctionCoreArtifactV9 {
        let program =
            crate::parse(SOURCE_V9, Path::new("component-generic-function-v9.spx")).unwrap();
        emit_private_generic_function_core_v9(&program).unwrap()
    }

    #[derive(Clone, Copy)]
    enum AdversarialOutcome {
        InvalidBool,
        InvalidTag,
        UnknownStatus,
    }

    fn adversarial_core(outcome: AdversarialOutcome) -> Vec<u8> {
        let program =
            crate::parse(SOURCE_V9, Path::new("generic-function-v9-adversarial.spx")).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        hir::validate(&resolved).unwrap();
        let ordered = validate_profile(&resolved).unwrap();
        let mut lowering =
            aggregate::lower_selected_function_instances(&resolved, &ordered, &ordered[0]).unwrap();
        for shape in Shape::ALL {
            let mut body = vec![0x00];
            match outcome {
                AdversarialOutcome::InvalidBool => {
                    body.extend([0x20, 0x02, 0x41, 0x02, 0x36, 0x02, 0x00]);
                    body.extend([0x41, 0x00, 0x0b]);
                }
                AdversarialOutcome::InvalidTag => {
                    body.push(0x41);
                    write_i64(&mut body, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
                    body.push(0x0b);
                }
                AdversarialOutcome::UnknownStatus => body.extend([0x41, 0x63, 0x0b]),
            }
            lowering.bodies[shape.index()] = body;
        }
        let graph_json = graph::to_json(&program).unwrap();
        compose(
            lowering,
            &graph::revision(&program),
            Sha256::digest(graph_json.as_bytes()).into(),
            plan_digest(),
        )
        .unwrap()
    }

    #[test]
    fn deterministic_core_is_upstream_valid_and_graph_v14_bound() {
        let first = artifact();
        assert_eq!(first, artifact());
        assert_eq!(
            first.source_revision,
            "sha256:218085fb5ea1bcc090c04ac0acb3395912d0dad09027b9118d8817978b2fde0c"
        );
        assert_eq!(
            first.graph_digest,
            [
                0x62, 0x90, 0x7c, 0x4b, 0x95, 0x49, 0x5b, 0xb5, 0x73, 0xb2, 0xb3, 0x7d, 0xe9, 0xf0,
                0xb0, 0x8c, 0x7a, 0x82, 0x21, 0x89, 0x34, 0x15, 0x45, 0x21, 0xe8, 0xc0, 0xc8, 0x39,
                0x61, 0x58, 0xcc, 0x6e,
            ]
        );
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&first.bytes)
            .expect("upstream validator rejected generic-function v9 core");
    }

    #[test]
    fn source_and_exact_instance_mutations_reject() {
        for hostile in [
            SOURCE_V9.replacen("preserve<i64>", "preserve<bool>", 1),
            SOURCE_V9.replacen("ordered<i64, bool>", "ordered<bool, i64>", 1),
            SOURCE_V9.replacen("control != -99", "control != -98", 1),
            SOURCE_V9.replacen("control != 13", "control != 12", 1),
            SOURCE_V9.replacen("fn invert<T>", "fn invert<U>", 1),
            SOURCE_V9.replacen("!marker", "marker", 1),
            SOURCE_V9.replacen("fn ordered<T, U>", "fn ordered<U, T>", 1),
        ] {
            let parsed = crate::parse(&hostile, Path::new("hostile-generic-function-v9.spx"));
            match parsed {
                Ok(program) => assert!(emit_private_generic_function_core_v9(&program).is_err()),
                Err(error) => assert!(!error.code.is_empty()),
            }
        }
    }

    #[test]
    fn hostile_resolved_profiles_reject_identity_and_call_confusion() {
        let program =
            crate::parse(SOURCE_V9, Path::new("generic-function-v9-hir-hostile.spx")).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        hir::validate(&resolved).unwrap();

        let mut hostile = resolved.clone();
        hostile.function_templates[2].type_parameters.swap(0, 1);
        assert!(validate_profile(&hostile).is_err());

        let mut hostile = resolved.clone();
        hostile.function_instances.swap(0, 1);
        assert!(validate_profile(&hostile).is_err());

        let mut hostile = resolved.clone();
        let ResolvedExprKind::Block { statements, .. } = &mut hostile.functions[0].body.kind else {
            panic!("materialize shape drifted");
        };
        let ResolvedStatement::Let { value, .. } = &mut statements[0];
        let ResolvedExprKind::Call { instance, .. } = &mut value.kind else {
            panic!("materialize call shape drifted");
        };
        *instance = Some(Shape::PreserveBool.instance());
        assert!(validate_profile(&hostile).is_err());

        let mut hostile = resolved;
        hostile.entrypoint = DeclarationId::new(MATERIALIZE_ID);
        assert!(validate_profile(&hostile).is_err());
    }

    #[test]
    fn node_executes_all_instances_contracts_and_invalid_input_bools() {
        let artifact = artifact();
        let stem = format!("semaprax-generic-function-v9-{}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, artifact.bytes).unwrap();
        std::fs::write(
            &script_path,
            "import fs from 'node:fs';\nconst {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]));const m=new DataView(instance.exports.memory.buffer);const u=new Uint8Array(instance.exports.memory.buffer);const names=['cabi_preserve_i64_v9','cabi_invert_i64_v9','cabi_preserve_bool_v9','cabi_invert_bool_v9','cabi_ordered_i64_bool_v9','cabi_ordered_bool_i64_v9'];const f=names.map(n=>instance.exports[n]);const results=[160,224,288,352,416,480];for(let i=0;i<6;i++){for(const b of [0,1]){let p=f[i](b,0n);const expected=(i===1||i===3)?1-b:b;if(m.getUint8(p)!==0||m.getUint8(p+4)!==expected)throw Error(`semantic-${i}-${b}`)}}for(let i=0;i<6;i++){let p=f[i](1,-99n);if(m.getUint8(p)!==1||m.getUint32(p+12,true)!==1)throw Error('requires');p=f[i](1,13n);if(m.getUint8(p)!==1||m.getUint32(p+12,true)!==2)throw Error('ensures');u.fill(0x3c,results[i],results[i]+20);let trapped=false;try{f[i](2,0n)}catch{trapped=true}if(!trapped)throw Error('bool2');for(let j=0;j<20;j++)if(u[results[i]+j]!==0xa5)throw Error(`bool2-poison-${i}-${j}`)}console.log('generic-function-v9-core-ok');\n",
        )
        .unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .output()
            .expect("Node is required by the existing Wasm gate");
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        assert!(
            output.status.success(),
            "Node generic-function v9 gate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn adversarial_status_and_output_bools_trap_before_any_publication() {
        let stem = format!(
            "semaprax-generic-function-v9-hostile-{}",
            std::process::id()
        );
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(
            &script_path,
            "import fs from 'node:fs';\nconst names=['cabi_preserve_i64_v9','cabi_invert_i64_v9','cabi_preserve_bool_v9','cabi_invert_bool_v9','cabi_ordered_i64_bool_v9','cabi_ordered_bool_i64_v9'];const results=[160,224,288,352,416,480];for(const path of process.argv.slice(2)){const {instance}=await WebAssembly.instantiate(fs.readFileSync(path));const u=new Uint8Array(instance.exports.memory.buffer);for(let i=0;i<6;i++){u.fill(0x3c,results[i],results[i]+20);let trapped=false;try{instance.exports[names[i]](1,0n)}catch{trapped=true}if(!trapped)throw Error(`hostile-no-trap-${path}-${i}`);for(let j=0;j<20;j++)if(u[results[i]+j]!==0xa5)throw Error(`hostile-published-${path}-${i}-${j}`)}}console.log('generic-function-v9-hostiles-ok');\n",
        )
        .unwrap();
        let mut paths = Vec::new();
        for (name, outcome) in [
            ("bool2", AdversarialOutcome::InvalidBool),
            ("tag", AdversarialOutcome::InvalidTag),
            ("unknown", AdversarialOutcome::UnknownStatus),
        ] {
            let path = std::env::temp_dir().join(format!("{stem}-{name}.wasm"));
            let bytes = adversarial_core(outcome);
            assert_ne!(bytes, artifact().bytes);
            wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
                .validate_all(&bytes)
                .expect("upstream validator rejected adversarial v9 core");
            std::fs::write(&path, bytes).unwrap();
            paths.push(path);
        }
        let output = Command::new("node")
            .arg(&script_path)
            .args(&paths)
            .output()
            .expect("Node is required by the existing Wasm gate");
        let _ = std::fs::remove_file(&script_path);
        for path in paths {
            let _ = std::fs::remove_file(path);
        }
        assert!(
            output.status.success(),
            "Node generic-function v9 hostile gate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
