//! Closed direct-scalar `Option`/`Result` core for private Component Model v5 evidence.

use std::collections::HashMap;

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::graph;
use crate::hir::{
    self, DeclarationId, IdentityOrigin, ResolvedExpr, ResolvedExprKind, ResolvedProgram,
    ResolvedType,
};
use crate::prelude;
use crate::variant_layout::{VariantLayout, VariantLayoutCache, VariantTarget};

use super::{
    aggregate, intern_type, section, write_bytes, write_i64, write_name, write_u32, Signature, I32,
    I64,
};

const IDS: [&str; 6] = [
    "component.option-i64",
    "component.option-bool",
    "component.result-i64-i64",
    "component.result-i64-bool",
    "component.result-bool-i64",
    "component.result-bool-bool",
];

pub(crate) const CANONICAL_EXPORTS: [&str; 6] = [
    "cabi_option_i64_v5",
    "cabi_option_bool_v5",
    "cabi_result_i64_i64_v5",
    "cabi_result_i64_bool_v5",
    "cabi_result_bool_i64_v5",
    "cabi_result_bool_bool_v5",
];

const CONTRACT_DOMAIN: &str = "semaprax.contract.v1";
const ARITHMETIC_DOMAIN: &str = "semaprax.arithmetic.v1";
const INTERNAL_RESULT_AREA: i32 = 128;
pub(crate) const RESULT_AREA: i32 = 256;
const POISON_I64: i64 = 0xa5a5_a5a5_a5a5_a5a5_u64 as i64;
const CUSTOM_SECTION: &str = "semaprax.component-scalar-algebra-v5";

pub(crate) const SOURCE_V5: &str = r#"module test.component_scalar_algebra_v5;

@id("component.option-i64")
fn option_i64(value: i64, select: bool, divisor: i64) -> Option<i64>
    requires value != -99
    ensures divisor != 13
{
    if select { Option<i64>::Some { value: (value + 1) / divisor } } else { Option<i64>::None {} }
}

@id("component.option-bool")
fn option_bool(value: i64, select: bool, divisor: i64) -> Option<bool>
    requires value != -99
    ensures divisor != 13
{
    if select { Option<bool>::Some { value: (value + 1) / divisor > 0 } } else { Option<bool>::None {} }
}

@id("component.result-i64-i64")
fn result_i64_i64(value: i64, select: bool, divisor: i64) -> Result<i64, i64>
    requires value != -99
    ensures divisor != 13
{
    if select { Result<i64, i64>::Err { error: value } } else { Result<i64, i64>::Ok { value: (value + 1) / divisor } }
}

@id("component.result-i64-bool")
fn result_i64_bool(value: i64, select: bool, divisor: i64) -> Result<i64, bool>
    requires value != -99
    ensures divisor != 13
{
    if select { Result<i64, bool>::Err { error: value > 0 } } else { Result<i64, bool>::Ok { value: (value + 1) / divisor } }
}

@id("component.result-bool-i64")
fn result_bool_i64(value: i64, select: bool, divisor: i64) -> Result<bool, i64>
    requires value != -99
    ensures divisor != 13
{
    if select { Result<bool, i64>::Err { error: value } } else { Result<bool, i64>::Ok { value: (value + 1) / divisor > 0 } }
}

