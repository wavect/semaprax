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
#[path = "source_result_v4/tests.rs"]
mod tests;
