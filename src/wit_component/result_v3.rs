//! Exact private WIT result/status Component Model v3 composition.

use sha2::{Digest, Sha256};

use crate::{ast::Program, diagnostic::Diagnostic, wasm};

use super::{
    push_counted_section, push_name, push_section, Cursor, PrivateComponentValidationError,
    COMPONENT_HEADER, WIT,
};

const INTERFACE_EXPORT: &str = "semaprax:private/evaluation@0.1.0";
const FUNCTION_EXPORT: &str = "evaluate";
const PROFILE: &[u8] = b"semaprax.private-result-component.v3\0canonical-abi-memory32-utf8\0status-word-class24-code24\0result-area-256\0";
const PROFILE_DIGEST_DOMAIN: &[u8] = b"semaprax.private-result-component-profile.v3\0";
const COMPONENT_DIGEST_DOMAIN: &[u8] = b"semaprax.private-result-component-artifact.v3\0";

/// Compiler-bound, import-free private Component Model artifact for the exact
/// `result<s64, status>` WIT projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateResultComponentArtifactV3 {
    bytes: Vec<u8>,
    digest: [u8; 32],
    generated_core_digest: [u8; 32],
    profile_digest: [u8; 32],
    source_revision: String,
}

impl PrivateResultComponentArtifactV3 {
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
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    #[must_use]
    pub const fn wit(&self) -> &'static str {
        WIT
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPrivateResultComponentV3<'a> {
    generated_core: &'a [u8],
    source_revision: &'a str,
}

impl<'a> ValidatedPrivateResultComponentV3<'a> {
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
}

pub fn emit_private_result_component_v3(
    program: &Program,
) -> Result<PrivateResultComponentArtifactV3, Diagnostic> {
    let core = wasm::emit_private_result_core_v3(program)?;
    let generated_core_digest: [u8; 32] = Sha256::digest(&core.bytes).into();
    let profile_digest = profile_digest();
    let bytes = compose(&core.bytes);
    let digest = artifact_digest(
        &core.source_revision,
        &generated_core_digest,
        &profile_digest,
        &bytes,
    );
    Ok(PrivateResultComponentArtifactV3 {
        bytes,
        digest,
        generated_core_digest,
        profile_digest,
        source_revision: core.source_revision,
    })
}

fn compose(core: &[u8]) -> Vec<u8> {
    let mut bytes = COMPONENT_HEADER.to_vec();
    push_section(&mut bytes, 1, core);
    push_counted_section(&mut bytes, 2, 1, &[0x00, 0x00, 0x00]);

    let mut aliases = vec![0x00, 0x00, 0x01, 0x00];
    push_name(&mut aliases, wasm::RESULT_COMPONENT_CANONICAL_EXPORT_V3);
    aliases.extend([0x00, 0x02, 0x01, 0x00]);
    push_name(&mut aliases, "memory");
    push_counted_section(&mut bytes, 6, 2, &aliases);

    push_section(&mut bytes, 7, &component_types());
    // canon lift core-func 0, UTF-8 plus core-memory 0, component-func type 3
    push_counted_section(
        &mut bytes,
        8,
        1,
        &[0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x03],
    );

    let mut interface = vec![0x01, 0x02]; // from-exports, two exports
    interface.push(0x00);
    push_name(&mut interface, "status");
    interface.extend([0x03, 0x01]); // component type 1
    interface.push(0x00); // component extern name discriminator
    push_name(&mut interface, FUNCTION_EXPORT);
    interface.extend([0x01, 0x00]); // component function 0
    push_counted_section(&mut bytes, 5, 1, &interface);

    let mut export = vec![0x00];
    push_name(&mut export, INTERFACE_EXPORT);
    export.extend([0x05, 0x00, 0x00]); // component instance 0, inferred exact type
    push_counted_section(&mut bytes, 11, 1, &export);
    bytes
}

fn component_types() -> Vec<u8> {
    let mut types = vec![0x04];
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
    types.push(0x00); // type 0
                      // type 2: result<s64, status>
    types.extend([0x6a, 0x01, 0x78, 0x01, 0x01]);
    // type 3: evaluate(left: s64, right: s64) -> type 2
    types.extend([0x40, 0x02]);
    push_name(&mut types, "left");
    types.push(0x78);
    push_name(&mut types, "right");
    types.extend([0x78, 0x00, 0x02]);
    types
}

pub fn validate_private_result_component_v3<'a>(
    candidate: &'a [u8],
    expected_source_revision: &str,
    expected_generated_core_digest: [u8; 32],
) -> Result<ValidatedPrivateResultComponentV3<'a>, PrivateComponentValidationError> {
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
        wasm::RESULT_COMPONENT_CANONICAL_EXPORT_V3,
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
        &[0x00, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x03],
        PrivateComponentValidationError::Profile,
    )?;

    let mut interface = Cursor::new(component.section(5)?);
    interface.expect_u32(1, PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x01], PrivateComponentValidationError::Profile)?;
    interface.expect_u32(2, PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
    interface.expect_name("status", PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x03, 0x01], PrivateComponentValidationError::Profile)?;
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

    Ok(ValidatedPrivateResultComponentV3 {
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
            0x02, 0x60, 0x03, 0x7e, 0x7e, 0x7f, 0x01, 0x7f, 0x60, 0x02, 0x7e, 0x7e, 0x01, 0x7f,
        ],
        PrivateComponentValidationError::CoreModule,
    )?;
    super::validate_exact_payload(
        module.section(3)?,
        &[0x02, 0x00, 0x01],
        PrivateComponentValidationError::CoreModule,
    )?;
    super::validate_exact_payload(
        module.section(5)?,
        &[0x01, 0x00, 0x01],
        PrivateComponentValidationError::CoreModule,
    )?;
    let mut exports = Cursor::new(module.section(7)?);
    exports.expect_u32(3, PrivateComponentValidationError::CoreModule)?;
    exports.expect_name("memory", PrivateComponentValidationError::CoreModule)?;
    exports.expect_bytes(&[0x02, 0x00], PrivateComponentValidationError::CoreModule)?;
    exports.expect_name(
        wasm::RESULT_COMPONENT_STATUS_OUT_EXPORT_V3,
        PrivateComponentValidationError::CoreModule,
    )?;
    exports.expect_bytes(&[0x00, 0x00], PrivateComponentValidationError::CoreModule)?;
    exports.expect_name(
        wasm::RESULT_COMPONENT_CANONICAL_EXPORT_V3,
        PrivateComponentValidationError::CoreModule,
    )?;
    exports.expect_bytes(&[0x00, 0x01], PrivateComponentValidationError::CoreModule)?;
    exports.finish(PrivateComponentValidationError::CoreModule)?;
    if module.section(10)?.is_empty() || module.section(11)?.is_empty() {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let mut custom = Cursor::new(module.section(0)?);
    custom.expect_name(
        "semaprax.component-result-v3",
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
    custom.finish(PrivateComponentValidationError::CoreModule)?;
    module.finish(PrivateComponentValidationError::CoreModule)?;
    Ok(revision)
}

fn profile_digest() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PROFILE_DIGEST_DOMAIN);
    hash.update((WIT.len() as u64).to_le_bytes());
    hash.update(WIT.as_bytes());
    hash.update((PROFILE.len() as u64).to_le_bytes());
    hash.update(PROFILE);
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

#[cfg(test)]
#[path = "result_v3/tests.rs"]
mod tests;
