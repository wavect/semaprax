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
#[path = "record_pattern_v8/tests.rs"]
mod tests;
