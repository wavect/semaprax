//! Closed concrete generic-record core for private Component Model v7 evidence.

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::aggregate_layout::{AggregateLayout, AggregateTarget};
use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::graph;
use crate::hir::{
    self, DeclarationId, IdentityOrigin, ResolvedProgram, ResolvedType, ResolvedTypeDeclarationKind,
};

use super::{
    aggregate, intern_type, section, write_bytes, write_i64, write_name, write_u32, Signature, I32,
    I64,
};

const DUO_ID: &str = "component.duo";
const PHANTOM_ID: &str = "component.phantom";
const FUNCTION_IDS: [&str; 4] = [
    "component.transform-i64-bool",
    "component.transform-bool-i64",
    "component.preserve-phantom-i64",
    "component.invert-phantom-bool",
];
pub(crate) const CANONICAL_EXPORTS: [&str; 4] = [
    "cabi_transform_i64_bool_v7",
    "cabi_transform_bool_i64_v7",
    "cabi_preserve_phantom_i64_v7",
    "cabi_invert_phantom_bool_v7",
];

const CONTRACT_DOMAIN: &str = "semaprax.contract.v1";
const ARITHMETIC_DOMAIN: &str = "semaprax.arithmetic.v1";
const POISON_I64: i64 = 0xa5a5_a5a5_a5a5_a5a5_u64 as i64;
const CUSTOM_SECTION: &str = "semaprax.component-generic-record-v7";
const PLAN_DOMAIN: &[u8] = b"semaprax.component-generic-record-plan.v7\0";

const CONTRACT_REQUIRES: i32 = status_word(1, 1);
const CONTRACT_ENSURES: i32 = status_word(1, 2);
const ARITHMETIC_BASE: i32 = status_word(2, 0);

const fn status_word(class: i32, code: i32) -> i32 {
    (class << 24) | code
}

pub(crate) const SOURCE_V7: &str = r#"module test.component_generic_record_v7;

@id("component.duo")
record Duo<T, U> {
    @id("component.duo.left") left: T,
    @id("component.duo.right") right: U,
}

@id("component.phantom")
record Phantom<T> {
    @id("component.phantom.marker") marker: bool,
}

@id("component.transform-i64-bool")
fn transform_i64_bool(input: Duo<i64, bool>, delta: i64, divisor: i64) -> Duo<i64, bool>
    requires delta != -99
    ensures divisor != 13
{
    input with { left: (input.left + delta) / divisor }
}

@id("component.transform-bool-i64")
fn transform_bool_i64(input: Duo<bool, i64>, delta: i64, divisor: i64) -> Duo<bool, i64>
    requires delta != -99
    ensures divisor != 13
{
    input with { right: (input.right + delta) / divisor }
}

@id("component.preserve-phantom-i64")
fn preserve_phantom_i64(input: Phantom<i64>) -> Phantom<i64> {
    input with { marker: input.marker }
}

