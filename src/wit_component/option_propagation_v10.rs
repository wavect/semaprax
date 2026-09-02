//! Exact private source-`Option` propagation Component Model v10 composition.

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

const INTERFACE_EXPORT: &str = "semaprax:private/option-propagation@0.8.0";
const FUNCTION_EXPORT: &str = "evaluate";
const SOURCE_OPTION_EXPORT: &str = "source-option";
const TARGET_OPTION_EXPORT: &str = "target-option";

const WIT_V10: &str =
    include_str!("../../platform-tests/component-runtime/wit/semaprax-private-v10.wit");

const PROFILE: &[u8] = b"semaprax.private-option-propagation-component.v10\0canonical-abi-memory32-utf8\0source-option-i64-target-option-bool\0status-first-carrier-never-flattened\0input-output-tag-bool-and-unknown-status-trap\0canonical-result-area-256-size20-align4\0outer-payload-offset4-inner-tag-offset4-inner-bool-offset5\0compiler-option-layout-v2-field-reconstruction\0graph-v11\0cleanup-plan-v3\0zero-import-empty-linker-no-wasi\0";
const PROFILE_DIGEST_DOMAIN: &[u8] = b"semaprax.private-option-propagation-component-profile.v10\0";
const COMPONENT_DIGEST_DOMAIN: &[u8] =
    b"semaprax.private-option-propagation-component-artifact.v10\0";
const SOURCE_REVISION_KAT: &str =
    "sha256:98b8fc892c183499153142d5bbdb4162e31bda95ef145d34dbb1ff57c9b8fc72";
const GENERATED_CORE_KAT: [u8; 32] = [
    0x16, 0xd1, 0xd3, 0x40, 0x24, 0xe3, 0xfa, 0xd9, 0x20, 0xd8, 0xd0, 0x0a, 0x61, 0xd7, 0xcb, 0x3b,
    0xd0, 0x10, 0x33, 0x5c, 0xa3, 0x82, 0xf2, 0x36, 0x15, 0xb3, 0xb3, 0xda, 0x41, 0x43, 0xaa, 0xec,
];

/// Compiler-bound, import-free private Component Model artifact for the exact
/// source-language `Option<i64>` to `Option<bool>` propagation projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateOptionPropagationComponentArtifactV10 {
    bytes: Vec<u8>,
    digest: [u8; 32],
    generated_core_digest: [u8; 32],
    profile_digest: [u8; 32],
    graph_digest: [u8; 32],
    prelude_digest: [u8; 32],
    option_i64_layout_digest: [u8; 32],
    option_bool_layout_digest: [u8; 32],
    plan_digest: [u8; 32],
    source_revision: String,
}

impl PrivateOptionPropagationComponentArtifactV10 {
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
    pub const fn prelude_digest(&self) -> [u8; 32] {
        self.prelude_digest
    }

    #[must_use]
    pub const fn option_i64_layout_digest(&self) -> [u8; 32] {
        self.option_i64_layout_digest
    }

    #[must_use]
    pub const fn option_bool_layout_digest(&self) -> [u8; 32] {
        self.option_bool_layout_digest
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
        WIT_V10
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPrivateOptionPropagationComponentV10<'a> {
    generated_core: &'a [u8],
    source_revision: &'a str,
}

impl<'a> ValidatedPrivateOptionPropagationComponentV10<'a> {
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
    pub const fn source_option_export_name(self) -> &'static str {
        SOURCE_OPTION_EXPORT
    }

    #[must_use]
    pub const fn target_option_export_name(self) -> &'static str {
        TARGET_OPTION_EXPORT
    }
}

