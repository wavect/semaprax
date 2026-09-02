//! Exact private nested direct-scalar record Component Model v6 composition.

use sha2::{Digest, Sha256};

use crate::aggregate_layout::{AggregateLayout, AggregateTarget};
use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, DeclarationId, IdentityOrigin, ResolvedType, ResolvedTypeDeclarationKind};
use crate::wasm;

use super::{
    push_counted_section, push_name, push_section, Cursor, PrivateComponentValidationError,
    COMPONENT_HEADER,
};

const INTERFACE_EXPORT: &str = "semaprax:private/nested-records@0.4.0";
const FUNCTION_EXPORT: &str = "transform";
const TYPE_EXPORTS: [&str; 3] = ["status", "inner", "outer"];

const WIT_V6: &str = "package semaprax:private@0.4.0;\n\ninterface nested-records {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  record inner { value: s64, flag: bool }\n  record outer { inner: inner, other: s64 }\n  transform: func(input: outer, delta: s64) -> result<outer, status>;\n}\n\nworld semaprax-private-v6 {\n  export nested-records;\n}\n";

const PROFILE: &[u8] = b"semaprax.private-nested-record-component.v6\0canonical-abi-memory32-utf8\0one-stable-id-export\0inner-i64-bool\0outer-inner-i64\0fieldwise-reconstruction\0status-first-tag-last\0no-layout-identity-inference\0";
const PROFILE_DIGEST_DOMAIN: &[u8] = b"semaprax.private-nested-record-component-profile.v6\0";
const COMPONENT_DIGEST_DOMAIN: &[u8] = b"semaprax.private-nested-record-component-artifact.v6\0";

// These independent roots are filled only from reviewed fixture output and are
// never derived by the validator from the candidate artifact.
const SOURCE_REVISION_KAT: &str =
    "sha256:d1fcbc45b3d86fa1d7910378578828df3c557dba92f90ed9459f928c5bf2fe8a";
const GENERATED_CORE_KAT: [u8; 32] = [
    0x42, 0x83, 0x5d, 0xcb, 0xf9, 0x80, 0x78, 0xac, 0x24, 0xbf, 0xd3, 0x65, 0x68, 0xf1, 0xb6, 0x91,
    0x7b, 0x5b, 0x64, 0xca, 0x2d, 0x82, 0x65, 0xef, 0x4d, 0xed, 0x16, 0x1d, 0x26, 0x43, 0x8d, 0xa1,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateNestedRecordComponentArtifactV6 {
    bytes: Vec<u8>,
    digest: [u8; 32],
    generated_core_digest: [u8; 32],
    profile_digest: [u8; 32],
    inner_layout_digest: [u8; 32],
    outer_layout_digest: [u8; 32],
    source_revision: String,
}

impl PrivateNestedRecordComponentArtifactV6 {
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
    pub const fn layout_digests(&self) -> [[u8; 32]; 2] {
        [self.inner_layout_digest, self.outer_layout_digest]
    }

    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    #[must_use]
    pub const fn wit(&self) -> &'static str {
        WIT_V6
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPrivateNestedRecordComponentV6<'a> {
    generated_core: &'a [u8],
    source_revision: &'a str,
}

impl<'a> ValidatedPrivateNestedRecordComponentV6<'a> {
    #[must_use]
    pub const fn generated_core(self) -> &'a [u8] {
        self.generated_core
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
    pub const fn function_export_name(self) -> &'static str {
        FUNCTION_EXPORT
    }

    #[must_use]
    pub const fn type_export_names(self) -> [&'static str; 3] {
        TYPE_EXPORTS
    }
}

pub fn emit_private_nested_record_component_v6(
    program: &Program,
) -> Result<PrivateNestedRecordComponentArtifactV6, Diagnostic> {
    let evidence = profile_evidence(program)?;
    let core = wasm::emit_private_nested_record_core_v6(program)?;
    if core.source_revision != evidence.source_revision
        || core.inner_layout_digest != evidence.inner_layout_digest
        || core.outer_layout_digest != evidence.outer_layout_digest
    {
        return Err(profile_error(
            "nested-record core disagrees with independently admitted source meaning",
        ));
    }
    let generated_core_digest: [u8; 32] = Sha256::digest(&core.bytes).into();
    let profile_digest = profile_digest(&evidence);
    let bytes = compose(&core.bytes);
    let digest = artifact_digest(
        &evidence.source_revision,
        &generated_core_digest,
        &profile_digest,
        &bytes,
    );
    Ok(PrivateNestedRecordComponentArtifactV6 {
        bytes,
        digest,
        generated_core_digest,
        profile_digest,
        inner_layout_digest: evidence.inner_layout_digest,
        outer_layout_digest: evidence.outer_layout_digest,
        source_revision: evidence.source_revision,
    })
}

