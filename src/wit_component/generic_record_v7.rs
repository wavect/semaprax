//! Exact private concrete generic-record Component Model v7 composition.

use sha2::{Digest, Sha256};

use crate::aggregate_layout::{AggregateLayout, AggregateTarget};
use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, DeclarationId, ResolvedType};
use crate::wasm;

use super::{
    push_counted_section, push_name, push_section, Cursor, PrivateComponentValidationError,
    COMPONENT_HEADER,
};

const INTERFACE_EXPORT: &str = "semaprax:private/generic-records@0.5.0";
const FUNCTION_EXPORTS: [&str; 4] = [
    "transform-i64-bool",
    "transform-bool-i64",
    "preserve-phantom-i64",
    "invert-phantom-bool",
];
const TYPE_EXPORTS: [&str; 5] = [
    "status",
    "duo-i64-bool",
    "duo-bool-i64",
    "phantom-i64",
    "phantom-bool",
];

const WIT_V7: &str = "package semaprax:private@0.5.0;\n\ninterface generic-records {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  record duo-i64-bool { left: s64, right: bool }\n  record duo-bool-i64 { left: bool, right: s64 }\n  record phantom-i64 { marker: bool }\n  record phantom-bool { marker: bool }\n  transform-i64-bool: func(input: duo-i64-bool, delta: s64, divisor: s64) -> result<duo-i64-bool, status>;\n  transform-bool-i64: func(input: duo-bool-i64, delta: s64, divisor: s64) -> result<duo-bool-i64, status>;\n  preserve-phantom-i64: func(input: phantom-i64) -> result<phantom-i64, status>;\n  invert-phantom-bool: func(input: phantom-bool) -> result<phantom-bool, status>;\n}\n\nworld semaprax-private-v7 {\n  export generic-records;\n}\n";

const PROFILE: &[u8] = b"semaprax.private-generic-record-component.v7\0canonical-abi-memory32-utf8\0four-ordered-concrete-instances\0duo-ordered-arguments\0phantom-identical-layout-distinct-instance\0fieldwise-status-first-tag-last\0graph-v12\0";
const PROFILE_DOMAIN: &[u8] = b"semaprax.private-generic-record-component-profile.v7\0";
const ARTIFACT_DOMAIN: &[u8] = b"semaprax.private-generic-record-component-artifact.v7\0";
const PLAN_DOMAIN: &[u8] = b"semaprax.component-generic-record-plan.v7\0";

const SOURCE_REVISION_KAT: &str =
    "sha256:2c2c38ae4a6400730bc6c91de659675074020651b9b58bb6a39d047630ef7303";
const GENERATED_CORE_KAT: [u8; 32] = [
    0xd2, 0x18, 0xff, 0x1e, 0xaf, 0xf5, 0xf3, 0xf6, 0x77, 0xfe, 0xe5, 0x8c, 0x7b, 0x2f, 0xeb, 0x50,
    0x0e, 0x9e, 0xfe, 0xd8, 0x22, 0x58, 0x00, 0xcf, 0xc3, 0xa6, 0x56, 0x2f, 0x97, 0xd1, 0x17, 0xd8,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateGenericRecordComponentArtifactV7 {
    bytes: Vec<u8>,
    digest: [u8; 32],
    generated_core_digest: [u8; 32],
    profile_digest: [u8; 32],
    graph_digest: [u8; 32],
    plan_digest: [u8; 32],
    layout_digests: [[u8; 32]; 4],
    source_revision: String,
}

impl PrivateGenericRecordComponentArtifactV7 {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
    #[must_use]
    pub const fn generated_core_digest(&self) -> [u8; 32] {
        self.generated_core_digest
    }
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }
    #[must_use]
    pub const fn graph_digest(&self) -> [u8; 32] {
        self.graph_digest
    }
    #[must_use]
    pub const fn plan_digest(&self) -> [u8; 32] {
        self.plan_digest
    }
    #[must_use]
    pub const fn layout_digests(&self) -> [[u8; 32]; 4] {
        self.layout_digests
    }
    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }
    #[must_use]
    pub const fn wit(&self) -> &'static str {
        WIT_V7
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPrivateGenericRecordComponentV7<'a> {
    core: &'a [u8],
    source_revision: &'a str,
}

