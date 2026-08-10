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
mod tests {
    use std::path::Path;

    use super::*;

    fn artifact() -> PrivateGenericRecordComponentArtifactV7 {
        let program = crate::parse(
            wasm::GENERIC_RECORD_COMPONENT_SOURCE_V7,
            Path::new("component-generic-record-v7.spx"),
        )
        .unwrap();
        emit_private_generic_record_component_v7(&program).unwrap()
    }

    #[test]
    fn deterministic_v7_component_is_upstream_valid() {
        let first = artifact();
        assert_eq!(first.source_revision(), SOURCE_REVISION_KAT);
        assert_eq!(first.generated_core_digest(), GENERATED_CORE_KAT);
        assert_eq!(
            first.profile_digest(),
            [
                0x7b, 0x19, 0xf7, 0x4a, 0xb1, 0x85, 0xda, 0x90, 0x44, 0x5a, 0x04, 0x2d, 0xbd, 0x04,
                0xb6, 0xf3, 0x9f, 0x7f, 0x9e, 0xff, 0x3f, 0xff, 0xf3, 0x4f, 0xc5, 0xf0, 0xa3, 0xbd,
                0xfd, 0x4a, 0x9b, 0xbf,
            ]
        );
        assert_eq!(
            first.graph_digest(),
            [
                0xcc, 0x0e, 0xab, 0x96, 0x9a, 0x90, 0x77, 0x87, 0x8c, 0x78, 0x84, 0x68, 0xe4, 0xe7,
                0xdd, 0xfa, 0x90, 0xb1, 0xd0, 0x04, 0x63, 0x78, 0x5e, 0x0b, 0xe2, 0x95, 0xa9, 0xbc,
                0xaa, 0xef, 0xe4, 0x2e,
            ]
        );
        assert_eq!(
            first.plan_digest(),
            [
                0x40, 0x95, 0x4a, 0xca, 0x3c, 0x3a, 0xc6, 0x7e, 0x23, 0x09, 0x6f, 0x19, 0x97, 0x5f,
                0x76, 0xf4, 0x26, 0x97, 0x6e, 0xf8, 0xcd, 0x68, 0x93, 0xed, 0x45, 0x42, 0x3d, 0x7b,
                0xc2, 0x11, 0xaf, 0x02,
            ]
        );
        assert_eq!(
            first.layout_digests(),
            [
                [
                    0x35, 0x5b, 0x17, 0x18, 0xb6, 0x50, 0x5d, 0xa3, 0x5e, 0x2f, 0xdd, 0x0f, 0xb1,
                    0x61, 0x1f, 0xe4, 0x35, 0x2f, 0x25, 0xe7, 0x17, 0x76, 0xaa, 0xc8, 0x41, 0x6b,
                    0xcc, 0x47, 0x48, 0xbc, 0x62, 0xc0,
                ],
                [
                    0x23, 0x34, 0x89, 0x5b, 0xca, 0xd1, 0xa0, 0x78, 0x8f, 0xcf, 0xcd, 0x8b, 0xbf,
                    0xa8, 0xb6, 0x74, 0x37, 0xc8, 0x8a, 0x93, 0x7e, 0x21, 0xf0, 0x11, 0x74, 0xad,
                    0x40, 0x14, 0xcc, 0xcb, 0x65, 0x23,
                ],
                [
                    0x33, 0x4f, 0xa6, 0xbc, 0xb6, 0x4f, 0x4f, 0x55, 0x1a, 0x98, 0xf9, 0x46, 0x2a,
                    0x5f, 0xdb, 0xe2, 0xd5, 0x1a, 0x9f, 0xc8, 0x38, 0x99, 0x2e, 0xb0, 0x83, 0xe2,
                    0xf3, 0x22, 0xbc, 0x5f, 0xaa, 0xf6,
                ],
                [
                    0xe3, 0x9e, 0x1d, 0xfa, 0x20, 0x60, 0xed, 0xd4, 0xb8, 0xcf, 0xca, 0xc4, 0xbb,
                    0xc6, 0x7e, 0x4e, 0x71, 0x95, 0xce, 0x99, 0x0e, 0x6d, 0xe7, 0xbe, 0x78, 0x1e,
                    0xac, 0x09, 0x8f, 0xe2, 0x0a, 0xfe,
                ],
            ]
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(first.bytes())),
            [
                0x78, 0x0a, 0x0c, 0xcf, 0xc3, 0x5c, 0x7f, 0xf6, 0xd9, 0x33, 0x48, 0x37, 0x11, 0xe9,
                0x58, 0xd2, 0x9c, 0xfd, 0x44, 0xc2, 0x90, 0x76, 0x2b, 0x05, 0xcd, 0x51, 0x83, 0xe6,
                0xbf, 0x04, 0xb5, 0xb0,
            ]
        );
        assert_eq!(
            first.digest(),
            [
                0xc3, 0xd1, 0xfd, 0x10, 0x50, 0x1b, 0xfe, 0x8d, 0xcd, 0x4b, 0x5c, 0x8f, 0x24, 0x18,
                0x4d, 0x12, 0x7e, 0x46, 0x2b, 0x9c, 0xa4, 0xbc, 0x6b, 0x1f, 0x94, 0x22, 0xad, 0x8f,
                0xbc, 0xc0, 0xb2, 0x6e,
            ]
        );
        assert_eq!(first, artifact());
        assert_eq!(first.wit(), WIT_V7);
        assert_ne!(first.layout_digests()[2], first.layout_digests()[3]);
        let validated = validate_private_generic_record_component_v7(
            first.bytes(),
            first.source_revision(),
            first.generated_core_digest(),
        )
        .unwrap();
        assert_eq!(validated.interface_export_name(), INTERFACE_EXPORT);
        assert_eq!(validated.function_export_names(), FUNCTION_EXPORTS);
        assert_eq!(validated.type_export_names(), TYPE_EXPORTS);
        assert_eq!(validated.source_revision(), first.source_revision());
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(validated.generated_core())),
            GENERATED_CORE_KAT
        );
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(first.bytes())
            .expect("upstream validator rejected generic-record component v7");
    }

    #[test]
    fn every_byte_truncation_trailing_and_noncanonical_length_reject() {
        let artifact = artifact();
        for index in 0..artifact.bytes().len() {
            let mut hostile = artifact.bytes().to_vec();
            hostile[index] ^= 1;
            assert!(validate_private_generic_record_component_v7(
                &hostile,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
        }
        for end in 0..artifact.bytes().len() {
            assert!(validate_private_generic_record_component_v7(
                &artifact.bytes()[..end],
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
        }
        let mut trailing = artifact.bytes().to_vec();
        trailing.push(0);
        assert!(validate_private_generic_record_component_v7(
            &trailing,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
        let mut noncanonical = artifact.bytes().to_vec();
        let anchor = [0x02, 0x04, 0x01, 0x00, 0x00, 0x00];
        let positions = noncanonical
            .windows(anchor.len())
            .enumerate()
            .filter_map(|(index, window)| (window == anchor).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 1, "v7 core-instance anchor drifted");
        noncanonical.splice(positions[0] + 1..positions[0] + 2, [0x84, 0x00]);
        assert_eq!(
            validate_private_generic_record_component_v7(
                &noncanonical,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            ),
            Err(PrivateComponentValidationError::Encoding)
        );
    }

    #[test]
    fn exact_field_type_lift_and_instance_mappings_reject() {
        let artifact = artifact();
        let rejects = |hostile: &[u8]| {
            assert!(validate_private_generic_record_component_v7(
                hostile,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
        };
        for needle in [
            b"duo-i64-bool".as_slice(),
            b"duo-bool-i64".as_slice(),
            b"phantom-i64".as_slice(),
            b"phantom-bool".as_slice(),
            b"left".as_slice(),
            b"right".as_slice(),
        ] {
            let offset = artifact
                .bytes()
                .windows(needle.len())
                .rposition(|window| window == needle)
                .expect("v7 semantic anchor drifted");
            let mut hostile = artifact.bytes().to_vec();
            hostile[offset] ^= 1;
            rejects(&hostile);
        }

        let duo_i64_bool = {
            let mut bytes = vec![0x72, 0x02];
            push_name(&mut bytes, "left");
            bytes.push(0x78);
            push_name(&mut bytes, "right");
            bytes.push(0x7f);
            bytes
        };
        let duo_at = artifact
            .bytes()
            .windows(duo_i64_bool.len())
            .position(|window| window == duo_i64_bool)
            .expect("v7 Duo<i64,bool> type anchor drifted");
        let mut hostile = artifact.bytes().to_vec();
        let mut swapped = vec![0x72, 0x02];
        push_name(&mut swapped, "right");
        swapped.push(0x7f);
        push_name(&mut swapped, "left");
        swapped.push(0x78);
        hostile.splice(duo_at..duo_at + duo_i64_bool.len(), swapped);
        rejects(&hostile);

        let mut canonical_anchor = Vec::new();
        for index in 0_u8..4 {
            canonical_anchor.extend([0x00, 0x00, index, 0x02, 0x00, 0x03, 0x00, 10 + index]);
        }
        let canonical_at = artifact
            .bytes()
            .windows(canonical_anchor.len())
            .position(|window| window == canonical_anchor)
            .expect("v7 canonical lift anchor drifted");

        // The two Phantom core functions have the same physical signature. A valid
        // Component can cross their core indices, but the exact v7 map must reject it.
        let mut hostile = artifact.bytes().to_vec();
        hostile.swap(canonical_at + 18, canonical_at + 26);
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&hostile)
            .expect("same-signature Phantom core-index swap should remain structurally valid");
        rejects(&hostile);

        // Crossing the distinct named Phantom result types is also exact-profile hostile.
        let mut hostile = artifact.bytes().to_vec();
        hostile.swap(canonical_at + 23, canonical_at + 31);
        rejects(&hostile);

        let mut interface_anchor = vec![0x01, 0x09];
        for (index, name) in TYPE_EXPORTS.into_iter().enumerate() {
            interface_anchor.push(0x00);
            push_name(&mut interface_anchor, name);
            interface_anchor.extend([0x03, 0x01 + index as u8]);
        }
        for (index, name) in FUNCTION_EXPORTS.into_iter().enumerate() {
            interface_anchor.push(0x00);
            push_name(&mut interface_anchor, name);
            interface_anchor.extend([0x01, index as u8]);
        }
        let interface_at = artifact
            .bytes()
            .windows(interface_anchor.len())
            .position(|window| window == interface_anchor)
            .expect("v7 interface anchor drifted");
        let mut hostile = artifact.bytes().to_vec();
        let preserve_ref = interface_anchor
            .windows(b"preserve-phantom-i64".len())
            .position(|window| window == b"preserve-phantom-i64")
            .unwrap()
            + b"preserve-phantom-i64".len()
            + 1;
        let invert_ref = interface_anchor
            .windows(b"invert-phantom-bool".len())
            .position(|window| window == b"invert-phantom-bool")
            .unwrap()
            + b"invert-phantom-bool".len()
            + 1;
        hostile.swap(interface_at + preserve_ref, interface_at + invert_ref);
        rejects(&hostile);

        let program = crate::parse(
            wasm::GENERIC_RECORD_COMPONENT_SOURCE_V7,
            Path::new("rehashed-core-v7.spx"),
        )
        .unwrap();
        let mut hostile_core = wasm::emit_private_generic_record_core_v7(&program)
            .unwrap()
            .bytes;
        let last = hostile_core.len() - 1;
        hostile_core[last] ^= 1;
        let rehashed: [u8; 32] = Sha256::digest(&hostile_core).into();
        let hostile_component = compose(&hostile_core);
        assert!(validate_private_generic_record_component_v7(
            &hostile_component,
            artifact.source_revision(),
            rehashed,
        )
        .is_err());
    }

    #[test]
    fn v1_through_v7_profiles_are_never_confused() {
        const V4_SOURCE: &str = r#"module v4;
@id("component.source")
fn source(value:i64,reject:bool)->Result<i64,bool> { if reject { Result<i64,bool>::Err { error: value > 0 } } else { Result<i64,bool>::Ok { value: value } } }
@id("component.evaluate")
fn evaluate(value:i64,reject:bool,divisor:i64)->Result<bool,bool>
requires value != -99
ensures divisor != 13
{ let checked = source(value,reject)?; Result<bool,bool>::Ok { value: (checked + 1) / divisor > 0 } }
@id("app.main") fn main()->i64 { 0 }
"#;
        let v1 = super::super::emit_private_component_v1();
        let v2_program = crate::parse(
            "module v2; @id(\"app.main\") fn main() -> i64 { 42 }",
            Path::new("v2.spx"),
        )
        .unwrap();
        let v2 = super::super::emit_private_checked_component_v2(&v2_program).unwrap();
        let v3_program = crate::parse(
            "module v3; @id(\"component.evaluate\") fn evaluate(left:i64,right:i64)->i64 { left + right } @id(\"app.main\") fn main()->i64 { 0 }",
            Path::new("v3.spx"),
        )
        .unwrap();
        let v3 = super::super::emit_private_result_component_v3(&v3_program).unwrap();
        let v4_program = crate::parse(V4_SOURCE, Path::new("v4.spx")).unwrap();
        let v4 = super::super::emit_private_source_result_component_v4(&v4_program).unwrap();
        let v5_program = crate::parse(
            wasm::SCALAR_ALGEBRA_COMPONENT_SOURCE_V5,
            Path::new("v5.spx"),
        )
        .unwrap();
        let v5 = super::super::emit_private_scalar_algebra_component_v5(&v5_program).unwrap();
        let v6_program =
            crate::parse(wasm::NESTED_RECORD_COMPONENT_SOURCE_V6, Path::new("v6.spx")).unwrap();
        let v6 = super::super::emit_private_nested_record_component_v6(&v6_program).unwrap();
        let v7 = artifact();
        for candidate in [
            v1.bytes(),
            v2.bytes(),
            v3.bytes(),
            v4.bytes(),
            v5.bytes(),
            v6.bytes(),
        ] {
            assert!(validate_private_generic_record_component_v7(
                candidate,
                v7.source_revision(),
                v7.generated_core_digest(),
            )
            .is_err());
        }
        assert!(super::super::validate_private_component_v1(v7.bytes()).is_err());
        assert!(super::super::validate_private_checked_component_v2(
            v7.bytes(),
            v2.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_result_component_v3(
            v7.bytes(),
            v3.source_revision(),
            v3.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_source_result_component_v4(
            v7.bytes(),
            v4.source_revision(),
            v4.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_scalar_algebra_component_v5(
            v7.bytes(),
            v5.source_revision(),
            v5.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_nested_record_component_v6(
            v7.bytes(),
            v6.source_revision(),
            v6.generated_core_digest(),
        )
        .is_err());
    }
}
