//! Exact private source-`Result` Component Model v4 composition.

use sha2::{Digest, Sha256};

use crate::aggregate_layout::AggregateTarget;
use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, DeclarationId, IdentityOrigin, ResolvedType};
use crate::prelude;
use crate::variant_layout::VariantLayout;
use crate::wasm;

use super::{
    push_counted_section, push_name, push_section, Cursor, PrivateComponentValidationError,
    COMPONENT_HEADER,
};

const INTERFACE_EXPORT: &str = "semaprax:private/evaluation@0.2.0";
const FUNCTION_EXPORT: &str = "evaluate";
const LANGUAGE_RESULT_EXPORT: &str = "language-result";

const WIT_V4: &str = "package semaprax:private@0.2.0;\n\ninterface evaluation {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  type language-result = result<bool, bool>;\n  evaluate: func(value: s64, reject: bool, divisor: s64) -> result<language-result, status>;\n}\n\nworld semaprax-private-v4 {\n  export evaluation;\n}\n";

const PROFILE: &[u8] = b"semaprax.private-source-result-component.v4\0canonical-abi-memory32-utf8\0nested-language-result-never-flattened\0status-first-known-v3-domains\0invalid-tag-and-unknown-status-trap\0canonical-result-area-256-size20-align4\0outer-payload-offset4-inner-tag-offset4-inner-bool-offset5\0compiler-result-layout-v2-field-reconstruction\0cleanup-plan-v2\0";
const PROFILE_DIGEST_DOMAIN: &[u8] = b"semaprax.private-source-result-component-profile.v4\0";
const COMPONENT_DIGEST_DOMAIN: &[u8] = b"semaprax.private-source-result-component-artifact.v4\0";

const RESULT_I64_BOOL: [ResolvedType; 2] = [ResolvedType::I64, ResolvedType::Bool];
const RESULT_BOOL_BOOL: [ResolvedType; 2] = [ResolvedType::Bool, ResolvedType::Bool];

/// Compiler-bound, import-free private Component Model artifact for the exact
/// nested source-language `Result<bool, bool>` projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateSourceResultComponentArtifactV4 {
    bytes: Vec<u8>,
    digest: [u8; 32],
    generated_core_digest: [u8; 32],
    profile_digest: [u8; 32],
    prelude_digest: [u8; 32],
    result_i64_bool_layout_digest: [u8; 32],
    result_bool_bool_layout_digest: [u8; 32],
    source_revision: String,
}

impl PrivateSourceResultComponentArtifactV4 {
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
    pub const fn prelude_digest(&self) -> [u8; 32] {
        self.prelude_digest
    }

    #[must_use]
    pub const fn result_i64_bool_layout_digest(&self) -> [u8; 32] {
        self.result_i64_bool_layout_digest
    }

    #[must_use]
    pub const fn result_bool_bool_layout_digest(&self) -> [u8; 32] {
        self.result_bool_bool_layout_digest
    }

    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    #[must_use]
    pub const fn wit(&self) -> &'static str {
        WIT_V4
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPrivateSourceResultComponentV4<'a> {
    generated_core: &'a [u8],
    source_revision: &'a str,
}

impl<'a> ValidatedPrivateSourceResultComponentV4<'a> {
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
    pub const fn language_result_export_name(self) -> &'static str {
        LANGUAGE_RESULT_EXPORT
    }
}