impl<'a> ValidatedPrivateGenericRecordComponentV7<'a> {
    #[must_use]
    pub const fn generated_core(self) -> &'a [u8] {
        self.core
    }
    #[must_use]
    pub const fn source_revision(self) -> &'a str {
        self.source_revision
    }
    #[must_use]
    pub const fn interface_export_name(self) -> &'static str {
        INTERFACE_EXPORT
    }
    #[must_use]
    pub const fn function_export_names(self) -> [&'static str; 4] {
        FUNCTION_EXPORTS
    }
    #[must_use]
    pub const fn type_export_names(self) -> [&'static str; 5] {
        TYPE_EXPORTS
    }
}

pub fn emit_private_generic_record_component_v7(
    program: &Program,
) -> Result<PrivateGenericRecordComponentArtifactV7, Diagnostic> {
    let evidence = profile_evidence(program)?;
    let core = wasm::emit_private_generic_record_core_v7(program)?;
    if core.source_revision != evidence.source_revision
        || core.graph_digest != evidence.graph_digest
        || core.plan_digest != evidence.plan_digest
        || core.layout_digests != evidence.layout_digests
    {
        return Err(profile_error(
            "generic-record core disagrees with independent profile evidence",
        ));
    }
    let generated_core_digest = Sha256::digest(&core.bytes).into();
    let profile_digest = profile_digest(&evidence);
    let bytes = compose(&core.bytes);
    let digest = artifact_digest(
        &evidence.source_revision,
        &generated_core_digest,
        &profile_digest,
        &bytes,
    );
    Ok(PrivateGenericRecordComponentArtifactV7 {
        bytes,
        digest,
        generated_core_digest,
        profile_digest,
        graph_digest: evidence.graph_digest,
        plan_digest: evidence.plan_digest,
        layout_digests: evidence.layout_digests,
        source_revision: evidence.source_revision,
    })
}

fn compose(core: &[u8]) -> Vec<u8> {
    let mut bytes = COMPONENT_HEADER.to_vec();
    push_section(&mut bytes, 1, core);
    push_counted_section(&mut bytes, 2, 1, &[0x00, 0x00, 0x00]);
    let mut aliases = Vec::new();
    for name in wasm::GENERIC_RECORD_COMPONENT_CANONICAL_EXPORTS_V7 {
        aliases.extend([0x00, 0x00, 0x01, 0x00]);
        push_name(&mut aliases, name);
    }
    aliases.extend([0x00, 0x02, 0x01, 0x00]);
    push_name(&mut aliases, "memory");
    push_counted_section(&mut bytes, 6, 5, &aliases);
    push_section(&mut bytes, 7, &component_types());
    let mut canonical = Vec::new();
    for index in 0_u8..4 {
        canonical.extend([0x00, 0x00, index, 0x02, 0x00, 0x03, 0x00, 10 + index]);
    }
    push_counted_section(&mut bytes, 8, 4, &canonical);
    let mut interface = vec![0x01, 0x09];
    for (index, name) in TYPE_EXPORTS.into_iter().enumerate() {
        interface.push(0x00);
        push_name(&mut interface, name);
        interface.extend([0x03, 0x01 + index as u8]);
    }
    for (index, name) in FUNCTION_EXPORTS.into_iter().enumerate() {
        interface.push(0x00);
        push_name(&mut interface, name);
        interface.extend([0x01, index as u8]);
    }
    push_counted_section(&mut bytes, 5, 1, &interface);
    let mut export = vec![0x00];
    push_name(&mut export, INTERFACE_EXPORT);
    export.extend([0x05, 0x00, 0x00]);
    push_counted_section(&mut bytes, 11, 1, &export);
    bytes
}