fn compose(core: &[u8]) -> Vec<u8> {
    let mut bytes = COMPONENT_HEADER.to_vec();
    push_section(&mut bytes, 1, core);
    push_counted_section(&mut bytes, 2, 1, &[0x00, 0x00, 0x00]);

    let mut aliases = vec![0x00, 0x00, 0x01, 0x00];
    push_name(
        &mut aliases,
        wasm::NESTED_RECORD_COMPONENT_CANONICAL_EXPORT_V6,
    );
    aliases.extend([0x00, 0x02, 0x01, 0x00]);
    push_name(&mut aliases, "memory");
    push_counted_section(&mut bytes, 6, 2, &aliases);

    push_section(&mut bytes, 7, &component_types());
    // Canon lift core-func 0, UTF-8 plus core-memory 0, component-func type 5.
    push_counted_section(
        &mut bytes,
        8,
        1,
        &[0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x05],
    );

    let mut interface = vec![0x01, 0x04]; // from-exports, four exports
    for (index, name) in TYPE_EXPORTS.into_iter().enumerate() {
        interface.push(0x00);
        push_name(&mut interface, name);
        interface.extend([0x03, 0x01 + index as u8]);
    }
    interface.push(0x00);
    push_name(&mut interface, FUNCTION_EXPORT);
    interface.extend([0x01, 0x00]);
    push_counted_section(&mut bytes, 5, 1, &interface);

    let mut export = vec![0x00];
    push_name(&mut export, INTERFACE_EXPORT);
    export.extend([0x05, 0x00, 0x00]);
    push_counted_section(&mut bytes, 11, 1, &export);
    bytes
}

fn component_types() -> Vec<u8> {
    let mut types = vec![0x06];
    // type 0: option<bool>; type 1: status.
    types.extend([0x6b, 0x7f, 0x72, 0x04]);
    push_name(&mut types, "domain");
    types.push(0x73);
    push_name(&mut types, "code");
    types.push(0x79);
    push_name(&mut types, "class");
    types.push(0x7d);
    push_name(&mut types, "retryable");
    types.push(0x00);
    // type 2: inner { value: s64, flag: bool }.
    types.extend([0x72, 0x02]);
    push_name(&mut types, "value");
    types.push(0x78);
    push_name(&mut types, "flag");
    types.push(0x7f);
    // type 3: outer { inner: inner, other: s64 }.
    types.extend([0x72, 0x02]);
    push_name(&mut types, "inner");
    types.push(0x02);
    push_name(&mut types, "other");
    types.push(0x78);
    // type 4: result<outer,status>.
    types.extend([0x6a, 0x01, 0x03, 0x01, 0x01]);
    // type 5: transform(input: outer, delta: s64) -> result<outer,status>.
    types.extend([0x40, 0x02]);
    push_name(&mut types, "input");
    types.push(0x03);
    push_name(&mut types, "delta");
    types.extend([0x78, 0x00, 0x04]);
    types
}

pub fn validate_private_nested_record_component_v6<'a>(
    candidate: &'a [u8],
    expected_source_revision: &str,
    expected_generated_core_digest: [u8; 32],
) -> Result<ValidatedPrivateNestedRecordComponentV6<'a>, PrivateComponentValidationError> {
    if expected_source_revision != SOURCE_REVISION_KAT
        || expected_generated_core_digest != GENERATED_CORE_KAT
    {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let mut component = Cursor::new(candidate);
    if component.take(8)? != COMPONENT_HEADER {
        return Err(PrivateComponentValidationError::Header);
    }
    let core = component.section(1)?;
    if <[u8; 32]>::from(Sha256::digest(core)) != expected_generated_core_digest {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let source_revision = validate_core(core, expected_source_revision)?;
    super::validate_exact_counted_section(
        component.section(2)?,
        &[0x00, 0x00, 0x00],
        PrivateComponentValidationError::Profile,
    )?;

    let mut aliases = Cursor::new(component.section(6)?);
    aliases.expect_u32(2, PrivateComponentValidationError::Profile)?;
    aliases.expect_bytes(
        &[0x00, 0x00, 0x01, 0x00],
        PrivateComponentValidationError::Profile,
    )?;
    aliases.expect_name(
        wasm::NESTED_RECORD_COMPONENT_CANONICAL_EXPORT_V6,
        PrivateComponentValidationError::Profile,
    )?;
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
    super::validate_exact_counted_section(
        component.section(8)?,
        &[0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x05],
        PrivateComponentValidationError::Profile,
    )?;

    let mut interface = Cursor::new(component.section(5)?);
    interface.expect_u32(1, PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x01], PrivateComponentValidationError::Profile)?;
    interface.expect_u32(4, PrivateComponentValidationError::Profile)?;
    for (index, name) in TYPE_EXPORTS.into_iter().enumerate() {
        interface.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
        interface.expect_name(name, PrivateComponentValidationError::Profile)?;
        interface.expect_bytes(
            &[0x03, 0x01 + index as u8],
            PrivateComponentValidationError::Profile,
        )?;
    }
    interface.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
    interface.expect_name(FUNCTION_EXPORT, PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x01, 0x00], PrivateComponentValidationError::Profile)?;
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

    Ok(ValidatedPrivateNestedRecordComponentV6 {
        generated_core: core,
        source_revision,
    })
}

