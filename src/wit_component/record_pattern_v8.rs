//! Exact private monomorphic record-pattern Component Model v8 composition.

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

const INTERFACE_EXPORT: &str = "semaprax:private/record-pattern-projections@0.6.0";
const FUNCTION_EXPORTS: [&str; 4] = [
    "preserve-phantom-i64",
    "invert-phantom-i64",
    "preserve-phantom-bool",
    "invert-phantom-bool",
];
const TYPE_EXPORTS: [&str; 3] = ["status", "phantom-i64", "phantom-bool"];
const WIT_V8: &str = "package semaprax:private@0.6.0;\n\ninterface record-pattern-projections {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  record phantom-i64 { marker: bool }\n  record phantom-bool { marker: bool }\n  preserve-phantom-i64: func(input: phantom-i64, control: s64) -> result<bool, status>;\n  invert-phantom-i64: func(input: phantom-i64, control: s64) -> result<bool, status>;\n  preserve-phantom-bool: func(input: phantom-bool, control: s64) -> result<bool, status>;\n  invert-phantom-bool: func(input: phantom-bool, control: s64) -> result<bool, status>;\n}\n\nworld semaprax-private-v8 {\n  export record-pattern-projections;\n}\n";

const PROFILE: &[u8] = b"semaprax.private-record-pattern-component.v8\0canonical-abi-memory32-utf8\0four-monomorphic-graph-v13-patterns\0two-exact-phantom-instances\0shared-function-types\0status-before-output\0tag-last\0";
const PROFILE_DOMAIN: &[u8] = b"semaprax.private-record-pattern-component-profile.v8\0";
const ARTIFACT_DOMAIN: &[u8] = b"semaprax.private-record-pattern-component-artifact.v8\0";
const PLAN_DOMAIN: &[u8] = b"semaprax.component-record-pattern-plan.v8\0";

const SOURCE_REVISION_KAT: &str =
    "sha256:2baac0c0920dbb153789767bf506a4a81713081586a81444d8e5f5a8f5a8516d";
const GENERATED_CORE_KAT: [u8; 32] = [
    0xb6, 0xe1, 0xdb, 0xf9, 0x52, 0x2d, 0xbb, 0x98, 0xdf, 0x9b, 0x6f, 0xcd, 0x37, 0x0b, 0x56, 0x2a,
    0x9a, 0x72, 0x2f, 0xcc, 0x67, 0x2d, 0x44, 0x48, 0x8a, 0xed, 0x80, 0xf1, 0x3b, 0x7a, 0xd3, 0x9e,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateRecordPatternComponentArtifactV8 {
    bytes: Vec<u8>,
    digest: [u8; 32],
    generated_core_digest: [u8; 32],
    profile_digest: [u8; 32],
    graph_digest: [u8; 32],
    plan_digest: [u8; 32],
    layout_digests: [[u8; 32]; 2],
    source_revision: String,
}

impl PrivateRecordPatternComponentArtifactV8 {
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
    pub const fn layout_digests(&self) -> [[u8; 32]; 2] {
        self.layout_digests
    }
    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }
    #[must_use]
    pub const fn wit(&self) -> &'static str {
        WIT_V8
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPrivateRecordPatternComponentV8<'a> {
    core: &'a [u8],
    source_revision: &'a str,
}

impl<'a> ValidatedPrivateRecordPatternComponentV8<'a> {
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
    pub const fn type_export_names(self) -> [&'static str; 3] {
        TYPE_EXPORTS
    }
}