fn component_types() -> Vec<u8> {
    let mut types = vec![0x0e];
    types.extend([0x6b, 0x7f, 0x72, 0x04]);
    push_name(&mut types, "domain");
    types.push(0x73);
    push_name(&mut types, "code");
    types.push(0x79);
    push_name(&mut types, "class");
    types.push(0x7d);
    push_name(&mut types, "retryable");
    types.push(0x00);
    for (left, right) in [(0x78, 0x7f), (0x7f, 0x78)] {
        types.extend([0x72, 0x02]);
        push_name(&mut types, "left");
        types.push(left);
        push_name(&mut types, "right");
        types.push(right);
    }
    for _ in 0..2 {
        types.extend([0x72, 0x01]);
        push_name(&mut types, "marker");
        types.push(0x7f);
    }
    for carrier in 2_u8..6 {
        types.extend([0x6a, 0x01, carrier, 0x01, 0x01]);
    }
    for (carrier, result) in [(2, 6), (3, 7)] {
        types.extend([0x40, 0x03]);
        push_name(&mut types, "input");
        types.push(carrier);
        push_name(&mut types, "delta");
        types.push(0x78);
        push_name(&mut types, "divisor");
        types.extend([0x78, 0x00, result]);
    }
    for (carrier, result) in [(4, 8), (5, 9)] {
        types.extend([0x40, 0x01]);
        push_name(&mut types, "input");
        types.extend([carrier, 0x00, result]);
    }
    types
}

pub fn validate_private_generic_record_component_v7<'a>(
    candidate: &'a [u8],
    expected_source_revision: &str,
    expected_core_digest: [u8; 32],
) -> Result<ValidatedPrivateGenericRecordComponentV7<'a>, PrivateComponentValidationError> {
    if expected_source_revision != SOURCE_REVISION_KAT || expected_core_digest != GENERATED_CORE_KAT
    {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let mut component = Cursor::new(candidate);
    if component.take(8)? != COMPONENT_HEADER {
        return Err(PrivateComponentValidationError::Header);
    }
    let core = component.section(1)?;
    if <[u8; 32]>::from(Sha256::digest(core)) != expected_core_digest {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let revision = validate_core(core, expected_source_revision)?;
    super::validate_exact_counted_section(
        component.section(2)?,
        &[0x00, 0x00, 0x00],
        PrivateComponentValidationError::Profile,
    )?;
    let mut aliases = Cursor::new(component.section(6)?);
    aliases.expect_u32(5, PrivateComponentValidationError::Profile)?;
    for name in wasm::GENERIC_RECORD_COMPONENT_CANONICAL_EXPORTS_V7 {
        aliases.expect_bytes(
            &[0x00, 0x00, 0x01, 0x00],
            PrivateComponentValidationError::Profile,
        )?;
        aliases.expect_name(name, PrivateComponentValidationError::Profile)?;
    }
    aliases.expect_bytes(
        &[0x00, 0x02, 0x01, 0x00],
        PrivateComponentValidationError::Profile,
    )?;
    aliases.expect_name("memory", PrivateComponentValidationError::Profile)?;
    aliases.finish(PrivateComponentValidationError::Profile)?;
    super::validate_exact_payload(
        component.section(7)?,
        &component_types(),
        PrivateComponentValidationError::Profile,
    )?;
    let mut expected_canonical = Vec::new();
    for index in 0_u8..4 {
        expected_canonical.extend([0x00, 0x00, index, 0x02, 0x00, 0x03, 0x00, 10 + index]);
    }
    let mut canonical = Cursor::new(component.section(8)?);
    canonical.expect_u32(4, PrivateComponentValidationError::Profile)?;
    canonical.expect_bytes(
        &expected_canonical,
        PrivateComponentValidationError::Profile,
    )?;
    canonical.finish(PrivateComponentValidationError::Profile)?;
    let mut interface = Cursor::new(component.section(5)?);
    interface.expect_u32(1, PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x01], PrivateComponentValidationError::Profile)?;
    interface.expect_u32(9, PrivateComponentValidationError::Profile)?;
    for (index, name) in TYPE_EXPORTS.into_iter().enumerate() {
        interface.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
        interface.expect_name(name, PrivateComponentValidationError::Profile)?;
        interface.expect_bytes(
            &[0x03, 0x01 + index as u8],
            PrivateComponentValidationError::Profile,
        )?;
    }
    for (index, name) in FUNCTION_EXPORTS.into_iter().enumerate() {
        interface.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
        interface.expect_name(name, PrivateComponentValidationError::Profile)?;
        interface.expect_bytes(
            &[0x01, index as u8],
            PrivateComponentValidationError::Profile,
        )?;
    }
    interface.finish(PrivateComponentValidationError::Profile)?;
    let mut export = Cursor::new(component.section(11)?);
    export.expect_u32(1, PrivateComponentValidationError::Profile)?;
    export.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
    export.expect_name(INTERFACE_EXPORT, PrivateComponentValidationError::Profile)?;
    export.expect_bytes(
        &[0x05, 0x00, 0x00],
        PrivateComponentValidationError::Profile,
    )?;
    export.finish(PrivateComponentValidationError::Profile)?;
    component.finish(PrivateComponentValidationError::Profile)?;
    Ok(ValidatedPrivateGenericRecordComponentV7 {
        core,
        source_revision: revision,
    })
}