pub fn emit_private_source_result_component_v4(
    program: &Program,
) -> Result<PrivateSourceResultComponentArtifactV4, Diagnostic> {
    let evidence = profile_evidence(program)?;
    let core = wasm::emit_private_source_result_core_v4(program)?;
    if core.source_revision != evidence.source_revision
        || core.prelude_digest != evidence.prelude_digest
        || core.result_i64_bool_layout_digest != evidence.result_i64_bool_layout_digest
        || core.result_bool_bool_layout_digest != evidence.result_bool_bool_layout_digest
    {
        return Err(profile_error(
            "source-result core bindings disagree with independently admitted source meaning",
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
    Ok(PrivateSourceResultComponentArtifactV4 {
        bytes,
        digest,
        generated_core_digest,
        profile_digest,
        prelude_digest: evidence.prelude_digest,
        result_i64_bool_layout_digest: evidence.result_i64_bool_layout_digest,
        result_bool_bool_layout_digest: evidence.result_bool_bool_layout_digest,
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
        wasm::SOURCE_RESULT_COMPONENT_CANONICAL_EXPORT_V4,
    );
    aliases.extend([0x00, 0x02, 0x01, 0x00]);
    push_name(&mut aliases, "memory");
    push_counted_section(&mut bytes, 6, 2, &aliases);

    push_section(&mut bytes, 7, &component_types());
    // Canon lift core-func 0, UTF-8 plus core-memory 0, component-func type 4.
    push_counted_section(
        &mut bytes,
        8,
        1,
        &[0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04],
    );

    let mut interface = vec![0x01, 0x03]; // from-exports, three exports
    interface.push(0x00);
    push_name(&mut interface, "status");
    interface.extend([0x03, 0x01]); // component type 1
    interface.push(0x00);
    push_name(&mut interface, LANGUAGE_RESULT_EXPORT);
    interface.extend([0x03, 0x02]); // component type 2
    interface.push(0x00);
    push_name(&mut interface, FUNCTION_EXPORT);
    interface.extend([0x01, 0x00]); // component function 0
    push_counted_section(&mut bytes, 5, 1, &interface);

    let mut export = vec![0x00];
    push_name(&mut export, INTERFACE_EXPORT);
    export.extend([0x05, 0x00, 0x00]);
    push_counted_section(&mut bytes, 11, 1, &export);
    bytes
}

fn component_types() -> Vec<u8> {
    let mut types = vec![0x05];
    // type 0: option<bool>
    types.extend([0x6b, 0x7f]);
    // type 1: record status
    types.extend([0x72, 0x04]);
    push_name(&mut types, "domain");
    types.push(0x73);
    push_name(&mut types, "code");
    types.push(0x79);
    push_name(&mut types, "class");
    types.push(0x7d);
    push_name(&mut types, "retryable");
    types.push(0x00);
    // type 2: language-result = result<bool, bool>
    types.extend([0x6a, 0x01, 0x7f, 0x01, 0x7f]);
    // type 3: result<language-result, status>
    types.extend([0x6a, 0x01, 0x02, 0x01, 0x01]);
    // type 4: evaluate(value: s64, reject: bool, divisor: s64) -> type 3
    types.extend([0x40, 0x03]);
    push_name(&mut types, "value");
    types.push(0x78);
    push_name(&mut types, "reject");
    types.push(0x7f);
    push_name(&mut types, "divisor");
    types.extend([0x78, 0x00, 0x03]);
    types
}

pub fn validate_private_source_result_component_v4<'a>(
    candidate: &'a [u8],
    expected_source_revision: &str,
    expected_generated_core_digest: [u8; 32],
) -> Result<ValidatedPrivateSourceResultComponentV4<'a>, PrivateComponentValidationError> {
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
        wasm::SOURCE_RESULT_COMPONENT_CANONICAL_EXPORT_V4,
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
        &[0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04],
        PrivateComponentValidationError::Profile,
    )?;

    let mut interface = Cursor::new(component.section(5)?);
    interface.expect_u32(1, PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x01], PrivateComponentValidationError::Profile)?;
    interface.expect_u32(3, PrivateComponentValidationError::Profile)?;
    for (name, index) in [("status", 1_u8), (LANGUAGE_RESULT_EXPORT, 2_u8)] {
        interface.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
        interface.expect_name(name, PrivateComponentValidationError::Profile)?;
        interface.expect_bytes(&[0x03, index], PrivateComponentValidationError::Profile)?;
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

    Ok(ValidatedPrivateSourceResultComponentV4 {
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
    super::validate_exact_payload(
        module.section(1)?,
        &[
            0x04, 0x60, 0x03, 0x7e, 0x7f, 0x7f, 0x01, 0x7f, 0x60, 0x04, 0x7e, 0x7f, 0x7e, 0x7f,
            0x01, 0x7f, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f, 0x60, 0x03, 0x7e, 0x7f, 0x7e, 0x01,
            0x7f,
        ],
        PrivateComponentValidationError::CoreModule,
    )?;
    super::validate_exact_payload(
        module.section(3)?,
        &[0x05, 0x00, 0x01, 0x02, 0x01, 0x03],
        PrivateComponentValidationError::CoreModule,
    )?;
    super::validate_exact_payload(
        module.section(5)?,
        &[0x01, 0x00, 0x01],
        PrivateComponentValidationError::CoreModule,
    )?;
    // The selected-function core owns one mutable shadow-stack global.
    super::validate_exact_payload(
        module.section(6)?,
        &[0x01, 0x7f, 0x01, 0x41, 0x80, 0x80, 0x04, 0x0b],
        PrivateComponentValidationError::CoreModule,
    )?;
    let mut exports = Cursor::new(module.section(7)?);
    exports.expect_u32(3, PrivateComponentValidationError::CoreModule)?;
    exports.expect_name("memory", PrivateComponentValidationError::CoreModule)?;
    exports.expect_bytes(&[0x02, 0x00], PrivateComponentValidationError::CoreModule)?;
    exports.expect_name(
        wasm::SOURCE_RESULT_COMPONENT_STATUS_OUT_EXPORT_V4,
        PrivateComponentValidationError::CoreModule,
    )?;
    exports.expect_bytes(&[0x00, 0x03], PrivateComponentValidationError::CoreModule)?;
    exports.expect_name(
        wasm::SOURCE_RESULT_COMPONENT_CANONICAL_EXPORT_V4,
        PrivateComponentValidationError::CoreModule,
    )?;
    exports.expect_bytes(&[0x00, 0x04], PrivateComponentValidationError::CoreModule)?;
    exports.finish(PrivateComponentValidationError::CoreModule)?;
    if module.section(10)?.is_empty() || module.section(11)?.is_empty() {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let mut custom = Cursor::new(module.section(0)?);
    custom.expect_name(
        "semaprax.component-source-result-v4",
        PrivateComponentValidationError::CoreModule,
    )?;
    let revision_length =
        usize::try_from(custom.u32()?).map_err(|_| PrivateComponentValidationError::Encoding)?;
    let revision_bytes = custom.take(revision_length)?;
    let revision = std::str::from_utf8(revision_bytes)
        .map_err(|_| PrivateComponentValidationError::CoreModule)?;
    if revision != expected_source_revision {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    custom.take(32)?; // compiler-owned prelude digest
    custom.take(32)?; // Result<i64, bool> Wasm32 layout-v2 digest
    custom.take(32)?; // Result<bool, bool> Wasm32 layout-v2 digest
    custom.finish(PrivateComponentValidationError::CoreModule)?;
    module.finish(PrivateComponentValidationError::CoreModule)?;
    Ok(revision)
}

struct ProfileEvidence {
    source_revision: String,
    prelude_digest: [u8; 32],
    result_i64_bool_layout_digest: [u8; 32],
    result_bool_bool_layout_digest: [u8; 32],
}

fn profile_evidence(program: &Program) -> Result<ProfileEvidence, Diagnostic> {
    let resolved = hir::resolve(program).map_err(first_error)?;
    hir::validate(&resolved)?;
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id == DeclarationId::new("component.evaluate"))
        .ok_or_else(|| {
            profile_error("source-result component requires `@id(\"component.evaluate\")`")
        })?;
    let expected_return = result_type(&RESULT_BOOL_BOOL);
    if !resolved.permits.is_empty()
        || !resolved.interfaces.is_empty()
        || resolved
            .functions
            .iter()
            .any(|item| !item.effects.is_empty())
        || resolved.types.iter().any(|declaration| {
            !resolved
                .declarations
                .declaration(&declaration.id)
                .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned)
        })
        || function.params.len() != 3
        || function.params[0].ty != ResolvedType::I64
        || function.params[1].ty != ResolvedType::Bool
        || function.params[2].ty != ResolvedType::I64
        || function.return_type != expected_return
    {
        return Err(profile_error(
            "source-result component requires a capability-free `(i64, bool, i64) -> Result<bool, bool>` function and only compiler-owned prelude types",
        ));
    }
    for id in [
        prelude::RESULT_ID,
        prelude::RESULT_OK_ID,
        prelude::RESULT_OK_VALUE_ID,
        prelude::RESULT_ERR_ID,
        prelude::RESULT_ERR_ERROR_ID,
    ] {
        if !resolved
            .declarations
            .declaration(&DeclarationId::new(id))
            .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned)
        {
            return Err(profile_error(
                "source-result component does not authenticate the compiler-owned Result prelude",
            ));
        }
    }
    let i64_bool = VariantLayout::for_type(
        &resolved,
        AggregateTarget::Wasm32,
        &result_type(&RESULT_I64_BOOL),
    )?;
    let bool_bool = VariantLayout::for_type(&resolved, AggregateTarget::Wasm32, &expected_return)?;
    i64_bool.validate(&resolved)?;
    bool_bool.validate(&resolved)?;
    Ok(ProfileEvidence {
        source_revision: crate::graph::revision(program),
        prelude_digest: prelude::digest_v1(),
        result_i64_bool_layout_digest: i64_bool.digest(),
        result_bool_bool_layout_digest: bool_bool.digest(),
    })
}

fn result_type(arguments: &[ResolvedType; 2]) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(prelude::RESULT_ID),
        arguments: arguments.to_vec(),
    }
}

