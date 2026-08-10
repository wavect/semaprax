//! Exact private direct-scalar algebraic Component Model v5 composition.

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

const INTERFACE_EXPORT: &str = "semaprax:private/scalar-algebra@0.3.0";
const FUNCTION_EXPORTS: [&str; 6] = [
    "option-i64",
    "option-bool",
    "result-i64-i64",
    "result-i64-bool",
    "result-bool-i64",
    "result-bool-bool",
];
const TYPE_EXPORTS: [&str; 6] = [
    "maybe-i64",
    "maybe-bool",
    "language-result-i64-i64",
    "language-result-i64-bool",
    "language-result-bool-i64",
    "language-result-bool-bool",
];

const WIT_V5: &str = "package semaprax:private@0.3.0;\n\ninterface scalar-algebra {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  type maybe-i64 = option<s64>;\n  type maybe-bool = option<bool>;\n  type language-result-i64-i64 = result<s64, s64>;\n  type language-result-i64-bool = result<s64, bool>;\n  type language-result-bool-i64 = result<bool, s64>;\n  type language-result-bool-bool = result<bool, bool>;\n  option-i64: func(value: s64, select: bool, divisor: s64) -> result<maybe-i64, status>;\n  option-bool: func(value: s64, select: bool, divisor: s64) -> result<maybe-bool, status>;\n  result-i64-i64: func(value: s64, select: bool, divisor: s64) -> result<language-result-i64-i64, status>;\n  result-i64-bool: func(value: s64, select: bool, divisor: s64) -> result<language-result-i64-bool, status>;\n  result-bool-i64: func(value: s64, select: bool, divisor: s64) -> result<language-result-bool-i64, status>;\n  result-bool-bool: func(value: s64, select: bool, divisor: s64) -> result<language-result-bool-bool, status>;\n}\n\nworld semaprax-private-v5 {\n  export scalar-algebra;\n}\n";

const PROFILE: &[u8] = b"semaprax.private-scalar-algebra-component.v5\0canonical-abi-memory32-utf8\0six-stable-id-exports\0option-i64-option-bool\0complete-result-i64-bool-matrix\0status-first-carrier-never-flattened\0no-layout-identity-inference\0compiler-prelude-layout-v2-reconstruction\0";
const PROFILE_DIGEST_DOMAIN: &[u8] = b"semaprax.private-scalar-algebra-component-profile.v5\0";
const COMPONENT_DIGEST_DOMAIN: &[u8] = b"semaprax.private-scalar-algebra-component-artifact.v5\0";
const SOURCE_REVISION_KAT: &str =
    "sha256:86411224efe3adace5ffdd410c243306859edc280dbe3342adcf830588b62259";
const GENERATED_CORE_KAT: [u8; 32] = [
    0x08, 0x25, 0xf2, 0x70, 0xcf, 0x2c, 0x94, 0xbd, 0x75, 0x19, 0x01, 0xd0, 0x5d, 0x74, 0x29, 0x3e,
    0x52, 0xb6, 0x9b, 0xda, 0x00, 0xa1, 0xaf, 0x99, 0xcd, 0xfb, 0xc4, 0x72, 0x53, 0x5a, 0xf3, 0x1b,
];