@id("component.result-bool-bool")
fn result_bool_bool(value: i64, select: bool, divisor: i64) -> Result<bool, bool>
    requires value != -99
    ensures divisor != 13
{
    if select { Result<bool, bool>::Err { error: value > 0 } } else { Result<bool, bool>::Ok { value: (value + 1) / divisor > 0 } }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const CONTRACT_REQUIRES: i32 = status_word(1, 1);
const CONTRACT_ENSURES: i32 = status_word(1, 2);
const ARITHMETIC_BASE: i32 = status_word(2, 0);

const fn status_word(class: i32, code: i32) -> i32 {
    (class << 24) | code
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScalarAlgebraShapeV5 {
    OptionI64,
    OptionBool,
    ResultI64I64,
    ResultI64Bool,
    ResultBoolI64,
    ResultBoolBool,
}

impl ScalarAlgebraShapeV5 {
    const ALL: [Self; 6] = [
        Self::OptionI64,
        Self::OptionBool,
        Self::ResultI64I64,
        Self::ResultI64Bool,
        Self::ResultBoolI64,
        Self::ResultBoolBool,
    ];

    fn ty(self) -> ResolvedType {
        match self {
            Self::OptionI64 => option_type(ResolvedType::I64),
            Self::OptionBool => option_type(ResolvedType::Bool),
            Self::ResultI64I64 => result_type(ResolvedType::I64, ResolvedType::I64),
            Self::ResultI64Bool => result_type(ResolvedType::I64, ResolvedType::Bool),
            Self::ResultBoolI64 => result_type(ResolvedType::Bool, ResolvedType::I64),
            Self::ResultBoolBool => result_type(ResolvedType::Bool, ResolvedType::Bool),
        }
    }

    const fn internal_payload_offset(self) -> i32 {
        match self {
            Self::OptionBool | Self::ResultBoolBool => 4,
            _ => 8,
        }
    }

    const fn carrier_payload_offset(self) -> i32 {
        match self {
            Self::OptionBool | Self::ResultBoolBool => 1,
            _ => 8,
        }
    }

    const fn outer_payload_offset(self) -> i32 {
        match self {
            Self::OptionBool | Self::ResultBoolBool => 4,
            _ => 8,
        }
    }

    const fn payload_kind(self, tag: u8) -> Option<PayloadKind> {
        match (self, tag) {
            (Self::OptionI64 | Self::OptionBool, 0) => None,
            (Self::OptionI64, 1) => Some(PayloadKind::I64),
            (Self::OptionBool, 1) => Some(PayloadKind::Bool),
            (Self::ResultI64I64, 0 | 1) => Some(PayloadKind::I64),
            (Self::ResultI64Bool, 0) => Some(PayloadKind::I64),
            (Self::ResultI64Bool, 1) => Some(PayloadKind::Bool),
            (Self::ResultBoolI64, 0) => Some(PayloadKind::Bool),
            (Self::ResultBoolI64, 1) => Some(PayloadKind::I64),
            (Self::ResultBoolBool, 0 | 1) => Some(PayloadKind::Bool),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PayloadKind {
    I64,
    Bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateScalarAlgebraCoreArtifactV5 {
    pub(crate) bytes: Vec<u8>,
    pub(crate) source_revision: String,
    pub(crate) layout_digests: [[u8; 32]; 6],
    pub(crate) prelude_digest: [u8; 32],
}

pub(crate) fn emit_private_scalar_algebra_core_v5(
    program: &Program,
) -> Result<PrivateScalarAlgebraCoreArtifactV5, Diagnostic> {
    require_exact_source(program)?;
    let resolved = hir::resolve(program).map_err(first_error)?;
    hir::validate(&resolved)?;
    let ordered = validate_profile(&resolved)?;
    let cache = VariantLayoutCache::build(&resolved, VariantTarget::Wasm32)?;
    let mut layout_digests = [[0_u8; 32]; 6];
    for (index, shape) in ScalarAlgebraShapeV5::ALL.into_iter().enumerate() {
        let layout = cache.layout(&shape.ty())?;
        require_layout(layout, shape)?;
        layout_digests[index] = layout.digest();
    }

    let mut lowerings = Vec::with_capacity(IDS.len());
    for id in IDS {
        lowerings.push(aggregate::lower_selected_functions(
            &resolved,
            &ordered,
            &DeclarationId::new(id),
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
                "scalar-algebra selected lowerings disagree on the shared function closure",
            ));
        }
        selected_indexes.push(lowering.selected_index);
    }

    let source_revision = graph::revision(program);
    let prelude_digest = prelude::digest_v1();
    let bytes = compose(
        primary,
        &selected_indexes,
        &source_revision,
        prelude_digest,
        layout_digests,
    )?;
    Ok(PrivateScalarAlgebraCoreArtifactV5 {
        bytes,
        source_revision,
        layout_digests,
        prelude_digest,
    })
}

fn require_exact_source(program: &Program) -> Result<(), Diagnostic> {
    let expected = crate::parse(
        SOURCE_V5,
        std::path::Path::new("scalar-algebra-v5-profile.spx"),
    )?;
    if crate::format::canonical(program) != crate::format::canonical(&expected) {
        return Err(profile_error(
            "scalar-algebra v5 requires the exact frozen source semantics",
        ));
    }
    Ok(())
}

fn option_type(value: ResolvedType) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(prelude::OPTION_ID),
        arguments: vec![value],
    }
}

fn result_type(ok: ResolvedType, err: ResolvedType) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(prelude::RESULT_ID),
        arguments: vec![ok, err],
    }
}

fn require_layout(layout: &VariantLayout, shape: ScalarAlgebraShapeV5) -> Result<(), Diagnostic> {
    let (variant, cases, size, align, payload_offset) = match shape {
        ScalarAlgebraShapeV5::OptionI64 => (
            prelude::OPTION_ID,
            [prelude::OPTION_NONE_ID, prelude::OPTION_SOME_ID],
            16,
            8,
            8,
        ),
        ScalarAlgebraShapeV5::OptionBool => (
            prelude::OPTION_ID,
            [prelude::OPTION_NONE_ID, prelude::OPTION_SOME_ID],
            8,
            4,
            4,
        ),
        ScalarAlgebraShapeV5::ResultI64I64
        | ScalarAlgebraShapeV5::ResultI64Bool
        | ScalarAlgebraShapeV5::ResultBoolI64 => (
            prelude::RESULT_ID,
            [prelude::RESULT_OK_ID, prelude::RESULT_ERR_ID],
            16,
            8,
            8,
        ),
        ScalarAlgebraShapeV5::ResultBoolBool => (
            prelude::RESULT_ID,
            [prelude::RESULT_OK_ID, prelude::RESULT_ERR_ID],
            8,
            4,
            4,
        ),
    };
    if layout.instance != shape.ty()
        || layout.variant != DeclarationId::new(variant)
        || layout.size != size
        || layout.align != align
        || layout.tag_size != 4
        || layout.payload_offset != payload_offset
        || layout.cases.len() != 2
        || layout.cases[0].case != DeclarationId::new(cases[0])
        || layout.cases[0].tag != 0
        || layout.cases[1].case != DeclarationId::new(cases[1])
        || layout.cases[1].tag != 1
    {
        return Err(profile_error(
            "scalar-algebra Wasm32 layout-v2 binding changed",
        ));
    }
    Ok(())
}

fn validate_profile(program: &ResolvedProgram) -> Result<Vec<DeclarationId>, Diagnostic> {
    if !program.permits.is_empty()
        || !program.interfaces.is_empty()
        || program.types.iter().any(|declaration| {
            !program
                .declarations
                .declaration(&declaration.id)
                .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned)
        })
        || program.functions.len() != 7
    {
        return Err(profile_error(
            "scalar-algebra v5 admits only its six exports, app.main, and compiler prelude types",
        ));
    }
    let mut expected_ids = IDS.into_iter().map(DeclarationId::new).collect::<Vec<_>>();
    expected_ids.push(DeclarationId::new("app.main"));
    if program
        .functions
        .iter()
        .map(|function| &function.id)
        .ne(expected_ids.iter())
    {
        return Err(profile_error(
            "scalar-algebra v5 function identities or declaration order changed",
        ));
    }
    for (function, shape) in program
        .functions
        .iter()
        .take(6)
        .zip(ScalarAlgebraShapeV5::ALL)
    {
        if function.params.len() != 3
            || function.params[0].ty != ResolvedType::I64
            || function.params[1].ty != ResolvedType::Bool
            || function.params[2].ty != ResolvedType::I64
            || function.return_type != shape.ty()
            || !function.effects.is_empty()
            || function.requires.len() != 1
            || function.ensures.len() != 1
        {
            return Err(profile_error(
                "scalar-algebra v5 export signature, effects, or contract shape changed",
            ));
        }
        for expr in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(function.ensures.iter())
        {
            validate_expr(program, expr)?;
        }
    }
    let main = &program.functions[6];
    if !main.params.is_empty()
        || main.return_type != ResolvedType::I64
        || !main.effects.is_empty()
        || !main.requires.is_empty()
        || !main.ensures.is_empty()
    {
        return Err(profile_error("scalar-algebra v5 app.main shape changed"));
    }
    validate_expr(program, &main.body)?;
    Ok(program
        .functions
        .iter()
        .take(6)
        .map(|function| function.id.clone())
        .collect())
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
        | ResolvedExprKind::ConstructVariant { .. } => {}
        _ => {
            return Err(profile_error(
                "scalar-algebra v5 admits only closed direct-copy variant expressions",
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
        } if (declaration == &DeclarationId::new(prelude::OPTION_ID)
            || declaration == &DeclarationId::new(prelude::RESULT_ID))
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
            "scalar-algebra v5 excludes type `{}`",
            ty.identity_key()
        ))),
    }
}