fn profile_digest(evidence: &ProfileEvidence) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PROFILE_DIGEST_DOMAIN);
    for field in [
        WIT_V4.as_bytes(),
        PROFILE,
        prelude::SCHEMA_V1.as_bytes(),
        prelude::RESULT_ID.as_bytes(),
        prelude::RESULT_OK_ID.as_bytes(),
        prelude::RESULT_OK_VALUE_ID.as_bytes(),
        prelude::RESULT_ERR_ID.as_bytes(),
        prelude::RESULT_ERR_ERROR_ID.as_bytes(),
    ] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field);
    }
    hash.update(evidence.prelude_digest);
    hash.update(evidence.result_i64_bool_layout_digest);
    hash.update(evidence.result_bool_bool_layout_digest);
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
        .unwrap_or_else(|| profile_error("source-result component HIR resolution failed"))
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-WIT108", message.into())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    const SOURCE: &str = r#"module test.component_source_result_v4;

@id("component.source")
fn source(value: i64, reject: bool) -> Result<i64, bool> {
    if reject {
        Result<i64, bool>::Err { error: value > 0 }
    } else {
        Result<i64, bool>::Ok { value: value }
    }
}

@id("component.evaluate")
fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<bool, bool>
    requires value != -99
    ensures divisor != 13
{
    let checked = source(value, reject)?;
    Result<bool, bool>::Ok { value: (checked + 1) / divisor > 0 }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn artifact() -> PrivateSourceResultComponentArtifactV4 {
        let program = crate::parse(SOURCE, Path::new("component-source-result-v4.spx")).unwrap();
        emit_private_source_result_component_v4(&program).unwrap()
    }

    #[test]
    fn deterministic_v4_artifact_is_exactly_parsed_and_upstream_valid() {
        let first = artifact();
        assert_eq!(
            first.generated_core_digest(),
            [
                0x54, 0xfa, 0x28, 0x22, 0xc5, 0x1a, 0x71, 0xce, 0xbf, 0xd8, 0x8d, 0x37, 0x9b, 0x45,
                0xc3, 0x7f, 0xfd, 0x3d, 0x0f, 0x0b, 0x28, 0x93, 0xcb, 0x4f, 0x29, 0x66, 0xf9, 0xe2,
                0xdb, 0x6d, 0x5e, 0x5f,
            ],
            "generated-core KAT changed"
        );
        assert_eq!(
            first.profile_digest(),
            [
                0xfa, 0x1f, 0x0b, 0x5e, 0xca, 0x07, 0xb4, 0xb3, 0xcb, 0xa2, 0xc3, 0xd9, 0xc5, 0xfd,
                0xd0, 0x07, 0x27, 0x6d, 0x7f, 0xa6, 0x72, 0xa3, 0xe4, 0xa4, 0x9e, 0x9f, 0xfd, 0x20,
                0xd3, 0xdc, 0xe0, 0x6c,
            ],
            "profile KAT changed"
        );
        assert_eq!(
            first.digest(),
            [
                0xf5, 0xfa, 0x5a, 0xe3, 0x90, 0x5d, 0x30, 0xc9, 0x98, 0xf7, 0x83, 0xe9, 0xb7, 0x78,
                0x67, 0x98, 0x68, 0x13, 0xb0, 0xe8, 0xb4, 0x41, 0x2f, 0xa4, 0xaf, 0xa9, 0x8e, 0x93,
                0x2e, 0xda, 0x4d, 0x40,
            ],
            "component DAG KAT changed"
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(first.bytes())),
            [
                0x3e, 0x7b, 0x9c, 0x2d, 0xdc, 0x8c, 0xa6, 0xfd, 0xfa, 0x80, 0x1e, 0xb5, 0x0a, 0xe3,
                0xa2, 0x15, 0x31, 0xfc, 0xe4, 0x46, 0x77, 0x34, 0x5d, 0xde, 0xa6, 0x8d, 0x20, 0x58,
                0x1c, 0x79, 0xb2, 0x3b,
            ],
            "exact component-byte SHA-256 KAT changed"
        );
        assert_eq!(
            first.prelude_digest(),
            [
                0xd3, 0x7b, 0xad, 0x7e, 0x39, 0x11, 0x66, 0x9b, 0xbf, 0x2c, 0x66, 0xb2, 0x5c, 0x8b,
                0x31, 0xd5, 0xc2, 0xe3, 0x6e, 0xb1, 0x81, 0xcc, 0x54, 0xfd, 0xc8, 0x6c, 0x3a, 0x49,
                0xa8, 0xfb, 0x9c, 0x5e,
            ],
            "prelude KAT changed"
        );
        assert_eq!(
            first.result_i64_bool_layout_digest(),
            [
                0xc0, 0x11, 0x12, 0xf9, 0x09, 0xa0, 0x74, 0x34, 0x3a, 0xe4, 0xeb, 0x3a, 0xbd, 0xe6,
                0xad, 0x70, 0x93, 0x02, 0x80, 0xe4, 0xa8, 0x01, 0x6c, 0x16, 0x5e, 0x05, 0xf3, 0x17,
                0xbe, 0xd9, 0xf1, 0x99,
            ],
            "Result<i64, bool> layout-v2 KAT changed"
        );
        assert_eq!(
            first.result_bool_bool_layout_digest(),
            [
                0x39, 0xaf, 0x02, 0x08, 0x45, 0x88, 0x12, 0x6c, 0x5f, 0x6d, 0x20, 0xab, 0x8f, 0x3e,
                0xf1, 0xf8, 0x24, 0x9b, 0x8c, 0xa1, 0x9e, 0x52, 0x15, 0x33, 0x98, 0xa5, 0x21, 0xc2,
                0xc4, 0x9a, 0x55, 0x8d,
            ],
            "Result<bool, bool> layout-v2 KAT changed"
        );
        assert_eq!(first, artifact());
        assert_eq!(first.wit(), WIT_V4);
        assert_eq!(
            first.source_revision(),
            "sha256:4391bc27b5db547f2b162c2b5467c2b75797e8a5ef64e4ffe4abef15678c6254",
            "source revision KAT changed"
        );
        let validated = validate_private_source_result_component_v4(
            first.bytes(),
            first.source_revision(),
            first.generated_core_digest(),
        )
        .unwrap();
        assert_eq!(validated.source_revision(), first.source_revision());
        assert_eq!(validated.interface_export_name(), INTERFACE_EXPORT);
        assert_eq!(validated.function_export_name(), FUNCTION_EXPORT);
        assert_eq!(
            validated.language_result_export_name(),
            LANGUAGE_RESULT_EXPORT
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(validated.generated_core())),
            first.generated_core_digest()
        );
        assert_eq!(first.prelude_digest(), prelude::digest_v1());
        assert_ne!(
            first.result_i64_bool_layout_digest(),
            first.result_bool_bool_layout_digest()
        );
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(first.bytes())
            .expect("pinned upstream validator rejected source-result component v4");
    }

    #[test]
    fn every_byte_truncation_trailing_and_noncanonical_length_reject() {
        let artifact = artifact();
        for index in 0..artifact.bytes().len() {
            let mut hostile = artifact.bytes().to_vec();
            hostile[index] ^= 1;
            assert!(
                validate_private_source_result_component_v4(
                    &hostile,
                    artifact.source_revision(),
                    artifact.generated_core_digest(),
                )
                .is_err(),
                "source-result component byte {index} escaped authentication"
            );
        }
        for end in 0..artifact.bytes().len() {
            assert!(validate_private_source_result_component_v4(
                &artifact.bytes()[..end],
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
        }
        let mut trailing = artifact.bytes().to_vec();
        trailing.push(0);
        assert!(validate_private_source_result_component_v4(
            &trailing,
            artifact.source_revision(),
            artifact.generated_core_digest(),
        )
        .is_err());
        let mut noncanonical = artifact.bytes().to_vec();
        let needle = [0x02, 0x04, 0x01, 0x00, 0x00, 0x00];
        let offsets = noncanonical
            .windows(needle.len())
            .enumerate()
            .filter_map(|(index, window)| (window == needle).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(offsets.len(), 1, "core-instance section anchor drifted");
        noncanonical.splice(offsets[0] + 1..offsets[0] + 2, [0x84, 0x00]);
        assert_eq!(
            validate_private_source_result_component_v4(
                &noncanonical,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            ),
            Err(PrivateComponentValidationError::Encoding)
        );
    }

    #[test]
    fn v1_v2_v3_v4_profiles_are_never_confused() {
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
        let v4 = artifact();
        for candidate in [v1.bytes(), v2.bytes(), v3.bytes()] {
            assert!(validate_private_source_result_component_v4(
                candidate,
                v4.source_revision(),
                v4.generated_core_digest(),
            )
            .is_err());
        }
        assert!(super::super::validate_private_component_v1(v4.bytes()).is_err());
        assert!(super::super::validate_private_checked_component_v2(
            v4.bytes(),
            v2.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_result_component_v3(
            v4.bytes(),
            v3.source_revision(),
            v3.generated_core_digest(),
        )
        .is_err());
    }

    #[test]
    fn rehashed_flattened_named_type_lift_and_export_hostiles_reject() {
        let artifact = artifact();
        let hostiles = [
            (
                &[0x6a, 0x01, 0x7f, 0x01, 0x7f][..],
                4,
                0x78,
                "inner-error-type",
            ),
            (
                &[0x6a, 0x01, 0x02, 0x01, 0x01][..],
                2,
                0x7f,
                "flattened-outer-ok",
            ),
            (
                &[0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04][..],
                7,
                0x03,
                "lift-type",
            ),
            (&[0x05, 0x00, 0x00][..], 0, 0x01, "interface-kind"),
        ];
        for (needle, relative, replacement, name) in hostiles {
            let mut hostile = artifact.bytes().to_vec();
            let offsets = hostile
                .windows(needle.len())
                .enumerate()
                .filter_map(|(index, window)| (window == needle).then_some(index))
                .collect::<Vec<_>>();
            assert_eq!(offsets.len(), 1, "hostile anchor {name} must be unique");
            hostile[offsets[0] + relative] = replacement;
            assert!(validate_private_source_result_component_v4(
                &hostile,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
        }
    }

    #[test]
    fn excluded_authority_type_and_signature_profiles_fail_closed() {
        for source in [
            SOURCE.replace(
                "fn evaluate(value: i64, reject: bool, divisor: i64)",
                "fn evaluate(value: i64, reject: bool, divisor: i64, extra: bool)",
            ),
            SOURCE
                .replace(
                    "fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<bool, bool>",
                    "fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<i64, bool>",
                )
                .replace(
                    "Result<bool, bool>::Ok { value: (checked + 1) / divisor > 0 }",
                    "Result<i64, bool>::Ok { value: checked }",
                ),
            SOURCE.replace(
                "module test.component_source_result_v4;",
                "module test.component_source_result_v4;\npermit { clock.read }",
            ).replace(
                "fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<bool, bool>",
                "fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<bool, bool> uses { clock.read }",
            ),
        ] {
            let program = crate::parse(
                &source,
                Path::new("excluded-component-source-result-v4.spx"),
            )
            .unwrap();
            assert_eq!(
                emit_private_source_result_component_v4(&program)
                    .unwrap_err()
                    .code,
                "SPX-WIT108"
            );
        }
    }
}
