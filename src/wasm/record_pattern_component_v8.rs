//! Closed monomorphic record-pattern core for private Component Model v8 evidence.

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::aggregate_layout::{AggregateLayout, AggregateTarget};
use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::graph;
use crate::hir::{
    self, DeclarationId, IdentityOrigin, ResolvedExprKind, ResolvedMatchPattern, ResolvedProgram,
    ResolvedRecordMatchFieldPattern, ResolvedType, ResolvedTypeDeclarationKind,
};

use super::{
    aggregate, intern_type, section, write_bytes, write_i64, write_name, write_u32, Signature, I32,
    I64,
};

const PHANTOM_ID: &str = "component.pattern.phantom";
const MARKER_ID: &str = "component.pattern.phantom.marker";
const FUNCTION_IDS: [&str; 4] = [
    "component.pattern.preserve-phantom-i64",
    "component.pattern.invert-phantom-i64",
    "component.pattern.preserve-phantom-bool",
    "component.pattern.invert-phantom-bool",
];
pub(crate) const CANONICAL_EXPORTS: [&str; 4] = [
    "cabi_preserve_pattern_phantom_i64_v8",
    "cabi_invert_pattern_phantom_i64_v8",
    "cabi_preserve_pattern_phantom_bool_v8",
    "cabi_invert_pattern_phantom_bool_v8",
];

const CONTRACT_DOMAIN: &str = "semaprax.contract.v1";
const POISON_I32: i32 = 0xa5a5_a5a5_u32 as i32;
const CUSTOM_SECTION: &str = "semaprax.component-record-pattern-v8";
const PLAN_DOMAIN: &[u8] = b"semaprax.component-record-pattern-plan.v8\0";
const CONTRACT_REQUIRES: i32 = status_word(1, 1);
const CONTRACT_ENSURES: i32 = status_word(1, 2);

const fn status_word(class: i32, code: i32) -> i32 {
    (class << 24) | code
}

pub(crate) const SOURCE_V8: &str = r#"module test.component_record_pattern_v8;

@id("component.pattern.phantom")
record Phantom<T> {
    @id("component.pattern.phantom.marker") marker: bool,
}

@id("component.pattern.preserve-phantom-i64")
fn preserve_phantom_i64(input: Phantom<i64>, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    match input { Phantom { marker } => marker, }
}

@id("component.pattern.invert-phantom-i64")
fn invert_phantom_i64(input: Phantom<i64>, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    match input { Phantom { marker } => !marker, }
}

@id("component.pattern.preserve-phantom-bool")
fn preserve_phantom_bool(input: Phantom<bool>, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    match input { Phantom { marker } => marker, }
}