fn walk_children(expr: &ResolvedExpr, mut visit: impl FnMut(&ResolvedExpr)) {
    match &expr.kind {
        ResolvedExprKind::Unary { value, .. } => visit(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            visit(left);
            visit(right);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                visit(statement.value());
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
    selected_indexes: &[u32],
    source_revision: &str,
    prelude_digest: [u8; 32],
    layout_digests: [[u8; 32]; 6],
) -> Result<Vec<u8>, Diagnostic> {
    let aggregate::SelectedAggregateLowering {
        mut types,
        function_type_indexes,
        bodies: mut source_bodies,
        ..
    } = lowering;
    if selected_indexes.len() != 6 || source_bodies.len() != 6 {
        return Err(profile_error(
            "scalar-algebra v5 core function count changed",
        ));
    }
    let mut type_indexes = types
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, signature)| (signature, index as u32))
        .collect::<HashMap<_, _>>();
    let canonical_type = intern_type(
        Signature {
            params: vec![I64, I32, I64],
            results: vec![I32],
        },
        &mut types,
        &mut type_indexes,
    );
    let canonical_start = u32::try_from(source_bodies.len())
        .map_err(|_| profile_error("scalar-algebra v5 function index overflows u32"))?;
    for (shape, selected) in ScalarAlgebraShapeV5::ALL
        .into_iter()
        .zip(selected_indexes.iter().copied())
    {
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

    let mut function_section = Vec::new();
    write_u32(&mut function_section, 12);
    for index in function_type_indexes {
        write_u32(&mut function_section, index);
    }
    for _ in 0..6 {
        write_u32(&mut function_section, canonical_type);
    }
    section(&mut module, 3, function_section);
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
    write_u32(&mut data, 2);
    active_data(&mut data, 0, CONTRACT_DOMAIN.as_bytes());
    active_data(&mut data, 32, ARITHMETIC_DOMAIN.as_bytes());
    section(&mut module, 11, data);

    let mut custom = Vec::new();
    write_name(&mut custom, CUSTOM_SECTION);
    write_name(&mut custom, source_revision);
    custom.extend(prelude_digest);
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

fn canonical_adapter_body(shape: ScalarAlgebraShapeV5, selected_index: u32) -> Vec<u8> {
    let status = 3_u32;
    let tag = 4_u32;
    let mut body = vec![0x01, 0x03, I32];
    for offset in [RESULT_AREA, RESULT_AREA + 8, RESULT_AREA + 16] {
        body.push(0x41);
        write_i64(&mut body, i64::from(offset));
        body.push(0x42);
        write_i64(&mut body, POISON_I64);
        body.extend([0x37, 0x03, 0x00]);
    }
    // Raw-core callers must also provide canonical booleans.
    body.extend([0x20, 0x01, 0x41, 0x01, 0x4b, 0x04, 0x40, 0x00, 0x0b]);
    body.extend([0x20, 0x00, 0x20, 0x01, 0x20, 0x02, 0x41]);
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

    // Success: validate the compiler representation before reading a payload.
    body.push(0x41);
    write_i64(&mut body, i64::from(INTERNAL_RESULT_AREA));
    body.extend([0x28, 0x02, 0x00, 0x22]);
    write_u32(&mut body, tag);
    body.extend([0x41, 0x02, 0x4f, 0x04, 0x40, 0x00, 0x0b]);

    for candidate_tag in [0_u8, 1_u8] {
        body.push(0x20);
        write_u32(&mut body, tag);
        body.extend([0x41, candidate_tag, 0x46, 0x04, 0x40]);
        if let Some(kind) = shape.payload_kind(candidate_tag) {
            emit_payload(
                &mut body,
                kind,
                INTERNAL_RESULT_AREA + shape.internal_payload_offset(),
                RESULT_AREA + shape.outer_payload_offset() + shape.carrier_payload_offset(),
            );
        }
        body.push(0x0b);
    }
    // Publish the inner carrier tag and outer success tag last.
    body.push(0x41);
    write_i64(
        &mut body,
        i64::from(RESULT_AREA + shape.outer_payload_offset()),
    );
    body.push(0x20);
    write_u32(&mut body, tag);
    body.extend([0x3a, 0x00, 0x00]);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA));
    body.extend([0x41, 0x00, 0x3a, 0x00, 0x00, 0x05]);

    // Failure: reject unknown statuses, then publish status fields and tag.
    emit_normalized_status(&mut body, status);
    body.push(0x21);
    write_u32(&mut body, status);
    body.push(0x20);
    write_u32(&mut body, status);
    body.push(0x41);
    write_i64(&mut body, i64::from(aggregate::STATUS_INTERNAL_INVALID_TAG));
    body.extend([0x46, 0x04, 0x40, 0x00, 0x0b]);
    emit_status_fields(
        &mut body,
        status,
        RESULT_AREA + shape.outer_payload_offset(),
    );
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA));
    body.extend([0x41, 0x01, 0x3a, 0x00, 0x00, 0x0b]);
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_AREA));
    body.push(0x0b);
    body
}