const SHAPES: [[ResolvedType; 2]; 6] = [
    [ResolvedType::I64, ResolvedType::I64],
    [ResolvedType::Bool, ResolvedType::Bool],
    [ResolvedType::I64, ResolvedType::I64],
    [ResolvedType::I64, ResolvedType::Bool],
    [ResolvedType::Bool, ResolvedType::I64],
    [ResolvedType::Bool, ResolvedType::Bool],
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateScalarAlgebraComponentArtifactV5 {
    bytes: Vec<u8>,
    digest: [u8; 32],
    generated_core_digest: [u8; 32],
    profile_digest: [u8; 32],
    prelude_digest: [u8; 32],
    layout_digests: [[u8; 32]; 6],
    source_revision: String,
}

impl PrivateScalarAlgebraComponentArtifactV5 {
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
    pub const fn layout_digests(&self) -> [[u8; 32]; 6] {
        self.layout_digests
    }

    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    #[must_use]
    pub const fn wit(&self) -> &'static str {
        WIT_V5
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPrivateScalarAlgebraComponentV5<'a> {
    generated_core: &'a [u8],
    source_revision: &'a str,
}

impl<'a> ValidatedPrivateScalarAlgebraComponentV5<'a> {
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
    pub const fn function_export_names(self) -> [&'static str; 6] {
        FUNCTION_EXPORTS
    }

    #[must_use]
    pub const fn type_export_names(self) -> [&'static str; 6] {
        TYPE_EXPORTS
    }
}

pub fn emit_private_scalar_algebra_component_v5(
    program: &Program,
) -> Result<PrivateScalarAlgebraComponentArtifactV5, Diagnostic> {
    let evidence = profile_evidence(program)?;
    let core = wasm::emit_private_scalar_algebra_core_v5(program)?;
    if core.source_revision != evidence.source_revision
        || core.prelude_digest != evidence.prelude_digest
        || core.layout_digests != evidence.layout_digests
    {
        return Err(profile_error(
            "scalar-algebra core disagrees with independently admitted source meaning",
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
    Ok(PrivateScalarAlgebraComponentArtifactV5 {
        bytes,
        digest,
        generated_core_digest,
        profile_digest,
        prelude_digest: evidence.prelude_digest,
        layout_digests: evidence.layout_digests,
        source_revision: evidence.source_revision,
    })
}

fn compose(core: &[u8]) -> Vec<u8> {
    let mut bytes = COMPONENT_HEADER.to_vec();
    push_section(&mut bytes, 1, core);
    push_counted_section(&mut bytes, 2, 1, &[0x00, 0x00, 0x00]);

    let mut aliases = Vec::new();
    for (index, name) in wasm::SCALAR_ALGEBRA_COMPONENT_CANONICAL_EXPORTS_V5
        .into_iter()
        .enumerate()
    {
        aliases.extend([0x00, 0x00, 0x01, 0x00]);
        push_name(&mut aliases, name);
        debug_assert!(index < 6);
    }
    aliases.extend([0x00, 0x02, 0x01, 0x00]);
    push_name(&mut aliases, "memory");
    push_counted_section(&mut bytes, 6, 7, &aliases);

    push_section(&mut bytes, 7, &component_types());
    let mut canonical = Vec::new();
    for index in 0_u8..6 {
        // Canon lift core-func index, UTF-8 plus core-memory 0, component-func type 14+index.
        canonical.extend([0x00, 0x00, index, 0x02, 0x00, 0x03, 0x00, 14 + index]);
    }
    push_counted_section(&mut bytes, 8, 6, &canonical);

    let mut interface = vec![0x01, 13]; // from-exports, thirteen exports
    interface.push(0x00);
    push_name(&mut interface, "status");
    interface.extend([0x03, 0x01]);
    for (offset, name) in TYPE_EXPORTS.into_iter().enumerate() {
        interface.push(0x00);
        push_name(&mut interface, name);
        interface.extend([0x03, 0x02 + offset as u8]);
    }
    for (offset, name) in FUNCTION_EXPORTS.into_iter().enumerate() {
        interface.push(0x00);
        push_name(&mut interface, name);
        interface.extend([0x01, offset as u8]);
    }
    push_counted_section(&mut bytes, 5, 1, &interface);

    let mut export = vec![0x00];
    push_name(&mut export, INTERFACE_EXPORT);
    export.extend([0x05, 0x00, 0x00]);
    push_counted_section(&mut bytes, 11, 1, &export);
    bytes
}

fn component_types() -> Vec<u8> {
    let mut types = vec![20];
    // type 0: option<bool>; type 1: status
    types.extend([0x6b, 0x7f, 0x72, 0x04]);
    push_name(&mut types, "domain");
    types.push(0x73);
    push_name(&mut types, "code");
    types.push(0x79);
    push_name(&mut types, "class");
    types.push(0x7d);
    push_name(&mut types, "retryable");
    types.push(0x00);
    // types 2..7: two options and four language results.
    types.extend([
        0x6b, 0x78, // option<s64>
        0x6b, 0x7f, // option<bool>
        0x6a, 0x01, 0x78, 0x01, 0x78, // result<s64,s64>
        0x6a, 0x01, 0x78, 0x01, 0x7f, // result<s64,bool>
        0x6a, 0x01, 0x7f, 0x01, 0x78, // result<bool,s64>
        0x6a, 0x01, 0x7f, 0x01, 0x7f, // result<bool,bool>
    ]);
    // types 8..13: outer result<carrier,status>.
    for carrier in 2_u8..8 {
        types.extend([0x6a, 0x01, carrier, 0x01, 0x01]);
    }
    // types 14..19: identical parameters, distinct exact result types.
    for result in 8_u8..14 {
        types.extend([0x40, 0x03]);
        push_name(&mut types, "value");
        types.push(0x78);
        push_name(&mut types, "select");
        types.push(0x7f);
        push_name(&mut types, "divisor");
        types.extend([0x78, 0x00, result]);
    }
    types
}

pub fn validate_private_scalar_algebra_component_v5<'a>(
    candidate: &'a [u8],
    expected_source_revision: &str,
    expected_generated_core_digest: [u8; 32],
) -> Result<ValidatedPrivateScalarAlgebraComponentV5<'a>, PrivateComponentValidationError> {
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
    aliases.expect_u32(7, PrivateComponentValidationError::Profile)?;
    for name in wasm::SCALAR_ALGEBRA_COMPONENT_CANONICAL_EXPORTS_V5 {
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
    for index in 0_u8..6 {
        expected_canonical.extend([0x00, 0x00, index, 0x02, 0x00, 0x03, 0x00, 14 + index]);
    }
    let mut canonical = Cursor::new(component.section(8)?);
    canonical.expect_u32(6, PrivateComponentValidationError::Profile)?;
    canonical.expect_bytes(
        &expected_canonical,
        PrivateComponentValidationError::Profile,
    )?;
    canonical.finish(PrivateComponentValidationError::Profile)?;

    let mut interface = Cursor::new(component.section(5)?);
    interface.expect_u32(1, PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x01], PrivateComponentValidationError::Profile)?;
    interface.expect_u32(13, PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
    interface.expect_name("status", PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x03, 0x01], PrivateComponentValidationError::Profile)?;
    for (offset, name) in TYPE_EXPORTS.into_iter().enumerate() {
        interface.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
        interface.expect_name(name, PrivateComponentValidationError::Profile)?;
        interface.expect_bytes(
            &[0x03, 0x02 + offset as u8],
            PrivateComponentValidationError::Profile,
        )?;
    }
    for (offset, name) in FUNCTION_EXPORTS.into_iter().enumerate() {
        interface.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
        interface.expect_name(name, PrivateComponentValidationError::Profile)?;
        interface.expect_bytes(
            &[0x01, offset as u8],
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

    Ok(ValidatedPrivateScalarAlgebraComponentV5 {
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
    exports.expect_u32(7, PrivateComponentValidationError::CoreModule)?;
    exports.expect_name("memory", PrivateComponentValidationError::CoreModule)?;
    exports.expect_bytes(&[0x02, 0x00], PrivateComponentValidationError::CoreModule)?;
    for (index, name) in wasm::SCALAR_ALGEBRA_COMPONENT_CANONICAL_EXPORTS_V5
        .into_iter()
        .enumerate()
    {
        exports.expect_name(name, PrivateComponentValidationError::CoreModule)?;
        exports.expect_bytes(
            &[0x00, 6 + index as u8],
            PrivateComponentValidationError::CoreModule,
        )?;
    }
    exports.finish(PrivateComponentValidationError::CoreModule)?;
    if module.section(10)?.is_empty() || module.section(11)?.is_empty() {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let mut custom = Cursor::new(module.section(0)?);
    custom.expect_name(
        "semaprax.component-scalar-algebra-v5",
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
    for _ in 0..6 {
        custom.take(32)?;
    }
    custom.finish(PrivateComponentValidationError::CoreModule)?;
    module.finish(PrivateComponentValidationError::CoreModule)?;
    Ok(revision)
}

struct ProfileEvidence {
    source_revision: String,
    prelude_digest: [u8; 32],
    layout_digests: [[u8; 32]; 6],
}

fn profile_evidence(program: &Program) -> Result<ProfileEvidence, Diagnostic> {
    let expected = crate::parse(
        crate::wasm::SCALAR_ALGEBRA_COMPONENT_SOURCE_V5,
        std::path::Path::new("scalar-algebra-v5-profile.spx"),
    )?;
    if crate::format::canonical(program) != crate::format::canonical(&expected) {
        return Err(profile_error(
            "scalar-algebra component requires the exact frozen source semantics",
        ));
    }
    let resolved = hir::resolve(program).map_err(first_error)?;
    hir::validate(&resolved)?;
    let ids = FUNCTION_EXPORTS.map(|name| DeclarationId::new(format!("component.{name}")));
    let main_id = DeclarationId::new("app.main");
    let expected_ids = ids.iter().chain(std::iter::once(&main_id));
    if !resolved.permits.is_empty()
        || !resolved.interfaces.is_empty()
        || resolved.functions.len() != 7
        || resolved
            .functions
            .iter()
            .map(|function| &function.id)
            .ne(expected_ids)
        || resolved
            .functions
            .iter()
            .any(|function| !function.effects.is_empty())
        || resolved.types.iter().any(|declaration| {
            !resolved
                .declarations
                .declaration(&declaration.id)
                .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned)
        })
    {
        return Err(profile_error(
            "scalar-algebra component requires its exact capability-free function table",
        ));
    }
    let mut layout_digests = [[0_u8; 32]; 6];
    for index in 0..6 {
        let ty = if index < 2 {
            ResolvedType::Nominal {
                declaration: DeclarationId::new(prelude::OPTION_ID),
                arguments: vec![SHAPES[index][0].clone()],
            }
        } else {
            ResolvedType::Nominal {
                declaration: DeclarationId::new(prelude::RESULT_ID),
                arguments: SHAPES[index].to_vec(),
            }
        };
        let function = &resolved.functions[index];
        if function.params.len() != 3
            || function.params[0].ty != ResolvedType::I64
            || function.params[1].ty != ResolvedType::Bool
            || function.params[2].ty != ResolvedType::I64
            || function.return_type != ty
        {
            return Err(profile_error(
                "scalar-algebra component export signature changed",
            ));
        }
        let layout = VariantLayout::for_type(&resolved, AggregateTarget::Wasm32, &ty)?;
        layout.validate(&resolved)?;
        layout_digests[index] = layout.digest();
    }
    Ok(ProfileEvidence {
        source_revision: crate::graph::revision(program),
        prelude_digest: prelude::digest_v1(),
        layout_digests,
    })
}

fn profile_digest(evidence: &ProfileEvidence) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PROFILE_DIGEST_DOMAIN);
    for field in [WIT_V5.as_bytes(), PROFILE, prelude::SCHEMA_V1.as_bytes()] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field);
    }
    hash.update(evidence.prelude_digest);
    for digest in evidence.layout_digests {
        hash.update(digest);
    }
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
        .unwrap_or_else(|| profile_error("scalar-algebra component HIR resolution failed"))
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-WIT108", message.into())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn artifact() -> PrivateScalarAlgebraComponentArtifactV5 {
        let program = crate::parse(
            crate::wasm::SCALAR_ALGEBRA_COMPONENT_SOURCE_V5,
            Path::new("component-scalar-algebra-v5.spx"),
        )
        .unwrap();
        emit_private_scalar_algebra_component_v5(&program).unwrap()
    }

    #[test]
    fn deterministic_v5_artifact_is_exactly_parsed_and_upstream_valid() {
        let first = artifact();
        assert_eq!(first.source_revision(), SOURCE_REVISION_KAT);
        assert_eq!(first.generated_core_digest(), GENERATED_CORE_KAT);
        assert_eq!(
            first.profile_digest(),
            [
                0xb4, 0x9d, 0x24, 0xae, 0x10, 0x0c, 0xf8, 0x3b, 0x49, 0xd8, 0xbb, 0x91, 0x46, 0x91,
                0x54, 0x35, 0x78, 0xa4, 0x29, 0x7e, 0x16, 0xb4, 0xfd, 0xd1, 0x97, 0xb8, 0xb7, 0x88,
                0xa6, 0x6c, 0x95, 0xf1,
            ]
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(first.bytes())),
            [
                0x6c, 0xeb, 0x9e, 0x30, 0x96, 0x94, 0xa5, 0xb9, 0x60, 0x94, 0x49, 0x58, 0xa4, 0xb0,
                0x52, 0x7e, 0x29, 0xef, 0xa6, 0xba, 0xe8, 0xf7, 0xfc, 0x27, 0xe9, 0x4a, 0xd0, 0x1a,
                0x84, 0x7b, 0xad, 0xca,
            ]
        );
        assert_eq!(
            first.digest(),
            [
                0x3f, 0x7c, 0xd7, 0x6b, 0xe5, 0x5f, 0x8f, 0x5f, 0x49, 0x88, 0x4b, 0xc0, 0x63, 0xb9,
                0xca, 0x1c, 0x7a, 0x97, 0xb1, 0xe2, 0xc3, 0x8e, 0x23, 0x5c, 0xf4, 0x02, 0x39, 0x53,
                0xca, 0x36, 0xbd, 0xcf,
            ]
        );
        assert_eq!(first, artifact());
        assert_eq!(first.wit(), WIT_V5);
        assert_ne!(first.layout_digests()[0], first.layout_digests()[1]);
        let validated = validate_private_scalar_algebra_component_v5(
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
            first.generated_core_digest()
        );
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(first.bytes())
            .expect("pinned upstream validator rejected scalar-algebra component v5");
    }

    #[test]
    fn every_byte_truncation_trailing_and_noncanonical_length_reject() {
        let artifact = artifact();
        for index in 0..artifact.bytes().len() {
            let mut hostile = artifact.bytes().to_vec();
            hostile[index] ^= 1;
            assert!(validate_private_scalar_algebra_component_v5(
                &hostile,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
        }
        for end in 0..artifact.bytes().len() {
            assert!(validate_private_scalar_algebra_component_v5(
                &artifact.bytes()[..end],
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
        }
        let mut trailing = artifact.bytes().to_vec();
        trailing.push(0);
        assert!(validate_private_scalar_algebra_component_v5(
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
        assert_eq!(offsets.len(), 1, "v5 core-instance anchor drifted");
        noncanonical.splice(offsets[0] + 1..offsets[0] + 2, [0x84, 0x00]);
        assert_eq!(
            validate_private_scalar_algebra_component_v5(
                &noncanonical,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            ),
            Err(PrivateComponentValidationError::Encoding)
        );
    }

    #[test]
    fn same_signature_function_and_type_reindexing_rejects() {
        let artifact = artifact();
        let canonical = {
            let mut bytes = vec![0x06];
            for index in 0_u8..6 {
                bytes.extend([0x00, 0x00, index, 0x02, 0x00, 0x03, 0x00, 14 + index]);
            }
            bytes
        };
        let canonical_at = artifact
            .bytes()
            .windows(canonical.len())
            .position(|window| window == canonical)
            .expect("canonical section anchor drifted");
        for (left, right) in [(1_usize, 5_usize), (3, 4)] {
            let mut hostile = artifact.bytes().to_vec();
            hostile.swap(canonical_at + 3 + left * 8, canonical_at + 3 + right * 8);
            assert!(validate_private_scalar_algebra_component_v5(
                &hostile,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());

            let mut hostile = artifact.bytes().to_vec();
            hostile.swap(canonical_at + 8 + left * 8, canonical_at + 8 + right * 8);
            assert!(validate_private_scalar_algebra_component_v5(
                &hostile,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
        }
    }

    #[test]
    fn v1_v2_v3_v4_v5_profiles_are_never_confused() {
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
        let v5 = artifact();
        for candidate in [v1.bytes(), v2.bytes(), v3.bytes(), v4.bytes()] {
            assert!(validate_private_scalar_algebra_component_v5(
                candidate,
                v5.source_revision(),
                v5.generated_core_digest(),
            )
            .is_err());
        }
        assert!(super::super::validate_private_component_v1(v5.bytes()).is_err());
        assert!(super::super::validate_private_checked_component_v2(
            v5.bytes(),
            v2.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_result_component_v3(
            v5.bytes(),
            v3.source_revision(),
            v3.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_source_result_component_v4(
            v5.bytes(),
            v4.source_revision(),
            v4.generated_core_digest(),
        )
        .is_err());
    }
}