fn validate_core<'a>(
    core: &'a [u8],
    expected_revision: &str,
) -> Result<&'a str, PrivateComponentValidationError> {
    let mut module = Cursor::new(core);
    if module.take(8)? != b"\0asm\x01\0\0\0" {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    if module.section(1)?.is_empty() || module.section(3)?.is_empty() {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    super::validate_exact_payload(
        module.section(5)?,
        &[0x01, 0x00, 0x01],
        PrivateComponentValidationError::CoreModule,
    )?;
    super::validate_exact_payload(
        module.section(6)?,
        &[0x01, 0x7f, 0x01, 0x41, 0x80, 0x80, 0x04, 0x0b],
        PrivateComponentValidationError::CoreModule,
    )?;
    let mut exports = Cursor::new(module.section(7)?);
    exports.expect_u32(5, PrivateComponentValidationError::CoreModule)?;
    exports.expect_name("memory", PrivateComponentValidationError::CoreModule)?;
    exports.expect_bytes(&[0x02, 0x00], PrivateComponentValidationError::CoreModule)?;
    for (index, name) in wasm::GENERIC_RECORD_COMPONENT_CANONICAL_EXPORTS_V7
        .into_iter()
        .enumerate()
    {
        exports.expect_name(name, PrivateComponentValidationError::CoreModule)?;
        exports.expect_bytes(
            &[0x00, 0x04 + index as u8],
            PrivateComponentValidationError::CoreModule,
        )?;
    }
    exports.finish(PrivateComponentValidationError::CoreModule)?;
    if module.section(10)?.is_empty() || module.section(11)?.is_empty() {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let mut custom = Cursor::new(module.section(0)?);
    custom.expect_name(
        "semaprax.component-generic-record-v7",
        PrivateComponentValidationError::CoreModule,
    )?;
    let len =
        usize::try_from(custom.u32()?).map_err(|_| PrivateComponentValidationError::Encoding)?;
    let revision = std::str::from_utf8(custom.take(len)?)
        .map_err(|_| PrivateComponentValidationError::CoreModule)?;
    if revision != expected_revision {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    for _ in 0..6 {
        custom.take(32)?;
    }
    custom.finish(PrivateComponentValidationError::CoreModule)?;
    module.finish(PrivateComponentValidationError::CoreModule)?;
    Ok(revision)
}

struct ProfileEvidence {
    source_revision: String,
    graph_digest: [u8; 32],
    plan_digest: [u8; 32],
    layout_digests: [[u8; 32]; 4],
}

fn profile_evidence(program: &Program) -> Result<ProfileEvidence, Diagnostic> {
    let expected = crate::parse(
        wasm::GENERIC_RECORD_COMPONENT_SOURCE_V7,
        std::path::Path::new("generic-record-v7-component-profile.spx"),
    )?;
    if crate::format::canonical(program) != crate::format::canonical(&expected) {
        return Err(profile_error(
            "generic-record component requires exact frozen source",
        ));
    }
    let resolved = hir::resolve(program).map_err(first_error)?;
    hir::validate(&resolved)?;
    let instances = [
        nominal("component.duo", vec![ResolvedType::I64, ResolvedType::Bool]),
        nominal("component.duo", vec![ResolvedType::Bool, ResolvedType::I64]),
        nominal("component.phantom", vec![ResolvedType::I64]),
        nominal("component.phantom", vec![ResolvedType::Bool]),
    ];
    let mut layout_digests = [[0_u8; 32]; 4];
    for (index, instance) in instances.iter().enumerate() {
        let layout = AggregateLayout::for_type(&resolved, AggregateTarget::Wasm32, instance)?;
        layout.validate(&resolved)?;
        layout_digests[index] = layout.digest();
    }
    if layout_digests[2] == layout_digests[3] {
        return Err(profile_error("Phantom exact-instance digests collided"));
    }
    let graph_json = crate::graph::to_json(program).map_err(first_error)?;
    if !graph_json.starts_with("{\"schema\":\"semaprax.graph.v12\",") {
        return Err(profile_error("generic-record component requires Graph v12"));
    }
    let graph_digest = Sha256::digest(graph_json.as_bytes()).into();
    let plan_digest = plan_digest(&instances, &layout_digests);
    Ok(ProfileEvidence {
        source_revision: crate::graph::revision(program),
        graph_digest,
        plan_digest,
        layout_digests,
    })
}

fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    }
}

fn plan_digest(instances: &[ResolvedType; 4], layouts: &[[u8; 32]; 4]) -> [u8; 32] {
    let function_ids = [
        "component.transform-i64-bool",
        "component.transform-bool-i64",
        "component.preserve-phantom-i64",
        "component.invert-phantom-bool",
    ];
    let inputs = [128_i32, 256, 384, 448];
    let internals = [160_i32, 288, 400, 464];
    let results = [192_i32, 320, 416, 480];
    let bool_parameters = [1_u32, 0, 0, 0];
    let bool_offsets = [8_i32, 0, 0, 0];
    let payload_offsets = [8_i32, 8, 4, 4];
    let mut hash = Sha256::new();
    hash.update(PLAN_DOMAIN);
    for index in 0..4 {
        for field in [
            function_ids[index].as_bytes(),
            wasm::GENERIC_RECORD_COMPONENT_CANONICAL_EXPORTS_V7[index].as_bytes(),
            instances[index].identity_key().as_bytes(),
        ] {
            hash.update((field.len() as u64).to_le_bytes());
            hash.update(field);
        }
        hash.update(inputs[index].to_le_bytes());
        hash.update(internals[index].to_le_bytes());
        hash.update(results[index].to_le_bytes());
        hash.update(bool_parameters[index].to_le_bytes());
        hash.update(bool_offsets[index].to_le_bytes());
        hash.update(payload_offsets[index].to_le_bytes());
        hash.update(layouts[index]);
    }
    hash.finalize().into()
}

fn profile_digest(evidence: &ProfileEvidence) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PROFILE_DOMAIN);
    for field in [WIT_V7.as_bytes(), PROFILE] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field);
    }
    hash.update(evidence.graph_digest);
    hash.update(evidence.plan_digest);
    for digest in evidence.layout_digests {
        hash.update(digest);
    }
    hash.finalize().into()
}

fn artifact_digest(revision: &str, core: &[u8; 32], profile: &[u8; 32], bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(ARTIFACT_DOMAIN);
    hash.update((revision.len() as u64).to_le_bytes());
    hash.update(revision.as_bytes());
    hash.update(core);
    hash.update(profile);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

fn first_error(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.severity.is_error())
        .unwrap_or_else(|| profile_error("generic-record component resolution failed"))
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-WIT108", message)
}

#[cfg(test)]
#[path = "generic_record_v7/tests.rs"]
mod tests;