@id("component.pattern.invert-phantom-bool")
fn invert_phantom_bool(input: Phantom<bool>, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    match input { Phantom { marker } => !marker, }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Shape {
    PreserveI64,
    InvertI64,
    PreserveBool,
    InvertBool,
}

impl Shape {
    const ALL: [Self; 4] = [
        Self::PreserveI64,
        Self::InvertI64,
        Self::PreserveBool,
        Self::InvertBool,
    ];

    const fn index(self) -> usize {
        match self {
            Self::PreserveI64 => 0,
            Self::InvertI64 => 1,
            Self::PreserveBool => 2,
            Self::InvertBool => 3,
        }
    }

    const fn layout_index(self) -> usize {
        match self {
            Self::PreserveI64 | Self::InvertI64 => 0,
            Self::PreserveBool | Self::InvertBool => 1,
        }
    }

    const fn invert(self) -> bool {
        matches!(self, Self::InvertI64 | Self::InvertBool)
    }

    fn ty(self) -> ResolvedType {
        let argument = if self.layout_index() == 0 {
            ResolvedType::I64
        } else {
            ResolvedType::Bool
        };
        ResolvedType::Nominal {
            declaration: DeclarationId::new(PHANTOM_ID),
            arguments: vec![argument],
        }
    }

    const fn input(self) -> i32 {
        [128, 192, 256, 320][self.index()]
    }

    const fn internal(self) -> i32 {
        [144, 208, 272, 336][self.index()]
    }

    const fn result(self) -> i32 {
        [160, 224, 288, 352][self.index()]
    }

    fn canonical_signature(self) -> Signature {
        Signature {
            params: vec![I32, I64],
            results: vec![I32],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateRecordPatternCoreArtifactV8 {
    pub(crate) bytes: Vec<u8>,
    pub(crate) source_revision: String,
    pub(crate) graph_digest: [u8; 32],
    pub(crate) plan_digest: [u8; 32],
    pub(crate) layout_digests: [[u8; 32]; 2],
}

pub(crate) fn emit_private_record_pattern_core_v8(
    program: &Program,
) -> Result<PrivateRecordPatternCoreArtifactV8, Diagnostic> {
    require_exact_source(program)?;
    let resolved = hir::resolve(program).map_err(first_error)?;
    hir::validate(&resolved)?;
    let ordered = validate_profile(&resolved)?;
    let instances = [Shape::PreserveI64.ty(), Shape::PreserveBool.ty()];
    let mut layout_digests = [[0_u8; 32]; 2];
    for (index, instance) in instances.iter().enumerate() {
        let layout = AggregateLayout::for_type(&resolved, AggregateTarget::Wasm32, instance)?;
        require_layout(&layout, instance)?;
        layout_digests[index] = layout.digest();
    }
    if layout_digests[0] == layout_digests[1] {
        return Err(profile_error(
            "record-pattern Phantom instances lost exact identity",
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
                "record-pattern selected lowerings disagree on their shared closure",
            ));
        }
        selected_indexes.push(lowering.selected_index);
    }
    let source_revision = graph::revision(program);
    let graph_json = graph::to_json(program).map_err(first_error)?;
    if !graph_json.starts_with("{\"schema\":\"semaprax.graph.v13\",") {
        return Err(profile_error("record-pattern v8 requires exact Graph v13"));
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
    Ok(PrivateRecordPatternCoreArtifactV8 {
        bytes,
        source_revision,
        graph_digest,
        plan_digest,
        layout_digests,
    })
}

fn require_exact_source(program: &Program) -> Result<(), Diagnostic> {
    let expected = crate::parse(
        SOURCE_V8,
        std::path::Path::new("record-pattern-v8-profile.spx"),
    )?;
    if crate::format::canonical(program) != crate::format::canonical(&expected) {
        return Err(profile_error(
            "record-pattern v8 requires exact monomorphic frozen source",
        ));
    }
    Ok(())
}

fn validate_profile(program: &ResolvedProgram) -> Result<Vec<DeclarationId>, Diagnostic> {
    if !program.permits.is_empty()
        || !program.interfaces.is_empty()
        || !program.function_templates.is_empty()
        || !program.function_instances.is_empty()
        || program.functions.len() != 5
    {
        return Err(profile_error(
            "record-pattern v8 requires four exports and app.main without authority",
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
    if authored.len() != 1
        || authored[0].id != DeclarationId::new(PHANTOM_ID)
        || authored[0].type_parameters.len() != 1
        || !matches!(authored[0].kind, ResolvedTypeDeclarationKind::Record { .. })
    {
        return Err(profile_error(
            "record-pattern v8 template identity or parameters changed",
        ));
    }
    let expected_ids = FUNCTION_IDS.map(DeclarationId::new);
    for (index, shape) in Shape::ALL.into_iter().enumerate() {
        let function = &program.functions[index];
        if function.id != expected_ids[index]
            || function.params.len() != 2
            || function.params[0].ty != shape.ty()
            || function.params[1].ty != ResolvedType::I64
            || function.return_type != ResolvedType::Bool
            || !function.effects.is_empty()
            || function.requires.len() != 1
            || function.ensures.len() != 1
            || !is_exact_control_contract(&function.requires[0], &function.params[1].id, -99)
            || !is_exact_control_contract(&function.ensures[0], &function.params[1].id, 13)
        {
            return Err(profile_error(
                "record-pattern v8 export signature or contract changed",
            ));
        }
        validate_exact_pattern_body(&function.body, shape, &function.params[0].id)?;
    }
    let main = &program.functions[4];
    if main.id != DeclarationId::new("app.main")
        || !main.params.is_empty()
        || main.return_type != ResolvedType::I64
        || !main.effects.is_empty()
        || !main.requires.is_empty()
        || !main.ensures.is_empty()
        || !matches!(
            &main.body.kind,
            ResolvedExprKind::Block { statements, tail }
                if statements.is_empty() && matches!(tail.kind, ResolvedExprKind::Int(0))
        )
    {
        return Err(profile_error("record-pattern v8 app.main shape changed"));
    }
    Ok(expected_ids.to_vec())
}

fn validate_exact_pattern_body(
    body: &crate::hir::ResolvedExpr,
    shape: Shape,
    input_id: &crate::hir::ValueId,
) -> Result<(), Diagnostic> {
    let ResolvedExprKind::Block { statements, tail } = &body.kind else {
        return Err(profile_error(
            "record-pattern v8 body is not an exact block",
        ));
    };
    if !statements.is_empty() {
        return Err(profile_error("record-pattern v8 body gained statements"));
    }
    let ResolvedExprKind::Match { scrutinee, arms } = &tail.kind else {
        return Err(profile_error(
            "record-pattern v8 body lost its explicit match",
        ));
    };
    let ResolvedExprKind::Place(scrutinee_place) = &scrutinee.kind else {
        return Err(profile_error(
            "record-pattern v8 scrutinee is not the exact input place",
        ));
    };
    if arms.len() != 1
        || scrutinee.ty != shape.ty()
        || &scrutinee_place.root != input_id
        || !scrutinee_place.projections.is_empty()
    {
        return Err(profile_error("record-pattern v8 match shape changed"));
    }
    let ResolvedMatchPattern::Record {
        record,
        instance,
        fields,
    } = &arms[0].pattern
    else {
        return Err(profile_error("record-pattern v8 match is not explicit"));
    };
    if fields.len() != 1 {
        return Err(profile_error(
            "record-pattern v8 requires one exact marker field",
        ));
    }
    let ResolvedRecordMatchFieldPattern::Binding(marker) = &fields[0].pattern else {
        return Err(profile_error(
            "record-pattern v8 marker is not an exact binding",
        ));
    };
    if record != &DeclarationId::new(PHANTOM_ID)
        || instance != &shape.ty()
        || fields[0].field != DeclarationId::new(MARKER_ID)
        || marker.ty != ResolvedType::Bool
        || marker.ownership != crate::hir::OwnershipMode::Value
    {
        return Err(profile_error(
            "record-pattern v8 exact instance or marker binding changed",
        ));
    }
    let value = &arms[0].value;
    let exact_marker = |candidate: &crate::hir::ResolvedExpr| {
        candidate.ty == ResolvedType::Bool
            && matches!(
                &candidate.kind,
                ResolvedExprKind::Place(place)
                    if place.root == marker.id && place.projections.is_empty()
            )
    };
    match (&value.kind, shape.invert()) {
        (ResolvedExprKind::Place(_), false) if exact_marker(value) => Ok(()),
        (
            ResolvedExprKind::Unary {
                op: crate::ast::UnaryOp::Not,
                value: operand,
            },
            true,
        ) if exact_marker(operand) => Ok(()),
        _ => Err(profile_error(
            "record-pattern v8 preserve/invert polarity changed",
        )),
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

fn require_layout(layout: &AggregateLayout, instance: &ResolvedType) -> Result<(), Diagnostic> {
    if layout.instance != *instance
        || layout.record != DeclarationId::new(PHANTOM_ID)
        || layout.size != 4
        || layout.align != 4
        || layout.fields.len() != 1
        || layout.fields[0].field != DeclarationId::new(MARKER_ID)
        || layout.fields[0].ty != ResolvedType::Bool
        || layout.fields[0].offset != 0
    {
        return Err(profile_error(
            "record-pattern v8 Wasm32 field layout changed",
        ));
    }
    Ok(())
}

fn plan_digest(layout_digests: &[[u8; 32]; 2]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PLAN_DOMAIN);
    for shape in Shape::ALL {
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
        hash.update([u8::from(shape.invert())]);
        hash.update([if shape.layout_index() == 0 { 5 } else { 6 }]);
        hash.update(layout_digests[shape.layout_index()]);
    }
    hash.finalize().into()
}

fn compose(
    lowering: aggregate::SelectedAggregateLowering,
    selected_indexes: &[u32],
    source_revision: &str,
    graph_digest: [u8; 32],
    plan_digest: [u8; 32],
    layout_digests: [[u8; 32]; 2],
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
            "record-pattern v8 selected function map changed",
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
    for _ in 0..4 {
        write_u32(&mut functions, canonical_type);
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
    write_u32(&mut data, 1);
    active_data(&mut data, 0, CONTRACT_DOMAIN.as_bytes());
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
    store_i32_local(&mut body, shape.input(), 0);
    body.push(0x41);
    write_i64(&mut body, i64::from(shape.input()));
    body.extend([0x20, 0x01, 0x41]);
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

fn store_i32_local(output: &mut Vec<u8>, address: i32, local: u32) {
    output.push(0x41);
    write_i64(output, i64::from(address));
    output.push(0x20);
    write_u32(output, local);
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
        .unwrap_or_else(|| profile_error("record-pattern v8 HIR resolution failed"))
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W112", message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::*;

    fn artifact() -> PrivateRecordPatternCoreArtifactV8 {
        let program =
            crate::parse(SOURCE_V8, Path::new("component-record-pattern-v8.spx")).unwrap();
        emit_private_record_pattern_core_v8(&program).unwrap()
    }

    #[derive(Clone, Copy)]
    enum AdversarialOutcome {
        InvalidBool,
        InvalidTag,
        UnknownStatus,
    }

    fn adversarial_core(outcome: AdversarialOutcome) -> Vec<u8> {
        let program =
            crate::parse(SOURCE_V8, Path::new("record-pattern-v8-adversarial.spx")).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        hir::validate(&resolved).unwrap();
        let ordered = validate_profile(&resolved).unwrap();
        let mut lowering =
            aggregate::lower_selected_functions(&resolved, &ordered, &ordered[0]).unwrap();
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
        let instances = [Shape::PreserveI64.ty(), Shape::PreserveBool.ty()];
        let mut layout_digests = [[0_u8; 32]; 2];
        for (index, instance) in instances.iter().enumerate() {
            let layout =
                AggregateLayout::for_type(&resolved, AggregateTarget::Wasm32, instance).unwrap();
            require_layout(&layout, instance).unwrap();
            layout_digests[index] = layout.digest();
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
    fn deterministic_core_is_upstream_valid_and_graph_v13_bound() {
        let first = artifact();
        assert_eq!(first, artifact());
        assert_ne!(first.layout_digests[0], first.layout_digests[1]);
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&first.bytes)
            .expect("upstream validator rejected record-pattern v8 core");
    }

    #[test]
    fn generic_and_pattern_semantic_mutations_reject() {
        for hostile in [
            SOURCE_V8.replacen("Phantom<T>", "Phantom<i64>", 1),
            SOURCE_V8.replacen("Phantom<i64>", "Phantom<bool>", 1),
            SOURCE_V8.replacen("control != -99", "control != -98", 1),
            SOURCE_V8.replacen("control != 13", "control != 12", 1),
            SOURCE_V8.replacen("Phantom { marker }", "_", 1),
            SOURCE_V8.replacen("=> marker", "=> !marker", 1),
            SOURCE_V8.replacen("fn preserve_phantom_i64", "fn preserve_phantom_i64<T>", 1),
        ] {
            let parsed = crate::parse(&hostile, Path::new("hostile-record-pattern-v8.spx"));
            match parsed {
                Ok(program) => assert!(emit_private_record_pattern_core_v8(&program).is_err()),
                Err(error) => assert!(!error.code.is_empty()),
            }
        }
    }

    #[test]
    fn node_executes_all_patterns_contracts_and_invalid_input_bools() {
        let artifact = artifact();
        let stem = format!("semaprax-record-pattern-v8-{}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, artifact.bytes).unwrap();
        std::fs::write(
            &script_path,
            "import fs from 'node:fs';\nconst {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]));const m=new DataView(instance.exports.memory.buffer);const u=new Uint8Array(instance.exports.memory.buffer);const names=['cabi_preserve_pattern_phantom_i64_v8','cabi_invert_pattern_phantom_i64_v8','cabi_preserve_pattern_phantom_bool_v8','cabi_invert_pattern_phantom_bool_v8'];const f=names.map(n=>instance.exports[n]);const results=[160,224,288,352];for(let i=0;i<4;i++){for(const b of [0,1]){let p=f[i](b,0n);if(m.getUint8(p)!==0||m.getUint8(p+4)!==(i%2?1-b:b))throw Error('semantic')}}for(let i=0;i<4;i++){let p=f[i](1,-99n);if(m.getUint8(p)!==1||m.getUint32(p+12,true)!==1)throw Error('requires');p=f[i](1,13n);if(m.getUint8(p)!==1||m.getUint32(p+12,true)!==2)throw Error('ensures');u.fill(0x3c,results[i],results[i]+20);let trapped=false;try{f[i](2,0n)}catch{trapped=true}if(!trapped)throw Error('bool2');for(let j=0;j<20;j++)if(u[results[i]+j]!==0xa5)throw Error(`bool2-poison-${i}-${j}`)}console.log('record-pattern-v8-core-ok');\n",
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
            "Node record-pattern v8 gate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn adversarial_status_and_output_bools_trap_before_any_publication() {
        let stem = format!("semaprax-record-pattern-v8-hostile-{}", std::process::id());
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(
            &script_path,
            "import fs from 'node:fs';\nconst names=['cabi_preserve_pattern_phantom_i64_v8','cabi_invert_pattern_phantom_i64_v8','cabi_preserve_pattern_phantom_bool_v8','cabi_invert_pattern_phantom_bool_v8'];const results=[160,224,288,352];for(const path of process.argv.slice(2)){const {instance}=await WebAssembly.instantiate(fs.readFileSync(path));const u=new Uint8Array(instance.exports.memory.buffer);for(let i=0;i<4;i++){u.fill(0x3c,results[i],results[i]+20);let trapped=false;try{instance.exports[names[i]](1,0n)}catch{trapped=true}if(!trapped)throw Error(`hostile-no-trap-${path}-${i}`);for(let j=0;j<20;j++)if(u[results[i]+j]!==0xa5)throw Error(`hostile-published-${path}-${i}-${j}`)}}console.log('record-pattern-v8-hostiles-ok');\n",
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
                .expect("upstream validator rejected adversarial v8 core");
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
            "Node record-pattern v8 hostile gate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