pub fn emit_private_option_propagation_component_v10(
    program: &Program,
) -> Result<PrivateOptionPropagationComponentArtifactV10, Diagnostic> {
    let evidence = profile_evidence(program)?;
    let core = wasm::emit_private_option_propagation_core_v10(program)?;
    if core.source_revision != evidence.source_revision
        || core.graph_digest != evidence.graph_digest
        || core.prelude_digest != evidence.prelude_digest
        || core.option_i64_layout_digest != evidence.option_i64_layout_digest
        || core.option_bool_layout_digest != evidence.option_bool_layout_digest
        || core.plan_digest != evidence.plan_digest
    {
        return Err(profile_error(
            "option-propagation core bindings disagree with independently admitted source meaning",
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
    Ok(PrivateOptionPropagationComponentArtifactV10 {
        bytes,
        digest,
        generated_core_digest,
        profile_digest,
        graph_digest: evidence.graph_digest,
        prelude_digest: evidence.prelude_digest,
        option_i64_layout_digest: evidence.option_i64_layout_digest,
        option_bool_layout_digest: evidence.option_bool_layout_digest,
        plan_digest: evidence.plan_digest,
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
        wasm::OPTION_PROPAGATION_COMPONENT_CANONICAL_EXPORT_V10,
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

    let mut interface = vec![0x01, 0x04]; // from-exports, four exports
    interface.push(0x00);
    push_name(&mut interface, "status");
    interface.extend([0x03, 0x02]); // component type 2
    interface.push(0x00);
    push_name(&mut interface, SOURCE_OPTION_EXPORT);
    interface.extend([0x03, 0x00]); // component type 0
    interface.push(0x00);
    push_name(&mut interface, TARGET_OPTION_EXPORT);
    interface.extend([0x03, 0x01]); // component type 1
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
    // type 0: source-option = option<s64>
    types.extend([0x6b, 0x78]);
    // type 1: target-option = option<bool>
    types.extend([0x6b, 0x7f]);
    // type 2: record status
    types.extend([0x72, 0x04]);
    push_name(&mut types, "domain");
    types.push(0x73);
    push_name(&mut types, "code");
    types.push(0x79);
    push_name(&mut types, "class");
    types.push(0x7d);
    push_name(&mut types, "retryable");
    types.push(0x01);
    // type 3: result<target-option, status>
    types.extend([0x6a, 0x01, 0x01, 0x01, 0x02]);
    // type 4: evaluate(input: source-option, divisor: s64) -> type 3
    types.extend([0x40, 0x02]);
    push_name(&mut types, "input");
    types.push(0x00);
    push_name(&mut types, "divisor");
    types.extend([0x78, 0x00, 0x03]);
    types
}

pub fn validate_private_option_propagation_component_v10<'a>(
    candidate: &'a [u8],
    expected_source_revision: &str,
    expected_generated_core_digest: [u8; 32],
) -> Result<ValidatedPrivateOptionPropagationComponentV10<'a>, PrivateComponentValidationError> {
    if expected_source_revision != SOURCE_REVISION_KAT
        || expected_generated_core_digest != GENERATED_CORE_KAT
    {
        return Err(PrivateComponentValidationError::Profile);
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
        wasm::OPTION_PROPAGATION_COMPONENT_CANONICAL_EXPORT_V10,
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
    interface.expect_u32(4, PrivateComponentValidationError::Profile)?;
    for (name, index) in [
        ("status", 2_u8),
        (SOURCE_OPTION_EXPORT, 0_u8),
        (TARGET_OPTION_EXPORT, 1_u8),
    ] {
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

    Ok(ValidatedPrivateOptionPropagationComponentV10 {
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
            0x03, 0x60, 0x03, 0x7f, 0x7e, 0x7f, 0x01, 0x7f, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
            0x60, 0x03, 0x7f, 0x7e, 0x7e, 0x01, 0x7f,
        ],
        PrivateComponentValidationError::CoreModule,
    )?;
    super::validate_exact_payload(
        module.section(3)?,
        &[0x04, 0x00, 0x01, 0x00, 0x02],
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
        wasm::OPTION_PROPAGATION_COMPONENT_STATUS_OUT_EXPORT_V10,
        PrivateComponentValidationError::CoreModule,
    )?;
    exports.expect_bytes(&[0x00, 0x02], PrivateComponentValidationError::CoreModule)?;
    exports.expect_name(
        wasm::OPTION_PROPAGATION_COMPONENT_CANONICAL_EXPORT_V10,
        PrivateComponentValidationError::CoreModule,
    )?;
    exports.expect_bytes(&[0x00, 0x03], PrivateComponentValidationError::CoreModule)?;
    exports.finish(PrivateComponentValidationError::CoreModule)?;
    if module.section(10)?.is_empty() || module.section(11)?.is_empty() {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let mut custom = Cursor::new(module.section(0)?);
    custom.expect_name(
        "semaprax.component-option-propagation-v10",
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
    custom.take(32)?; // Graph v11 digest
    custom.take(32)?; // compiler-owned prelude digest
    custom.take(32)?; // Option<i64> Wasm32 layout-v2 digest
    custom.take(32)?; // Option<bool> Wasm32 layout-v2 digest
    custom.take(32)?; // CleanupPlan v3 digest
    custom.finish(PrivateComponentValidationError::CoreModule)?;
    module.finish(PrivateComponentValidationError::CoreModule)?;
    Ok(revision)
}

struct ProfileEvidence {
    source_revision: String,
    graph_digest: [u8; 32],
    prelude_digest: [u8; 32],
    option_i64_layout_digest: [u8; 32],
    option_bool_layout_digest: [u8; 32],
    plan_digest: [u8; 32],
}

fn profile_evidence(program: &Program) -> Result<ProfileEvidence, Diagnostic> {
    let expected = crate::parse(
        wasm::OPTION_PROPAGATION_SOURCE_V10,
        std::path::Path::new("option-propagation-v10-component-profile.spx"),
    )?;
    if crate::format::canonical(program) != crate::format::canonical(&expected) {
        return Err(profile_error(
            "option-propagation component requires exact frozen source",
        ));
    }
    let resolved = hir::resolve(program).map_err(first_error)?;
    hir::validate(&resolved)?;
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id == DeclarationId::new("component.option-propagation.evaluate"))
        .ok_or_else(|| {
            profile_error("option-propagation component requires exact evaluate identity")
        })?;
    let option_i64 = option_type(ResolvedType::I64);
    let option_bool = option_type(ResolvedType::Bool);
    if !resolved.permits.is_empty()
        || !resolved.interfaces.is_empty()
        || !resolved.function_templates.is_empty()
        || !resolved.function_instances.is_empty()
        || resolved.functions.len() != 2
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
        || function.params.len() != 2
        || function.params[0].ty != option_i64
        || function.params[1].ty != ResolvedType::I64
        || function.return_type != option_bool
        || function.cleanup_plan.schema != crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V3
    {
        return Err(profile_error(
            "option-propagation component requires exact capability-free Option propagation and CleanupPlan v3",
        ));
    }
    for id in [
        prelude::OPTION_ID,
        prelude::OPTION_NONE_ID,
        prelude::OPTION_SOME_ID,
        prelude::OPTION_SOME_VALUE_ID,
    ] {
        if !resolved
            .declarations
            .declaration(&DeclarationId::new(id))
            .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned)
        {
            return Err(profile_error(
                "option-propagation component does not authenticate the compiler-owned Option prelude",
            ));
        }
    }
    let i64_layout = VariantLayout::for_type(&resolved, AggregateTarget::Wasm32, &option_i64)?;
    let bool_layout = VariantLayout::for_type(&resolved, AggregateTarget::Wasm32, &option_bool)?;
    i64_layout.validate(&resolved)?;
    bool_layout.validate(&resolved)?;
    let graph_json = crate::graph::to_json(program).map_err(first_error)?;
    if !graph_json.starts_with("{\"schema\":\"semaprax.graph.v11\",") {
        return Err(profile_error(
            "option-propagation component requires exact Graph v11",
        ));
    }
    let graph_digest = Sha256::digest(graph_json.as_bytes()).into();
    let plan_json = crate::graph_cleanup::cleanup_plan_json(&function.cleanup_plan);
    Ok(ProfileEvidence {
        source_revision: crate::graph::revision(program),
        graph_digest,
        prelude_digest: prelude::digest_v1(),
        option_i64_layout_digest: i64_layout.digest(),
        option_bool_layout_digest: bool_layout.digest(),
        plan_digest: plan_digest(&plan_json),
    })
}

fn option_type(argument: ResolvedType) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(prelude::OPTION_ID),
        arguments: vec![argument],
    }
}

fn plan_digest(plan_json: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"semaprax.component-option-propagation-plan.v10\0");
    hash.update((plan_json.len() as u64).to_le_bytes());
    hash.update(plan_json.as_bytes());
    hash.finalize().into()
}

fn profile_digest(evidence: &ProfileEvidence) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PROFILE_DIGEST_DOMAIN);
    for field in [
        WIT_V10.as_bytes(),
        PROFILE,
        prelude::SCHEMA_V1.as_bytes(),
        prelude::OPTION_ID.as_bytes(),
        prelude::OPTION_NONE_ID.as_bytes(),
        prelude::OPTION_SOME_ID.as_bytes(),
        prelude::OPTION_SOME_VALUE_ID.as_bytes(),
    ] {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field);
    }
    hash.update(evidence.graph_digest);
    hash.update(evidence.prelude_digest);
    hash.update(evidence.option_i64_layout_digest);
    hash.update(evidence.option_bool_layout_digest);
    hash.update(evidence.plan_digest);
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
        .unwrap_or_else(|| profile_error("option-propagation component HIR resolution failed"))
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-WIT108", message.into())
}

#[cfg(test)]
#[path = "option_propagation_v10/tests.rs"]
mod tests;
