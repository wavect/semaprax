//! Exact private generic-function-instance Component Model v9 composition.

use sha2::{Digest, Sha256};

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, DeclarationId, FunctionInstanceId, IdentityOrigin, ResolvedType};
use crate::wasm;

use super::{
    push_counted_section, push_name, push_section, Cursor, PrivateComponentValidationError,
    COMPONENT_HEADER,
};

const INTERFACE_EXPORT: &str = "semaprax:private/generic-function-instances@0.7.0";
const FUNCTION_EXPORTS: [&str; 6] = [
    "preserve-i64",
    "invert-i64",
    "preserve-bool",
    "invert-bool",
    "ordered-i64-bool",
    "ordered-bool-i64",
];
const TYPE_EXPORTS: [&str; 1] = ["status"];
const TEMPLATE_IDS: [&str; 3] = [
    "component.generic-function.preserve",
    "component.generic-function.invert",
    "component.generic-function.ordered",
];
const WIT_V9: &str = "package semaprax:private@0.7.0;\n\ninterface generic-function-instances {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  preserve-i64: func(marker: bool, control: s64) -> result<bool, status>;\n  invert-i64: func(marker: bool, control: s64) -> result<bool, status>;\n  preserve-bool: func(marker: bool, control: s64) -> result<bool, status>;\n  invert-bool: func(marker: bool, control: s64) -> result<bool, status>;\n  ordered-i64-bool: func(marker: bool, control: s64) -> result<bool, status>;\n  ordered-bool-i64: func(marker: bool, control: s64) -> result<bool, status>;\n}\n\nworld semaprax-private-v9 {\n  export generic-function-instances;\n}\n";

const PROFILE: &[u8] = b"semaprax.private-generic-function-component.v9\0canonical-abi-memory32-utf8\0six-exact-graph-v14-function-instances\0three-phantom-copy-templates\0one-shared-function-type\0no-record-or-layout-roots\0status-before-output\0tag-last\0";
const PROFILE_DOMAIN: &[u8] = b"semaprax.private-generic-function-component-profile.v9\0";
const ARTIFACT_DOMAIN: &[u8] = b"semaprax.private-generic-function-component-artifact.v9\0";
const PLAN_DOMAIN: &[u8] = b"semaprax.component-generic-function-plan.v9\0";

const SOURCE_REVISION_KAT: &str =
    "sha256:218085fb5ea1bcc090c04ac0acb3395912d0dad09027b9118d8817978b2fde0c";
const GENERATED_CORE_KAT: [u8; 32] = [
    0x9f, 0x17, 0x82, 0x07, 0xa0, 0x40, 0x6f, 0x74, 0x01, 0x98, 0xee, 0x8c, 0x71, 0xd5, 0xd0, 0x08,
    0xef, 0xdf, 0x4d, 0x99, 0x5f, 0xf0, 0x4e, 0x11, 0xe8, 0x0e, 0xa7, 0x3b, 0x79, 0x15, 0x5d, 0x44,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateGenericFunctionComponentArtifactV9 {
    bytes: Vec<u8>,
    digest: [u8; 32],
    generated_core_digest: [u8; 32],
    profile_digest: [u8; 32],
    graph_digest: [u8; 32],
    plan_digest: [u8; 32],
    source_revision: String,
}

impl PrivateGenericFunctionComponentArtifactV9 {
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
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }
    #[must_use]
    pub const fn wit(&self) -> &'static str {
        WIT_V9
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPrivateGenericFunctionComponentV9<'a> {
    core: &'a [u8],
    source_revision: &'a str,
}

impl<'a> ValidatedPrivateGenericFunctionComponentV9<'a> {
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
    pub const fn function_export_names(self) -> [&'static str; 6] {
        FUNCTION_EXPORTS
    }
    #[must_use]
    pub const fn type_export_names(self) -> [&'static str; 1] {
        TYPE_EXPORTS
    }
}