pub fn emit_private_record_pattern_component_v8(
    program: &Program,
) -> Result<PrivateRecordPatternComponentArtifactV8, Diagnostic> {
    let evidence = profile_evidence(program)?;
    let core = wasm::emit_private_record_pattern_core_v8(program)?;
    if core.source_revision != evidence.source_revision
        || core.graph_digest != evidence.graph_digest
        || core.plan_digest != evidence.plan_digest
        || core.layout_digests != evidence.layout_digests
    {
        return Err(profile_error(
            "record-pattern core disagrees with independent profile evidence",
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
    Ok(PrivateRecordPatternComponentArtifactV8 {
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
    for name in wasm::RECORD_PATTERN_COMPONENT_CANONICAL_EXPORTS_V8 {
        aliases.extend([0x00, 0x00, 0x01, 0x00]);
        push_name(&mut aliases, name);
    }
    aliases.extend([0x00, 0x02, 0x01, 0x00]);
    push_name(&mut aliases, "memory");
    push_counted_section(&mut bytes, 6, 5, &aliases);
    push_section(&mut bytes, 7, &component_types());
    let mut canonical = Vec::new();
    for (index, ty) in [5_u8, 5, 6, 6].into_iter().enumerate() {
        canonical.extend([0x00, 0x00, index as u8, 0x02, 0x00, 0x03, 0x00, ty]);
    }
    push_counted_section(&mut bytes, 8, 4, &canonical);
    let mut interface = vec![0x01, 0x07];
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
    let mut types = vec![0x07, 0x6b, 0x7f, 0x72, 0x04];
    push_name(&mut types, "domain");
    types.push(0x73);
    push_name(&mut types, "code");
    types.push(0x79);
    push_name(&mut types, "class");
    types.push(0x7d);
    push_name(&mut types, "retryable");
    types.push(0x00);
    for _ in 0..2 {
        types.extend([0x72, 0x01]);
        push_name(&mut types, "marker");
        types.push(0x7f);
    }
    types.extend([0x6a, 0x01, 0x7f, 0x01, 0x01]);
    for carrier in [2_u8, 3] {
        types.extend([0x40, 0x02]);
        push_name(&mut types, "input");
        types.push(carrier);
        push_name(&mut types, "control");
        types.extend([0x78, 0x00, 0x04]);
    }
    types
}

pub fn validate_private_record_pattern_component_v8<'a>(
    candidate: &'a [u8],
    expected_source_revision: &str,
    expected_core_digest: [u8; 32],
) -> Result<ValidatedPrivateRecordPatternComponentV8<'a>, PrivateComponentValidationError> {
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
    for name in wasm::RECORD_PATTERN_COMPONENT_CANONICAL_EXPORTS_V8 {
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
    for (index, ty) in [5_u8, 5, 6, 6].into_iter().enumerate() {
        expected_canonical.extend([0x00, 0x00, index as u8, 0x02, 0x00, 0x03, 0x00, ty]);
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
    interface.expect_u32(7, PrivateComponentValidationError::Profile)?;
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
    Ok(ValidatedPrivateRecordPatternComponentV8 {
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
    for (index, name) in wasm::RECORD_PATTERN_COMPONENT_CANONICAL_EXPORTS_V8
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
        "semaprax.component-record-pattern-v8",
        PrivateComponentValidationError::CoreModule,
    )?;
    let len =
        usize::try_from(custom.u32()?).map_err(|_| PrivateComponentValidationError::Encoding)?;
    let revision = std::str::from_utf8(custom.take(len)?)
        .map_err(|_| PrivateComponentValidationError::CoreModule)?;
    if revision != expected_revision {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    for _ in 0..4 {
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
    layout_digests: [[u8; 32]; 2],
}

fn profile_evidence(program: &Program) -> Result<ProfileEvidence, Diagnostic> {
    let expected = crate::parse(
        wasm::RECORD_PATTERN_COMPONENT_SOURCE_V8,
        std::path::Path::new("record-pattern-v8-component-profile.spx"),
    )?;
    if crate::format::canonical(program) != crate::format::canonical(&expected) {
        return Err(profile_error(
            "record-pattern component requires exact monomorphic source",
        ));
    }
    let resolved = hir::resolve(program).map_err(first_error)?;
    hir::validate(&resolved)?;
    let instances = [
        nominal(vec![ResolvedType::I64]),
        nominal(vec![ResolvedType::Bool]),
    ];
    let mut layout_digests = [[0_u8; 32]; 2];
    for (index, instance) in instances.iter().enumerate() {
        let layout = AggregateLayout::for_type(&resolved, AggregateTarget::Wasm32, instance)?;
        layout.validate(&resolved)?;
        if layout.size != 4
            || layout.align != 4
            || layout.fields.len() != 1
            || layout.fields[0].field != DeclarationId::new("component.pattern.phantom.marker")
            || layout.fields[0].ty != ResolvedType::Bool
            || layout.fields[0].offset != 0
        {
            return Err(profile_error("record-pattern component layout changed"));
        }
        layout_digests[index] = layout.digest();
    }
    if layout_digests[0] == layout_digests[1] {
        return Err(profile_error("record-pattern Phantom digests collided"));
    }
    let graph_json = crate::graph::to_json(program).map_err(first_error)?;
    if !graph_json.starts_with("{\"schema\":\"semaprax.graph.v13\",") {
        return Err(profile_error("record-pattern component requires Graph v13"));
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

fn nominal(arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new("component.pattern.phantom"),
        arguments,
    }
}

fn plan_digest(instances: &[ResolvedType; 2], layouts: &[[u8; 32]; 2]) -> [u8; 32] {
    let functions = [
        "component.pattern.preserve-phantom-i64",
        "component.pattern.invert-phantom-i64",
        "component.pattern.preserve-phantom-bool",
        "component.pattern.invert-phantom-bool",
    ];
    let inputs = [128_i32, 192, 256, 320];
    let internals = [144_i32, 208, 272, 336];
    let results = [160_i32, 224, 288, 352];
    let mut hash = Sha256::new();
    hash.update(PLAN_DOMAIN);
    for index in 0..4 {
        let layout = usize::from(index >= 2);
        for field in [
            functions[index].as_bytes(),
            wasm::RECORD_PATTERN_COMPONENT_CANONICAL_EXPORTS_V8[index].as_bytes(),
            instances[layout].identity_key().as_bytes(),
        ] {
            hash.update((field.len() as u64).to_le_bytes());
            hash.update(field);
        }
        hash.update(inputs[index].to_le_bytes());
        hash.update(internals[index].to_le_bytes());
        hash.update(results[index].to_le_bytes());
        hash.update([u8::from(index % 2 == 1)]);
        hash.update([if layout == 0 { 5 } else { 6 }]);
        hash.update(layouts[layout]);
    }
    hash.finalize().into()
}

fn profile_digest(evidence: &ProfileEvidence) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PROFILE_DOMAIN);
    for field in [WIT_V8.as_bytes(), PROFILE] {
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
        .unwrap_or_else(|| profile_error("record-pattern component resolution failed"))
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-WIT109", message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn artifact() -> PrivateRecordPatternComponentArtifactV8 {
        let program = crate::parse(
            wasm::RECORD_PATTERN_COMPONENT_SOURCE_V8,
            Path::new("component-record-pattern-v8.spx"),
        )
        .unwrap();
        emit_private_record_pattern_component_v8(&program).unwrap()
    }

    #[test]
    fn deterministic_v8_component_is_upstream_valid() {
        let first = artifact();
        assert_eq!(first.source_revision(), SOURCE_REVISION_KAT);
        assert_eq!(first.generated_core_digest(), GENERATED_CORE_KAT);
        assert_eq!(
            first.profile_digest(),
            [
                0x79, 0xd4, 0xba, 0xde, 0x38, 0xdd, 0x3f, 0xff, 0x9c, 0x71, 0x45, 0xb4, 0x06, 0xbb,
                0x0b, 0xb2, 0x65, 0xff, 0x3e, 0xf7, 0xcf, 0x08, 0x4e, 0xda, 0xc8, 0x33, 0x84, 0xc8,
                0x46, 0x10, 0xbc, 0xe2,
            ]
        );
        assert_eq!(
            first.graph_digest(),
            [
                0xc5, 0x87, 0x41, 0x58, 0x19, 0x39, 0x5e, 0x3d, 0x61, 0x8b, 0x1e, 0x72, 0x4d, 0x63,
                0x9d, 0x65, 0x0e, 0x7c, 0x55, 0xb0, 0x46, 0xf4, 0xb7, 0x7b, 0x8b, 0xcb, 0x5d, 0xe4,
                0xff, 0x95, 0x68, 0x2b,
            ]
        );
        assert_eq!(
            first.plan_digest(),
            [
                0xc7, 0x7c, 0x40, 0x60, 0xfb, 0x0b, 0x00, 0x51, 0xaf, 0x12, 0x5f, 0x4c, 0xa3, 0x53,
                0xdf, 0x3a, 0x6f, 0x5d, 0xbd, 0x36, 0x7c, 0xdc, 0x5f, 0xfd, 0x61, 0x34, 0x7a, 0x7c,
                0x22, 0x84, 0x70, 0x59,
            ]
        );
        assert_eq!(
            first.layout_digests(),
            [
                [
                    0xd2, 0xff, 0x60, 0x84, 0xbc, 0xfc, 0x95, 0x70, 0x1b, 0x1d, 0xd5, 0x98, 0x35,
                    0xd0, 0xac, 0x3a, 0xf9, 0x63, 0x62, 0xe0, 0x5e, 0x56, 0xdc, 0xad, 0xcb, 0xd4,
                    0xb8, 0xe5, 0xdc, 0x7d, 0x9d, 0x80,
                ],
                [
                    0x3e, 0x09, 0xce, 0xfc, 0x7d, 0x1a, 0xe9, 0xbc, 0x52, 0xec, 0x82, 0x7d, 0xeb,
                    0xdb, 0xcd, 0x07, 0x53, 0xd6, 0x3b, 0xcc, 0xa9, 0x94, 0xef, 0x77, 0x6e, 0xad,
                    0xb6, 0x6b, 0xa2, 0x54, 0xe6, 0x7a,
                ],
            ]
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(first.bytes())),
            [
                0xd8, 0x85, 0x90, 0x75, 0x2e, 0xd7, 0xb0, 0x8b, 0x0f, 0x0a, 0x32, 0x01, 0x9b, 0xa8,
                0xb4, 0xc5, 0xfc, 0x48, 0x9d, 0x59, 0xf0, 0x6b, 0x96, 0x98, 0x6d, 0x7a, 0xd6, 0x9e,
                0x25, 0x54, 0xa1, 0x0e,
            ]
        );
        assert_eq!(
            first.digest(),
            [
                0xe3, 0x2f, 0xe0, 0xa1, 0x5a, 0x34, 0x58, 0xf1, 0x6a, 0xa4, 0xda, 0x59, 0xd8, 0x76,
                0x83, 0x01, 0x3d, 0xbe, 0xba, 0x03, 0x75, 0x49, 0x66, 0xf3, 0x5e, 0x0c, 0xb6, 0x36,
                0x00, 0xe6, 0x13, 0xa3,
            ]
        );
        assert_eq!(first, artifact());
        assert_eq!(first.wit(), WIT_V8);
        assert_ne!(first.layout_digests()[0], first.layout_digests()[1]);
        let validated = validate_private_record_pattern_component_v8(
            first.bytes(),
            first.source_revision(),
            first.generated_core_digest(),
        )
        .unwrap();
        assert_eq!(validated.interface_export_name(), INTERFACE_EXPORT);
        assert_eq!(validated.function_export_names(), FUNCTION_EXPORTS);
        assert_eq!(validated.type_export_names(), TYPE_EXPORTS);
        assert_eq!(validated.source_revision(), SOURCE_REVISION_KAT);
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(validated.generated_core())),
            GENERATED_CORE_KAT
        );
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(first.bytes())
            .expect("upstream validator rejected record-pattern component v8");
    }

    #[test]
    fn every_byte_truncation_trailing_and_noncanonical_length_reject() {
        let artifact = artifact();
        for index in 0..artifact.bytes().len() {
            let mut hostile = artifact.bytes().to_vec();
            hostile[index] ^= 1;
            assert!(validate_private_record_pattern_component_v8(
                &hostile,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
        }
        for end in 0..artifact.bytes().len() {
            assert!(validate_private_record_pattern_component_v8(
                &artifact.bytes()[..end],
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
        }
        let mut trailing = artifact.bytes().to_vec();
        trailing.push(0);
        assert!(validate_private_record_pattern_component_v8(
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
        assert_eq!(positions.len(), 1, "v8 core-instance anchor drifted");
        noncanonical.splice(positions[0] + 1..positions[0] + 2, [0x84, 0x00]);
        assert_eq!(
            validate_private_record_pattern_component_v8(
                &noncanonical,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            ),
            Err(PrivateComponentValidationError::Encoding)
        );
    }

    #[test]
    fn all_equal_signature_identity_type_and_lift_swaps_reject() {
        let artifact = artifact();
        let rejects = |hostile: &[u8]| {
            assert!(validate_private_record_pattern_component_v8(
                hostile,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
        };

        let mut canonical_anchor = Vec::new();
        for (index, ty) in [5_u8, 5, 6, 6].into_iter().enumerate() {
            canonical_anchor.extend([0x00, 0x00, index as u8, 0x02, 0x00, 0x03, 0x00, ty]);
        }
        let canonical_at = artifact
            .bytes()
            .windows(canonical_anchor.len())
            .position(|window| window == canonical_anchor)
            .expect("v8 canonical lift anchor drifted");

        // Every pair has the same flattened core signature. All six valid
        // reindexings are rejected by identity, never admitted by layout.
        for left in 0..4 {
            for right in left + 1..4 {
                let mut hostile = artifact.bytes().to_vec();
                hostile.swap(canonical_at + 2 + left * 8, canonical_at + 2 + right * 8);
                wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
                    .validate_all(&hostile)
                    .expect("same-signature v8 core-index swap should be structurally valid");
                rejects(&hostile);
            }
        }

        // Crossing concrete Phantom function types changes named instance
        // identity despite an equal physical record layout.
        let mut hostile = artifact.bytes().to_vec();
        hostile.swap(canonical_at + 7, canonical_at + 23);
        rejects(&hostile);

        let mut interface_anchor = vec![0x01, 0x07];
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
            .expect("v8 interface anchor drifted");
        let function_ref = |name: &str| {
            interface_anchor
                .windows(name.len())
                .position(|window| window == name.as_bytes())
                .expect("v8 function interface anchor drifted")
                + name.len()
                + 1
        };
        let mut hostile = artifact.bytes().to_vec();
        hostile.swap(
            interface_at + function_ref(FUNCTION_EXPORTS[0]),
            interface_at + function_ref(FUNCTION_EXPORTS[3]),
        );
        rejects(&hostile);

        for needle in [
            b"phantom-i64".as_slice(),
            b"phantom-bool".as_slice(),
            b"marker".as_slice(),
        ] {
            let at = artifact
                .bytes()
                .windows(needle.len())
                .rposition(|window| window == needle)
                .expect("v8 named type anchor drifted");
            let mut hostile = artifact.bytes().to_vec();
            hostile[at] ^= 1;
            rejects(&hostile);
        }

        let program = crate::parse(
            wasm::RECORD_PATTERN_COMPONENT_SOURCE_V8,
            Path::new("rehashed-core-v8.spx"),
        )
        .unwrap();
        let mut hostile_core = wasm::emit_private_record_pattern_core_v8(&program)
            .unwrap()
            .bytes;
        let last = hostile_core.len() - 1;
        hostile_core[last] ^= 1;
        let rehashed: [u8; 32] = Sha256::digest(&hostile_core).into();
        let hostile_component = compose(&hostile_core);
        assert!(validate_private_record_pattern_component_v8(
            &hostile_component,
            artifact.source_revision(),
            rehashed,
        )
        .is_err());
    }

    #[test]
    fn v1_through_v8_profiles_are_never_confused() {
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
        let v7_program = crate::parse(
            wasm::GENERIC_RECORD_COMPONENT_SOURCE_V7,
            Path::new("v7.spx"),
        )
        .unwrap();
        let v7 = super::super::emit_private_generic_record_component_v7(&v7_program).unwrap();
        let v8 = artifact();
        for candidate in [
            v1.bytes(),
            v2.bytes(),
            v3.bytes(),
            v4.bytes(),
            v5.bytes(),
            v6.bytes(),
            v7.bytes(),
        ] {
            assert!(validate_private_record_pattern_component_v8(
                candidate,
                v8.source_revision(),
                v8.generated_core_digest(),
            )
            .is_err());
        }
        assert!(super::super::validate_private_component_v1(v8.bytes()).is_err());
        assert!(super::super::validate_private_checked_component_v2(
            v8.bytes(),
            v2.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_result_component_v3(
            v8.bytes(),
            v3.source_revision(),
            v3.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_source_result_component_v4(
            v8.bytes(),
            v4.source_revision(),
            v4.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_scalar_algebra_component_v5(
            v8.bytes(),
            v5.source_revision(),
            v5.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_nested_record_component_v6(
            v8.bytes(),
            v6.source_revision(),
            v6.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_generic_record_component_v7(
            v8.bytes(),
            v7.source_revision(),
            v7.generated_core_digest(),
        )
        .is_err());
    }
}