fn emit_payload(output: &mut Vec<u8>, kind: PayloadKind, source: i32, target: i32) {
    output.push(0x41);
    write_i64(output, i64::from(target));
    output.push(0x41);
    write_i64(output, i64::from(source));
    match kind {
        PayloadKind::I64 => output.extend([0x29, 0x03, 0x00, 0x37, 0x03, 0x00]),
        PayloadKind::Bool => {
            output.extend([
                0x28, 0x02, 0x00, 0x22, 0x05, 0x41, 0x01, 0x4b, 0x04, 0x40, 0x00, 0x0b,
            ]);
            output.extend([0x20, 0x05, 0x3a, 0x00, 0x00]);
        }
    }
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
    // domain pointer
    output.push(0x41);
    write_i64(output, i64::from(base));
    output.push(0x20);
    write_u32(output, status);
    output.extend([
        0x41, 0x18, 0x76, 0x41, 0x01, 0x46, 0x04, I32, 0x41, 0x00, 0x05, 0x41,
    ]);
    write_i64(output, 32);
    output.extend([0x0b, 0x36, 0x02, 0x00]);
    // domain length
    output.push(0x41);
    write_i64(output, i64::from(base + 4));
    output.push(0x20);
    write_u32(output, status);
    output.extend([0x41, 0x18, 0x76, 0x41, 0x01, 0x46, 0x04, I32, 0x41]);
    write_i64(output, CONTRACT_DOMAIN.len() as i64);
    output.extend([0x05, 0x41]);
    write_i64(output, ARITHMETIC_DOMAIN.len() as i64);
    output.extend([0x0b, 0x36, 0x02, 0x00]);
    // code and class
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
    // retryable = some(false)
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
        .unwrap_or_else(|| profile_error("scalar-algebra v5 HIR resolution failed"))
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-WIT108", message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::*;

    #[test]
    fn emits_deterministic_import_free_scalar_algebra_core() {
        let program =
            crate::parse(SOURCE_V5, Path::new("component-scalar-algebra-v5.spx")).unwrap();
        let first = emit_private_scalar_algebra_core_v5(&program).unwrap();
        let second = emit_private_scalar_algebra_core_v5(&program).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.layout_digests.len(), 6);
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&first.bytes)
            .expect("pinned upstream validator rejected scalar-algebra v5 core");
    }

    #[test]
    fn admission_rejects_effects_user_types_and_signature_drift() {
        for hostile in [
            SOURCE_V5.replace("fn main()", "fn main(extra: i64)"),
            SOURCE_V5.replace("Result<bool, bool>", "Result<bool, i64>"),
            SOURCE_V5.replace(
                "module test.component_scalar_algebra_v5;",
                "module test.component_scalar_algebra_v5;\nrecord Extra { value: i64, }",
            ),
        ] {
            let program = crate::parse(&hostile, Path::new("hostile-v5.spx")).unwrap();
            assert!(emit_private_scalar_algebra_core_v5(&program).is_err());
        }
    }

    #[test]
    fn admission_rejects_each_frozen_semantic_mutation() {
        for hostile in [
            SOURCE_V5.replacen("value != -99", "value != -98", 1),
            SOURCE_V5.replacen("divisor != 13", "divisor != 12", 1),
            SOURCE_V5.replacen("if select", "if !select", 1),
            SOURCE_V5.replacen("value + 1", "value + 2", 1),
            SOURCE_V5.replacen("/ divisor", "/ (divisor + 1)", 1),
            SOURCE_V5.replacen("value > 0", "value < 0", 1),
        ] {
            let program = crate::parse(&hostile, Path::new("semantic-hostile-v5.spx")).unwrap();
            let error = emit_private_scalar_algebra_core_v5(&program).unwrap_err();
            assert_eq!(error.code, "SPX-WIT108");
        }
    }

    #[test]
    fn raw_core_rejects_noncanonical_boolean_before_source_execution() {
        let program =
            crate::parse(SOURCE_V5, Path::new("component-scalar-algebra-v5.spx")).unwrap();
        let artifact = emit_private_scalar_algebra_core_v5(&program).unwrap();
        let stem = format!("semaprax-scalar-algebra-v5-{}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, artifact.bytes).unwrap();
        let names = CANONICAL_EXPORTS
            .into_iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(",");
        std::fs::write(
            &script_path,
            format!(
                "import fs from 'node:fs';\nconst {{instance}}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]));\nfor(const name of [{names}]){{let trapped=false;try{{instance.exports[name](1n,2,1n);}}catch(_error){{trapped=true;}}if(!trapped)throw new Error(`noncanonical bool escaped ${{name}}`);}}\nconsole.log('scalar-algebra-v5-invalid-bool-ok');\n"
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
            "Node raw-core invalid-bool gate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "scalar-algebra-v5-invalid-bool-ok"
        );
    }
}