@id("component.invert-phantom-bool")
fn invert_phantom_bool(input: Phantom<bool>) -> Phantom<bool> {
    input with { marker: !input.marker }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Shape {
    DuoI64Bool,
    DuoBoolI64,
    PhantomI64,
    PhantomBool,
}

impl Shape {
    const ALL: [Self; 4] = [
        Self::DuoI64Bool,
        Self::DuoBoolI64,
        Self::PhantomI64,
        Self::PhantomBool,
    ];

    const fn index(self) -> usize {
        match self {
            Self::DuoI64Bool => 0,
            Self::DuoBoolI64 => 1,
            Self::PhantomI64 => 2,
            Self::PhantomBool => 3,
        }
    }

    fn ty(self) -> ResolvedType {
        let (declaration, arguments) = match self {
            Self::DuoI64Bool => (DUO_ID, vec![ResolvedType::I64, ResolvedType::Bool]),
            Self::DuoBoolI64 => (DUO_ID, vec![ResolvedType::Bool, ResolvedType::I64]),
            Self::PhantomI64 => (PHANTOM_ID, vec![ResolvedType::I64]),
            Self::PhantomBool => (PHANTOM_ID, vec![ResolvedType::Bool]),
        };
        ResolvedType::Nominal {
            declaration: DeclarationId::new(declaration),
            arguments,
        }
    }

    const fn input(self) -> i32 {
        [128, 256, 384, 448][self.index()]
    }

    const fn internal(self) -> i32 {
        [160, 288, 400, 464][self.index()]
    }

    const fn result(self) -> i32 {
        [192, 320, 416, 480][self.index()]
    }

    const fn bool_parameter(self) -> u32 {
        match self {
            Self::DuoI64Bool => 1,
            Self::DuoBoolI64 | Self::PhantomI64 | Self::PhantomBool => 0,
        }
    }

    const fn bool_offset(self) -> i32 {
        match self {
            Self::DuoI64Bool => 8,
            Self::DuoBoolI64 | Self::PhantomI64 | Self::PhantomBool => 0,
        }
    }

    const fn payload_offset(self) -> i32 {
        match self {
            Self::DuoI64Bool | Self::DuoBoolI64 => 8,
            Self::PhantomI64 | Self::PhantomBool => 4,
        }
    }

    fn canonical_signature(self) -> Signature {
        let params = match self {
            Self::DuoI64Bool => vec![I64, I32, I64, I64],
            Self::DuoBoolI64 => vec![I32, I64, I64, I64],
            Self::PhantomI64 | Self::PhantomBool => vec![I32],
        };
        Signature {
            params,
            results: vec![I32],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateGenericRecordCoreArtifactV7 {
    pub(crate) bytes: Vec<u8>,
    pub(crate) source_revision: String,
    pub(crate) graph_digest: [u8; 32],
    pub(crate) plan_digest: [u8; 32],
    pub(crate) layout_digests: [[u8; 32]; 4],
}

pub(crate) fn emit_private_generic_record_core_v7(
    program: &Program,
) -> Result<PrivateGenericRecordCoreArtifactV7, Diagnostic> {
    require_exact_source(program)?;
    let resolved = hir::resolve(program).map_err(first_error)?;
    hir::validate(&resolved)?;
    let ordered = validate_profile(&resolved)?;
    let mut layout_digests = [[0_u8; 32]; 4];
    for shape in Shape::ALL {
        let layout = AggregateLayout::for_type(&resolved, AggregateTarget::Wasm32, &shape.ty())?;
        require_layout(&layout, shape)?;
        layout_digests[shape.index()] = layout.digest();
    }
    if layout_digests[2] == layout_digests[3] {
        return Err(profile_error(
            "Phantom concrete instances lost exact-instance layout identity",
        ));
    }
    let mut lowerings = Vec::new();
    for id in &ordered {
        lowerings.push(aggregate::lower_selected_functions(
            &resolved, &ordered, id,
        )?);
    }
    let primary = lowerings.remove(0);
    let mut selected_indexes = vec![primary.selected_index];
    for lowering in lowerings {
        if lowering.types != primary.types
            || lowering.function_type_indexes != primary.function_type_indexes
            || lowering.bodies != primary.bodies
        {
            return Err(profile_error(
                "generic-record selected lowerings disagree on their shared closure",
            ));
        }
        selected_indexes.push(lowering.selected_index);
    }
    let source_revision = graph::revision(program);
    let graph_json = graph::to_json(program).map_err(first_error)?;
    if !graph_json.starts_with("{\"schema\":\"semaprax.graph.v12\",") {
        return Err(profile_error("generic-record v7 requires exact Graph v12"));
    }
    let graph_digest = Sha256::digest(graph_json.as_bytes()).into();
    let plan_digest = plan_digest(&layout_digests);
    let bytes = compose(
        primary,
        &selected_indexes,
        &source_revision,
        graph_digest,
        plan_digest,
        layout_digests,
    )?;
    Ok(PrivateGenericRecordCoreArtifactV7 {
        bytes,
        source_revision,
        graph_digest,
        plan_digest,
        layout_digests,
    })
}

fn require_exact_source(program: &Program) -> Result<(), Diagnostic> {
    let expected = crate::parse(
        SOURCE_V7,
        std::path::Path::new("generic-record-v7-profile.spx"),
    )?;
    if crate::format::canonical(program) != crate::format::canonical(&expected) {
        return Err(profile_error(
            "generic-record v7 requires the exact frozen source semantics",
        ));
    }
    Ok(())
}

fn validate_profile(program: &ResolvedProgram) -> Result<Vec<DeclarationId>, Diagnostic> {
    if !program.permits.is_empty() || !program.interfaces.is_empty() || program.functions.len() != 5
    {
        return Err(profile_error(
            "generic-record v7 requires four exports and app.main without authority",
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
        || authored[0].id != DeclarationId::new(DUO_ID)
        || authored[1].id != DeclarationId::new(PHANTOM_ID)
        || authored[0].type_parameters.len() != 2
        || authored[1].type_parameters.len() != 1
        || !matches!(authored[0].kind, ResolvedTypeDeclarationKind::Record { .. })
        || !matches!(authored[1].kind, ResolvedTypeDeclarationKind::Record { .. })
    {
        return Err(profile_error(
            "generic-record v7 template identities or parameter order changed",
        ));
    }
    let expected_ids = FUNCTION_IDS.map(DeclarationId::new);
    if program
        .functions
        .iter()
        .take(4)
        .map(|function| &function.id)
        .ne(expected_ids.iter())
    {
        return Err(profile_error(
            "generic-record v7 function identities or order changed",
        ));
    }
    for (index, shape) in Shape::ALL.into_iter().enumerate() {
        let function = &program.functions[index];
        let expected_params = if index < 2 { 3 } else { 1 };
        if function.params.len() != expected_params
            || function.params[0].ty != shape.ty()
            || function.return_type != shape.ty()
            || !function.effects.is_empty()
            || function.requires.len() != usize::from(index < 2)
            || function.ensures.len() != usize::from(index < 2)
            || (index < 2
                && (function.params[1].ty != ResolvedType::I64
                    || function.params[2].ty != ResolvedType::I64))
        {
            return Err(profile_error(
                "generic-record v7 export signature or contract shape changed",
            ));
        }
    }
    let main = &program.functions[4];
    if main.id != DeclarationId::new("app.main")
        || !main.params.is_empty()
        || main.return_type != ResolvedType::I64
        || !main.effects.is_empty()
        || !main.requires.is_empty()
        || !main.ensures.is_empty()
    {
        return Err(profile_error("generic-record v7 app.main shape changed"));
    }
    Ok(expected_ids.to_vec())
}

fn require_layout(layout: &AggregateLayout, shape: Shape) -> Result<(), Diagnostic> {
    let expected_record = match shape {
        Shape::DuoI64Bool | Shape::DuoBoolI64 => DeclarationId::new(DUO_ID),
        Shape::PhantomI64 | Shape::PhantomBool => DeclarationId::new(PHANTOM_ID),
    };
    if layout.instance != shape.ty() || layout.record != expected_record {
        return Err(profile_error("generic-record v7 layout instance changed"));
    }
    let valid = match shape {
        Shape::DuoI64Bool => {
            layout.size == 16
                && layout.align == 8
                && layout.fields.len() == 2
                && layout.fields[0].field == DeclarationId::new("component.duo.left")
                && layout.fields[0].ty == ResolvedType::I64
                && layout.fields[0].offset == 0
                && layout.fields[1].field == DeclarationId::new("component.duo.right")
                && layout.fields[1].ty == ResolvedType::Bool
                && layout.fields[1].offset == 8
        }
        Shape::DuoBoolI64 => {
            layout.size == 16
                && layout.align == 8
                && layout.fields.len() == 2
                && layout.fields[0].field == DeclarationId::new("component.duo.left")
                && layout.fields[0].ty == ResolvedType::Bool
                && layout.fields[0].offset == 0
                && layout.fields[1].field == DeclarationId::new("component.duo.right")
                && layout.fields[1].ty == ResolvedType::I64
                && layout.fields[1].offset == 8
        }
        Shape::PhantomI64 | Shape::PhantomBool => {
            layout.size == 4
                && layout.align == 4
                && layout.fields.len() == 1
                && layout.fields[0].field == DeclarationId::new("component.phantom.marker")
                && layout.fields[0].ty == ResolvedType::Bool
                && layout.fields[0].offset == 0
        }
    };
    if !valid {
        return Err(profile_error(
            "generic-record v7 Wasm32 field layout changed",
        ));
    }
    Ok(())
}

fn plan_digest(layout_digests: &[[u8; 32]; 4]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PLAN_DOMAIN);
    for (shape, layout) in Shape::ALL.into_iter().zip(layout_digests) {
        for field in [
            FUNCTION_IDS[shape.index()].as_bytes(),
            CANONICAL_EXPORTS[shape.index()].as_bytes(),
            shape.ty().identity_key().as_bytes(),
        ] {
            hash.update((field.len() as u64).to_le_bytes());
            hash.update(field);
        }
        hash.update(shape.input().to_le_bytes());
        hash.update(shape.internal().to_le_bytes());
        hash.update(shape.result().to_le_bytes());
        hash.update(shape.bool_parameter().to_le_bytes());
        hash.update(shape.bool_offset().to_le_bytes());
        hash.update(shape.payload_offset().to_le_bytes());
        hash.update(layout);
    }
    hash.finalize().into()
}

fn compose(
    lowering: aggregate::SelectedAggregateLowering,
    selected_indexes: &[u32],
    source_revision: &str,
    graph_digest: [u8; 32],
    plan_digest: [u8; 32],
    layout_digests: [[u8; 32]; 4],
) -> Result<Vec<u8>, Diagnostic> {
    let aggregate::SelectedAggregateLowering {
        mut types,
        function_type_indexes,
        bodies: mut source_bodies,
        ..
    } = lowering;
    if selected_indexes != [0, 1, 2, 3]
        || source_bodies.len() != 4
        || function_type_indexes.len() != 4
    {
        return Err(profile_error(
            "generic-record v7 selected function map changed",
        ));
    }
    let mut type_indexes = types
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, signature)| (signature, index as u32))
        .collect::<HashMap<_, _>>();
    let mut canonical_types = Vec::new();
    for shape in Shape::ALL {
        canonical_types.push(intern_type(
            shape.canonical_signature(),
            &mut types,
            &mut type_indexes,
        ));
    }
    let canonical_start = source_bodies.len() as u32;
    for (shape, selected) in Shape::ALL.into_iter().zip(selected_indexes.iter().copied()) {
        source_bodies.push(canonical_adapter_body(shape, selected));
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
    write_u32(&mut functions, 8);
    for index in function_type_indexes {
        write_u32(&mut functions, index);
    }
    for index in canonical_types {
        write_u32(&mut functions, index);
    }
    section(&mut module, 3, functions);
    section(&mut module, 5, vec![0x01, 0x00, 0x01]);
    let mut globals = vec![0x01, I32, 0x01, 0x41];
    write_i64(&mut globals, i64::from(aggregate::SHADOW_STACK_TOP));
    globals.push(0x0b);
    section(&mut module, 6, globals);
    let mut exports = Vec::new();
    write_u32(&mut exports, 5);
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
    write_u32(&mut data, 2);
    active_data(&mut data, 0, CONTRACT_DOMAIN.as_bytes());
    active_data(&mut data, 32, ARITHMETIC_DOMAIN.as_bytes());
    section(&mut module, 11, data);
    let mut custom = Vec::new();
    write_name(&mut custom, CUSTOM_SECTION);
    write_name(&mut custom, source_revision);
    custom.extend(graph_digest);
    custom.extend(plan_digest);
    for digest in layout_digests {
        custom.extend(digest);
    }
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

fn canonical_adapter_body(shape: Shape, selected_index: u32) -> Vec<u8> {
    let param_count = shape.canonical_signature().params.len() as u32;
    let status = param_count;
    let flag = status + 1;
    let mut body = vec![0x01, 0x02, I32];
    for offset in [shape.result(), shape.result() + 8, shape.result() + 16] {
        body.push(0x41);
        write_i64(&mut body, i64::from(offset));
        body.push(0x42);
        write_i64(&mut body, POISON_I64);
        body.extend([0x37, 0x03, 0x00]);
    }
    body.push(0x20);
    write_u32(&mut body, shape.bool_parameter());
    body.extend([0x41, 0x01, 0x4b, 0x04, 0x40, 0x00, 0x0b]);
    match shape {
        Shape::DuoI64Bool => {
            store_i64_param(&mut body, shape.input(), 0);
            store_i32_param(&mut body, shape.input() + 8, 1);
        }
        Shape::DuoBoolI64 => {
            store_i32_param(&mut body, shape.input(), 0);
            store_i64_param(&mut body, shape.input() + 8, 1);
        }
        Shape::PhantomI64 | Shape::PhantomBool => {
            store_i32_param(&mut body, shape.input(), 0);
        }
    }
    body.push(0x41);
    write_i64(&mut body, i64::from(shape.input()));
    if matches!(shape, Shape::DuoI64Bool | Shape::DuoBoolI64) {
        body.extend([0x20, 0x02, 0x20, 0x03]);
    }
    body.push(0x41);
    write_i64(&mut body, i64::from(shape.internal()));
    body.push(0x10);
    write_u32(&mut body, selected_index);
    body.push(0x21);
    write_u32(&mut body, status);
    trap_invalid_status(&mut body, status);
    body.push(0x20);
    write_u32(&mut body, status);
    body.extend([0x45, 0x04, 0x40]);
    body.push(0x41);
    write_i64(&mut body, i64::from(shape.internal() + shape.bool_offset()));
    body.extend([0x28, 0x02, 0x00, 0x22]);
    write_u32(&mut body, flag);
    body.extend([0x41, 0x01, 0x4b, 0x04, 0x40, 0x00, 0x0b]);
    match shape {
        Shape::DuoI64Bool => {
            copy_i64(&mut body, shape.internal(), shape.result() + 8);
            store_i32_byte_local(&mut body, shape.result() + 16, flag);
        }
        Shape::DuoBoolI64 => {
            store_i32_byte_local(&mut body, shape.result() + 8, flag);
            copy_i64(&mut body, shape.internal() + 8, shape.result() + 16);
        }
        Shape::PhantomI64 | Shape::PhantomBool => {
            store_i32_byte_local(&mut body, shape.result() + 4, flag);
        }
    }
    store_tag(&mut body, shape.result(), 0);
    body.push(0x05);
    emit_normalized_status(&mut body, status);
    body.push(0x21);
    write_u32(&mut body, status);
    trap_invalid_status(&mut body, status);
    emit_status_fields(&mut body, status, shape.result() + shape.payload_offset());
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
        .unwrap_or_else(|| profile_error("generic-record v7 HIR resolution failed"))
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-WIT108", message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::*;

    fn artifact() -> PrivateGenericRecordCoreArtifactV7 {
        let program =
            crate::parse(SOURCE_V7, Path::new("component-generic-record-v7.spx")).unwrap();
        emit_private_generic_record_core_v7(&program).unwrap()
    }

    #[derive(Clone, Copy)]
    enum AdversarialOutcome {
        InvalidBool,
        InvalidTag,
        UnknownStatus,
    }

    fn adversarial_core(outcome: AdversarialOutcome) -> Vec<u8> {
        let program =
            crate::parse(SOURCE_V7, Path::new("adversarial-generic-record-v7.spx")).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        hir::validate(&resolved).unwrap();
        let ordered = validate_profile(&resolved).unwrap();
        let mut lowering =
            aggregate::lower_selected_functions(&resolved, &ordered, &ordered[0]).unwrap();
        for shape in Shape::ALL {
            let mut body = vec![0x00];
            match outcome {
                AdversarialOutcome::InvalidBool => {
                    let output_parameter = if matches!(shape, Shape::DuoI64Bool | Shape::DuoBoolI64)
                    {
                        3
                    } else {
                        1
                    };
                    body.push(0x20);
                    write_u32(&mut body, output_parameter);
                    body.extend([0x41, 0x02, 0x36, 0x02]);
                    write_u32(&mut body, shape.bool_offset() as u32);
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
        let mut layout_digests = [[0_u8; 32]; 4];
        for shape in Shape::ALL {
            let layout =
                AggregateLayout::for_type(&resolved, AggregateTarget::Wasm32, &shape.ty()).unwrap();
            require_layout(&layout, shape).unwrap();
            layout_digests[shape.index()] = layout.digest();
        }
        let graph_json = graph::to_json(&program).unwrap();
        compose(
            lowering,
            &[0, 1, 2, 3],
            &graph::revision(&program),
            Sha256::digest(graph_json.as_bytes()).into(),
            plan_digest(&layout_digests),
            layout_digests,
        )
        .unwrap()
    }

    #[test]
    fn deterministic_core_is_upstream_valid_and_exact_instance_bound() {
        let first = artifact();
        assert_eq!(first, artifact());
        assert_ne!(first.layout_digests[0], first.layout_digests[1]);
        assert_ne!(first.layout_digests[2], first.layout_digests[3]);
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&first.bytes)
            .expect("upstream validator rejected generic-record v7 core");
    }

    #[test]
    fn exact_source_semantic_and_instance_mutations_reject() {
        for hostile in [
            SOURCE_V7.replacen("Duo<T, U>", "Duo<U, T>", 1),
            SOURCE_V7.replacen("Duo<i64, bool>", "Duo<bool, i64>", 1),
            SOURCE_V7.replacen("left + delta", "left - delta", 1),
            SOURCE_V7.replacen("right + delta", "right - delta", 1),
            SOURCE_V7.replacen("delta != -99", "delta != -98", 1),
            SOURCE_V7.replacen("divisor != 13", "divisor != 12", 1),
            SOURCE_V7.replacen("Phantom<i64>", "Phantom<bool>", 1),
            SOURCE_V7.replacen("!input.marker", "input.marker", 1),
        ] {
            let parsed = crate::parse(&hostile, Path::new("hostile-v7.spx"));
            match parsed {
                Ok(program) => assert!(emit_private_generic_record_core_v7(&program).is_err()),
                Err(error) => assert!(!error.code.is_empty()),
            }
        }
    }

    #[test]
    fn node_executes_all_instances_status_order_poison_and_invalid_bools() {
        let artifact = artifact();
        let stem = format!("semaprax-generic-record-v7-{}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, artifact.bytes).unwrap();
        std::fs::write(
            &script_path,
            "import fs from 'node:fs';\nconst {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]));\nconst m=new DataView(instance.exports.memory.buffer);\nconst a=instance.exports.cabi_transform_i64_bool_v7;const b=instance.exports.cabi_transform_bool_i64_v7;const p64=instance.exports.cabi_preserve_phantom_i64_v7;const pb=instance.exports.cabi_invert_phantom_bool_v7;\nlet p=a(83n,1,1n,2n);if(m.getUint8(p)!==0||m.getBigInt64(p+8,true)!==42n||m.getUint8(p+16)!==1)throw Error('a');\np=b(0,83n,1n,2n);if(m.getUint8(p)!==0||m.getUint8(p+8)!==0||m.getBigInt64(p+16,true)!==42n)throw Error('b');\np=p64(1);if(m.getUint8(p)!==0||m.getUint8(p+4)!==1)throw Error('p64');p=pb(0);if(m.getUint8(p)!==0||m.getUint8(p+4)!==1)throw Error('pb');\np=a(9223372036854775807n,1,1n,0n);if(m.getUint8(p)!==1||m.getUint32(p+16,true)!==1)throw Error('sticky-a');p=b(1,9223372036854775807n,1n,0n);if(m.getUint8(p)!==1||m.getUint32(p+16,true)!==1)throw Error('sticky-b');\nfor(const [f,args] of [[a,[1n,1,1n,0n]],[b,[1,1n,1n,0n]]]){p=f(...args);if(m.getUint8(p)!==1||m.getUint32(p+16,true)!==4)throw Error('div0');}\nfor(const [f,args] of [[a,[1n,2,1n,2n]],[b,[2,1n,1n,2n]],[p64,[2]],[pb,[2]]]){let trapped=false;try{f(...args)}catch{trapped=true}if(!trapped)throw Error('bool2');}\nconsole.log('generic-record-v7-core-ok');\n",
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
            "Node generic-record v7 gate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn adversarial_selected_bodies_trap_before_publishing_poisoned_results() {
        let stem = format!("semaprax-generic-record-v7-hostile-{}", std::process::id());
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(
            &script_path,
            "import fs from 'node:fs';\nconst names=['cabi_transform_i64_bool_v7','cabi_transform_bool_i64_v7','cabi_preserve_phantom_i64_v7','cabi_invert_phantom_bool_v7'];const args=[[7n,1,1n,2n],[1,7n,1n,2n],[1],[0]];const results=[192,320,416,480];\nfor(const path of process.argv.slice(2)){const {instance}=await WebAssembly.instantiate(fs.readFileSync(path));const bytes=new Uint8Array(instance.exports.memory.buffer);for(let i=0;i<4;i++){bytes.fill(0x3c,results[i],results[i]+24);let trapped=false;try{instance.exports[names[i]](...args[i])}catch{trapped=true}if(!trapped)throw Error(`hostile did not trap ${path} ${names[i]}`);for(let j=0;j<24;j++)if(bytes[results[i]+j]!==0xa5)throw Error(`published byte ${path} ${names[i]} ${j}`)}}\nconsole.log('generic-record-v7-hostiles-ok');\n",
        )
        .unwrap();
        let mut wasm_paths = Vec::new();
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
                .expect("upstream validator rejected adversarial v7 core");
            std::fs::write(&path, bytes).unwrap();
            wasm_paths.push(path);
        }
        let output = Command::new("node")
            .arg(&script_path)
            .args(&wasm_paths)
            .output()
            .expect("Node is required by the existing Wasm gate");
        let _ = std::fs::remove_file(&script_path);
        for path in wasm_paths {
            let _ = std::fs::remove_file(path);
        }
        assert!(
            output.status.success(),
            "Node generic-record v7 hostile gate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