fn validate_core<'a>(
    core: &'a [u8],
    expected_source_revision: &str,
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
    exports.expect_u32(2, PrivateComponentValidationError::CoreModule)?;
    exports.expect_name("memory", PrivateComponentValidationError::CoreModule)?;
    exports.expect_bytes(&[0x02, 0x00], PrivateComponentValidationError::CoreModule)?;
    exports.expect_name(
        wasm::NESTED_RECORD_COMPONENT_CANONICAL_EXPORT_V6,
        PrivateComponentValidationError::CoreModule,
    )?;
    exports.expect_bytes(&[0x00, 0x01], PrivateComponentValidationError::CoreModule)?;
    exports.finish(PrivateComponentValidationError::CoreModule)?;
    if module.section(10)?.is_empty() || module.section(11)?.is_empty() {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let mut custom = Cursor::new(module.section(0)?);
    custom.expect_name(
        "semaprax.component-nested-record-v6",
        PrivateComponentValidationError::CoreModule,
    )?;
    let revision_length =
        usize::try_from(custom.u32()?).map_err(|_| PrivateComponentValidationError::Encoding)?;
    let revision = std::str::from_utf8(custom.take(revision_length)?)
        .map_err(|_| PrivateComponentValidationError::CoreModule)?;
    if revision != expected_source_revision {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    custom.take(32)?;
    custom.take(32)?;
    custom.finish(PrivateComponentValidationError::CoreModule)?;
    module.finish(PrivateComponentValidationError::CoreModule)?;
    Ok(revision)
}

struct ProfileEvidence {
    source_revision: String,
    inner_layout_digest: [u8; 32],
    outer_layout_digest: [u8; 32],
}

fn profile_evidence(program: &Program) -> Result<ProfileEvidence, Diagnostic> {
    let expected = crate::parse(
        wasm::NESTED_RECORD_COMPONENT_SOURCE_V6,
        std::path::Path::new("nested-record-v6-profile.spx"),
    )?;
    if crate::format::canonical(program) != crate::format::canonical(&expected) {
        return Err(profile_error(
            "nested-record component requires the exact frozen source semantics",
        ));
    }
    let resolved = hir::resolve(program).map_err(first_error)?;
    hir::validate(&resolved)?;
    let authored = resolved
        .types
        .iter()
        .filter(|declaration| {
            !resolved
                .declarations
                .declaration(&declaration.id)
                .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned)
        })
        .collect::<Vec<_>>();
    let outer_ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("component.outer"),
        arguments: Vec::new(),
    };
    if authored.len() != 2
        || authored[0].id != DeclarationId::new("component.inner")
        || authored[1].id != DeclarationId::new("component.outer")
        || !matches!(authored[0].kind, ResolvedTypeDeclarationKind::Record { .. })
        || !matches!(authored[1].kind, ResolvedTypeDeclarationKind::Record { .. })
        || !resolved.permits.is_empty()
        || !resolved.interfaces.is_empty()
        || resolved.functions.len() != 2
        || resolved.functions[0].id != DeclarationId::new("component.transform")
        || resolved.functions[0].params.len() != 2
        || resolved.functions[0].params[0].ty != outer_ty
        || resolved.functions[0].params[1].ty != ResolvedType::I64
        || resolved.functions[0].return_type != outer_ty
        || !resolved.functions[0].effects.is_empty()
        || resolved.functions[1].id != DeclarationId::new("app.main")
    {
        return Err(profile_error(
            "nested-record component requires its exact capability-free declaration table",
        ));
    }
    let inner = AggregateLayout::for_record(
        &resolved,
        AggregateTarget::Wasm32,
        &DeclarationId::new("component.inner"),
    )?;
    let outer = AggregateLayout::for_record(
        &resolved,
        AggregateTarget::Wasm32,
        &DeclarationId::new("component.outer"),
    )?;
    inner.validate(&resolved)?;
    outer.validate(&resolved)?;
    if inner.size != 16 || inner.align != 8 || outer.size != 24 || outer.align != 8 {
        return Err(profile_error(
            "nested-record component Wasm32 layout changed",
        ));
    }
    Ok(ProfileEvidence {
        source_revision: crate::graph::revision(program),
        inner_layout_digest: inner.digest(),
        outer_layout_digest: outer.digest(),
    })
}

fn profile_digest(evidence: &ProfileEvidence) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PROFILE_DIGEST_DOMAIN);
    for field in [WIT_V6.as_bytes(), PROFILE] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field);
    }
    hash.update(evidence.inner_layout_digest);
    hash.update(evidence.outer_layout_digest);
    hash.finalize().into()
}

fn artifact_digest(
    source_revision: &str,
    generated_core_digest: &[u8; 32],
    profile_digest: &[u8; 32],
    bytes: &[u8],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(COMPONENT_DIGEST_DOMAIN);
    hash.update((source_revision.len() as u64).to_le_bytes());
    hash.update(source_revision.as_bytes());
    hash.update(generated_core_digest);
    hash.update(profile_digest);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

fn first_error(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.severity.is_error())
        .unwrap_or_else(|| profile_error("nested-record component HIR resolution failed"))
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-WIT108", message.into())
}

#[cfg(test)]
#[path = "nested_record_v6/tests.rs"]
mod tests;