pub fn emit_private_generic_function_component_v9(
    program: &Program,
) -> Result<PrivateGenericFunctionComponentArtifactV9, Diagnostic> {
    let evidence = profile_evidence(program)?;
    let core = wasm::emit_private_generic_function_core_v9(program)?;
    if core.source_revision != evidence.source_revision
        || core.graph_digest != evidence.graph_digest
        || core.plan_digest != evidence.plan_digest
    {
        return Err(profile_error(
            "generic-function core disagrees with independent profile evidence",
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
    Ok(PrivateGenericFunctionComponentArtifactV9 {
        bytes,
        digest,
        generated_core_digest,
        profile_digest,
        graph_digest: evidence.graph_digest,
        plan_digest: evidence.plan_digest,
        source_revision: evidence.source_revision,
    })
}

fn compose(core: &[u8]) -> Vec<u8> {
    let mut bytes = COMPONENT_HEADER.to_vec();
    push_section(&mut bytes, 1, core);
    push_counted_section(&mut bytes, 2, 1, &[0x00, 0x00, 0x00]);
    let mut aliases = Vec::new();
    for name in wasm::GENERIC_FUNCTION_COMPONENT_CANONICAL_EXPORTS_V9 {
        aliases.extend([0x00, 0x00, 0x01, 0x00]);
        push_name(&mut aliases, name);
    }
    aliases.extend([0x00, 0x02, 0x01, 0x00]);
    push_name(&mut aliases, "memory");
    push_counted_section(&mut bytes, 6, 7, &aliases);
    push_section(&mut bytes, 7, &component_types());
    let mut canonical = Vec::new();
    for index in 0_u8..6 {
        canonical.extend([0x00, 0x00, index, 0x02, 0x00, 0x03, 0x00, 0x03]);
    }
    push_counted_section(&mut bytes, 8, 6, &canonical);
    let mut interface = vec![0x01, 0x07];
    interface.push(0x00);
    push_name(&mut interface, TYPE_EXPORTS[0]);
    interface.extend([0x03, 0x01]);
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
    let mut types = vec![0x04, 0x6b, 0x7f, 0x72, 0x04];
    push_name(&mut types, "domain");
    types.push(0x73);
    push_name(&mut types, "code");
    types.push(0x79);
    push_name(&mut types, "class");
    types.push(0x7d);
    push_name(&mut types, "retryable");
    types.push(0x00);
    types.extend([0x6a, 0x01, 0x7f, 0x01, 0x01]);
    types.extend([0x40, 0x02]);
    push_name(&mut types, "marker");
    types.push(0x7f);
    push_name(&mut types, "control");
    types.extend([0x78, 0x00, 0x02]);
    types
}

pub fn validate_private_generic_function_component_v9<'a>(
    candidate: &'a [u8],
    expected_source_revision: &str,
    expected_core_digest: [u8; 32],
) -> Result<ValidatedPrivateGenericFunctionComponentV9<'a>, PrivateComponentValidationError> {
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
    aliases.expect_u32(7, PrivateComponentValidationError::Profile)?;
    for name in wasm::GENERIC_FUNCTION_COMPONENT_CANONICAL_EXPORTS_V9 {
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
        expected_canonical.extend([0x00, 0x00, index, 0x02, 0x00, 0x03, 0x00, 0x03]);
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
    interface.expect_u32(7, PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
    interface.expect_name(TYPE_EXPORTS[0], PrivateComponentValidationError::Profile)?;
    interface.expect_bytes(&[0x03, 0x01], PrivateComponentValidationError::Profile)?;
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
    Ok(ValidatedPrivateGenericFunctionComponentV9 {
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
    exports.expect_u32(7, PrivateComponentValidationError::CoreModule)?;
    exports.expect_name("memory", PrivateComponentValidationError::CoreModule)?;
    exports.expect_bytes(&[0x02, 0x00], PrivateComponentValidationError::CoreModule)?;
    for (index, name) in wasm::GENERIC_FUNCTION_COMPONENT_CANONICAL_EXPORTS_V9
        .into_iter()
        .enumerate()
    {
        exports.expect_name(name, PrivateComponentValidationError::CoreModule)?;
        exports.expect_bytes(
            &[0x00, 0x06 + index as u8],
            PrivateComponentValidationError::CoreModule,
        )?;
    }
    exports.finish(PrivateComponentValidationError::CoreModule)?;
    if module.section(10)?.is_empty() || module.section(11)?.is_empty() {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let mut custom = Cursor::new(module.section(0)?);
    custom.expect_name(
        "semaprax.component-generic-function-v9",
        PrivateComponentValidationError::CoreModule,
    )?;
    let len =
        usize::try_from(custom.u32()?).map_err(|_| PrivateComponentValidationError::Encoding)?;
    let revision = std::str::from_utf8(custom.take(len)?)
        .map_err(|_| PrivateComponentValidationError::CoreModule)?;
    if revision != expected_revision {
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
    graph_digest: [u8; 32],
    plan_digest: [u8; 32],
}

fn profile_evidence(program: &Program) -> Result<ProfileEvidence, Diagnostic> {
    let expected = crate::parse(
        wasm::GENERIC_FUNCTION_COMPONENT_SOURCE_V9,
        std::path::Path::new("generic-function-v9-component-profile.spx"),
    )?;
    if crate::format::canonical(program) != crate::format::canonical(&expected) {
        return Err(profile_error(
            "generic-function component requires exact frozen source",
        ));
    }
    let resolved = hir::resolve(program).map_err(first_error)?;
    hir::validate(&resolved)?;
    if resolved.function_templates.len() != 3
        || resolved.function_instances.len() != 6
        || resolved.functions.len() != 2
        || resolved.types.iter().any(|declaration| {
            !resolved
                .declarations
                .declaration(&declaration.id)
                .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned)
        })
    {
        return Err(profile_error(
            "generic-function component HIR cardinality or no-layout boundary changed",
        ));
    }
    let expected_instances = instance_specs();
    for (actual, (template, arguments, id)) in resolved
        .function_instances
        .iter()
        .zip(expected_instances.iter())
    {
        if &actual.template != template
            || &actual.type_arguments != arguments
            || &actual.id != id
            || actual.function.params.len() != 2
            || actual.function.params[0].ty != ResolvedType::Bool
            || actual.function.params[1].ty != ResolvedType::I64
            || actual.function.return_type != ResolvedType::Bool
        {
            return Err(profile_error(
                "generic-function component exact FunctionInstanceId map changed",
            ));
        }
    }
    let graph_json = crate::graph::to_json(program).map_err(first_error)?;
    if !graph_json.starts_with("{\"schema\":\"semaprax.graph.v14\",") {
        return Err(profile_error(
            "generic-function component requires exact Graph v14",
        ));
    }
    let graph_digest = Sha256::digest(graph_json.as_bytes()).into();
    Ok(ProfileEvidence {
        source_revision: crate::graph::revision(program),
        graph_digest,
        plan_digest: plan_digest(&expected_instances),
    })
}

fn instance_specs() -> Vec<(DeclarationId, Vec<ResolvedType>, FunctionInstanceId)> {
    let specs = [
        (TEMPLATE_IDS[0], vec![ResolvedType::I64]),
        (TEMPLATE_IDS[1], vec![ResolvedType::I64]),
        (TEMPLATE_IDS[0], vec![ResolvedType::Bool]),
        (TEMPLATE_IDS[1], vec![ResolvedType::Bool]),
        (TEMPLATE_IDS[2], vec![ResolvedType::I64, ResolvedType::Bool]),
        (TEMPLATE_IDS[2], vec![ResolvedType::Bool, ResolvedType::I64]),
    ];
    specs
        .into_iter()
        .map(|(template, arguments)| {
            let template = DeclarationId::new(template);
            let id = FunctionInstanceId::derive(&template, &arguments);
            (template, arguments, id)
        })
        .collect()
}

fn plan_digest(instances: &[(DeclarationId, Vec<ResolvedType>, FunctionInstanceId)]) -> [u8; 32] {
    let internals = [144_i32, 208, 272, 336, 400, 464];
    let results = [160_i32, 224, 288, 352, 416, 480];
    let mut hash = Sha256::new();
    hash.update(PLAN_DOMAIN);
    for (index, (template, arguments, instance)) in instances.iter().enumerate() {
        for field in [
            template.as_str().as_bytes(),
            instance.as_str().as_bytes(),
            wasm::GENERIC_FUNCTION_COMPONENT_CANONICAL_EXPORTS_V9[index].as_bytes(),
        ] {
            hash.update((field.len() as u64).to_le_bytes());
            hash.update(field);
        }
        for argument in arguments {
            let key = argument.identity_key();
            hash.update((key.len() as u64).to_le_bytes());
            hash.update(key.as_bytes());
        }
        hash.update(internals[index].to_le_bytes());
        hash.update(results[index].to_le_bytes());
        hash.update([u8::from(index == 1 || index == 3)]);
        hash.update([index as u8]);
    }
    hash.finalize().into()
}

fn profile_digest(evidence: &ProfileEvidence) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PROFILE_DOMAIN);
    for field in [WIT_V9.as_bytes(), PROFILE] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field);
    }
    hash.update(evidence.graph_digest);
    hash.update(evidence.plan_digest);
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
        .unwrap_or_else(|| profile_error("generic-function component resolution failed"))
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-WIT110", message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn artifact() -> PrivateGenericFunctionComponentArtifactV9 {
        let program = crate::parse(
            wasm::GENERIC_FUNCTION_COMPONENT_SOURCE_V9,
            Path::new("component-generic-function-v9.spx"),
        )
        .unwrap();
        emit_private_generic_function_component_v9(&program).unwrap()
    }

    #[test]
    fn deterministic_v9_component_is_upstream_valid_and_kat_bound() {
        let first = artifact();
        assert_eq!(first, artifact());
        assert_eq!(first.source_revision(), SOURCE_REVISION_KAT);
        assert_eq!(first.wit(), WIT_V9);
        assert_eq!(
            first.graph_digest(),
            hex32("62907c4b95495bb573b2b37de9f0b08c7a82218934154521e8c0c8396158cc6e")
        );
        assert_eq!(first.generated_core_digest(), GENERATED_CORE_KAT);
        assert_eq!(
            first.profile_digest(),
            hex32("365897ddb2770cc25a11690dddbfef5d232244ec5d328c79a24a1410e684615e")
        );
        assert_eq!(
            first.plan_digest(),
            hex32("edd11c98bbc902d9dbc9c942375477fcf1e6c3f1befbe3c4a9f260107104485e")
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(first.bytes())),
            hex32("3cf6c7d7d02e838fb374478a2b5b25077c7c612ad36e30deaffd15311a25a688")
        );
        assert_eq!(
            first.digest(),
            hex32("2623ff9a7eda5526616a15befd4951de86874a59911dcba2a7d3bcc2d178a474")
        );
        let validated = validate_private_generic_function_component_v9(
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
            .expect("upstream validator rejected generic-function component v9");
    }

    #[test]
    fn every_byte_truncation_trailing_and_all_fifteen_swaps_reject() {
        let artifact = artifact();
        for index in 0..artifact.bytes().len() {
            let mut hostile = artifact.bytes().to_vec();
            hostile[index] ^= 1;
            assert!(validate_private_generic_function_component_v9(
                &hostile,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
        }
        for end in 0..artifact.bytes().len() {
            assert!(validate_private_generic_function_component_v9(
                &artifact.bytes()[..end],
                artifact.source_revision(),
                artifact.generated_core_digest(),
            )
            .is_err());
        }
        let mut trailing = artifact.bytes().to_vec();
        trailing.push(0);
        assert!(validate_private_generic_function_component_v9(
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
        assert_eq!(positions.len(), 1, "v9 core-instance anchor drifted");
        noncanonical.splice(positions[0] + 1..positions[0] + 2, [0x84, 0x00]);
        assert_eq!(
            validate_private_generic_function_component_v9(
                &noncanonical,
                artifact.source_revision(),
                artifact.generated_core_digest(),
            ),
            Err(PrivateComponentValidationError::Encoding)
        );

        let mut canonical_anchor = Vec::new();
        for index in 0_u8..6 {
            canonical_anchor.extend([0x00, 0x00, index, 0x02, 0x00, 0x03, 0x00, 0x03]);
        }
        let canonical_at = artifact
            .bytes()
            .windows(canonical_anchor.len())
            .position(|window| window == canonical_anchor)
            .expect("v9 canonical lift anchor drifted");
        let mut swaps = 0;
        for left in 0..6 {
            for right in left + 1..6 {
                let mut hostile = artifact.bytes().to_vec();
                hostile.swap(canonical_at + 2 + left * 8, canonical_at + 2 + right * 8);
                wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
                    .validate_all(&hostile)
                    .expect("same-signature v9 core-index swap should be structurally valid");
                assert!(validate_private_generic_function_component_v9(
                    &hostile,
                    artifact.source_revision(),
                    artifact.generated_core_digest(),
                )
                .is_err());
                swaps += 1;
            }
        }
        assert_eq!(swaps, 15);
    }

    #[test]
    fn independent_profile_mutations_reject() {
        for hostile in [
            wasm::GENERIC_FUNCTION_COMPONENT_SOURCE_V9.replacen(
                "preserve<i64>",
                "preserve<bool>",
                1,
            ),
            wasm::GENERIC_FUNCTION_COMPONENT_SOURCE_V9.replacen(
                "ordered<i64, bool>",
                "ordered<bool, i64>",
                1,
            ),
            wasm::GENERIC_FUNCTION_COMPONENT_SOURCE_V9.replacen(
                "fn ordered<T, U>",
                "fn ordered<U, T>",
                1,
            ),
        ] {
            let parsed = crate::parse(&hostile, Path::new("hostile-v9-profile.spx"));
            match parsed {
                Ok(program) => {
                    assert!(emit_private_generic_function_component_v9(&program).is_err())
                }
                Err(error) => assert!(!error.code.is_empty()),
            }
        }
    }

    #[test]
    fn v1_through_v9_profiles_are_never_confused() {
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
        let v8_program = crate::parse(
            wasm::RECORD_PATTERN_COMPONENT_SOURCE_V8,
            Path::new("v8.spx"),
        )
        .unwrap();
        let v8 = super::super::emit_private_record_pattern_component_v8(&v8_program).unwrap();
        let v9 = artifact();
        for candidate in [
            v1.bytes(),
            v2.bytes(),
            v3.bytes(),
            v4.bytes(),
            v5.bytes(),
            v6.bytes(),
            v7.bytes(),
            v8.bytes(),
        ] {
            assert!(validate_private_generic_function_component_v9(
                candidate,
                v9.source_revision(),
                v9.generated_core_digest(),
            )
            .is_err());
        }
        assert!(super::super::validate_private_component_v1(v9.bytes()).is_err());
        assert!(super::super::validate_private_checked_component_v2(
            v9.bytes(),
            v2.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_result_component_v3(
            v9.bytes(),
            v3.source_revision(),
            v3.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_source_result_component_v4(
            v9.bytes(),
            v4.source_revision(),
            v4.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_scalar_algebra_component_v5(
            v9.bytes(),
            v5.source_revision(),
            v5.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_nested_record_component_v6(
            v9.bytes(),
            v6.source_revision(),
            v6.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_generic_record_component_v7(
            v9.bytes(),
            v7.source_revision(),
            v7.generated_core_digest(),
        )
        .is_err());
        assert!(super::super::validate_private_record_pattern_component_v8(
            v9.bytes(),
            v8.source_revision(),
            v8.generated_core_digest(),
        )
        .is_err());
    }

    fn hex32(value: &str) -> [u8; 32] {
        assert_eq!(value.len(), 64);
        let mut bytes = [0_u8; 32];
        for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
            let nibble = |byte: u8| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => panic!("non-lowercase-hex KAT"),
            };
            bytes[index] = (nibble(chunk[0]) << 4) | nibble(chunk[1]);
        }
        bytes
    }
}
