//! Compiler-derived native host ownership contracts for the gated resource slice.
//!
//! This module is private groundwork. It derives every semantic identity and
//! ownership requirement from validated HIR plus the validated native resource
//! ABI. Private Stage B captures binding-instance authority and observes the
//! real Rust thread internally; compiler preflight creates Stage A only. Public
//! native resource lowering remains closed behind `SPX-B104`.

#![cfg_attr(
    not(test),
    allow(dead_code, reason = "native resource host entry points remain gated")
)]

use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::ThreadId;

use sha2::{Digest, Sha256};

use crate::conformance::NormalizedStatus;
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, OwnershipMode, ResolvedProgram, ResolvedResourceDropKind, ResolvedType,
    ResolvedTypeDeclarationKind, ValueId,
};
use crate::host_ownership::{
    HostBoundaryRejection, HostBoundaryResult, HostCallContract, HostCallRequest,
    HostCommittedResource, HostIdentity, HostOwnerToken, HostOwnershipRegistry,
    HostResourceRequirement, HostResultPlan,
};

use super::native_cleanup::NativeCleanupIndex;
use super::native_resource::{NativeFinalizerKind, NativeResourceAbi};
use super::native_value::{NativeValuePlan, NativeValueResult};

const MODULE_IDENTITY_DOMAIN: &str = "semaprax.native-host-module.v1";
const ADAPTER_IDENTITY_DOMAIN: &str = "semaprax.native-host-adapter.v1";
const FUNCTION_IDENTITY_DOMAIN: &str = "semaprax.native-host-function.v1";
const BOUND_FUNCTION_IDENTITY_DOMAIN: &str = "semaprax.native-host-bound-function.v1";
const RESOURCE_IDENTITY_DOMAIN: &str = "semaprax.native-host-resource.v1";
const LIFECYCLE_IDENTITY_DOMAIN: &str = "semaprax.native-host-lifecycle.v1";
const TEMPLATE_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.native-host-template.v1\0";
const MODULE_ABI_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.native-host-module-abi.v1\0";

static NEXT_ADAPTER_BINDING_INSTANCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeHostScalarKind {
    I64,
    Bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum NativeHostParameter {
    Scalar {
        parameter_index: usize,
        value_id: ValueId,
        kind: NativeHostScalarKind,
    },
    OwnedResource {
        parameter_index: usize,
        value_id: ValueId,
        owner_ordinal: usize,
        resource_type: HostIdentity,
        lifecycle: HostIdentity,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum NativeHostResult {
    ScalarI64,
    OwnedInput {
        parameter_index: usize,
        value_id: ValueId,
        owner_ordinal: usize,
    },
}

/// Adapter-independent compiler proof for one exact exported function shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeHostContractTemplate {
    semantic_module: String,
    module: HostIdentity,
    module_abi_fingerprint: String,
    semantic_function: String,
    function: HostIdentity,
    parameters: Vec<NativeHostParameter>,
    result: NativeHostResult,
    fingerprint: String,
}

/// Read-only semantic projection consumed by the private physical descriptor
/// emitter.  The projection has no public constructor: only an already
/// admitted host template can create it, so descriptor emission cannot accept
/// foreign signature metadata or independently reclassify HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeAdapterTemplateProjection {
    pub(super) module: String,
    pub(super) module_abi_fingerprint: String,
    pub(super) function: String,
    pub(super) parameters: Vec<NativeAdapterParameterProjection>,
    pub(super) result: NativeAdapterResultProjection,
    pub(super) function_template_fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum NativeAdapterParameterProjection {
    Scalar {
        parameter_index: usize,
        value_id: ValueId,
        kind: NativeHostScalarKind,
    },
    OwnedResource {
        parameter_index: usize,
        value_id: ValueId,
        owner_ordinal: usize,
        resource_type: String,
        lifecycle: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum NativeAdapterResultProjection {
    ScalarI64,
    OwnedInput {
        parameter_index: usize,
        value_id: ValueId,
        owner_ordinal: usize,
    },
}

pub(super) fn project_for_adapter_abi(
    template: &NativeHostContractTemplate,
) -> NativeAdapterTemplateProjection {
    NativeAdapterTemplateProjection {
        module: template.module.as_str().to_owned(),
        module_abi_fingerprint: template.module_abi_fingerprint.clone(),
        function: template.function.as_str().to_owned(),
        parameters: template
            .parameters
            .iter()
            .map(|parameter| match parameter {
                NativeHostParameter::Scalar {
                    parameter_index,
                    value_id,
                    kind,
                } => NativeAdapterParameterProjection::Scalar {
                    parameter_index: *parameter_index,
                    value_id: value_id.clone(),
                    kind: *kind,
                },
                NativeHostParameter::OwnedResource {
                    parameter_index,
                    value_id,
                    owner_ordinal,
                    resource_type,
                    lifecycle,
                } => NativeAdapterParameterProjection::OwnedResource {
                    parameter_index: *parameter_index,
                    value_id: value_id.clone(),
                    owner_ordinal: *owner_ordinal,
                    resource_type: resource_type.as_str().to_owned(),
                    lifecycle: lifecycle.as_str().to_owned(),
                },
            })
            .collect(),
        result: match &template.result {
            NativeHostResult::ScalarI64 => NativeAdapterResultProjection::ScalarI64,
            NativeHostResult::OwnedInput {
                parameter_index,
                value_id,
                owner_ordinal,
            } => NativeAdapterResultProjection::OwnedInput {
                parameter_index: *parameter_index,
                value_id: value_id.clone(),
                owner_ordinal: *owner_ordinal,
            },
        },
        function_template_fingerprint: template.fingerprint.clone(),
    }
}

pub(super) fn project_for_callable_abi(
    template: &NativeHostContractTemplate,
) -> NativeAdapterTemplateProjection {
    let mut projection = project_for_adapter_abi(template);
    projection.module.clone_from(&template.semantic_module);
    projection.function.clone_from(&template.semantic_function);
    projection
}

/// Validated binding policy for the current import-free trivial-resource host.
///
/// There is no caller-provided adapter identity. Each trusted binding instance
/// receives a binding-instance-distinct process-local identity scoped to its
/// validated module ABI.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct NativeHostAdapterBinding {
    module_abi_fingerprint: String,
    identity: HostIdentity,
    bound_thread: ThreadId,
    host_thread_identity: u64,
}

/// Stage-B authority retained through synchronous registry execution. A bare
/// logical request is never handed to an adapter that could assert its own
/// thread or move a validated request elsewhere.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct NativeBoundHostContract {
    contract: HostCallContract,
    bound_thread: ThreadId,
    host_thread_identity: u64,
}

impl NativeBoundHostContract {
    pub(super) fn execute_scalar<F>(
        &self,
        registry: &mut HostOwnershipRegistry,
        owners: Vec<HostOwnerToken>,
        execute: F,
    ) -> HostBoundaryResult
    where
        F: FnOnce(&[HostCommittedResource]) -> Result<i64, NormalizedStatus>,
    {
        if std::thread::current().id() != self.bound_thread {
            return HostBoundaryResult::Rejected(HostBoundaryRejection::WrongThread);
        }
        registry.execute_scalar(
            HostCallRequest::new(self.contract.clone(), self.host_thread_identity, owners),
            execute,
        )
    }

    pub(super) fn execute_owned<F>(
        &self,
        registry: &mut HostOwnershipRegistry,
        owners: Vec<HostOwnerToken>,
        execute: F,
    ) -> HostBoundaryResult
    where
        F: FnOnce(&[HostCommittedResource]) -> Result<(), NormalizedStatus>,
    {
        if std::thread::current().id() != self.bound_thread {
            return HostBoundaryResult::Rejected(HostBoundaryRejection::WrongThread);
        }
        registry.execute_owned(
            HostCallRequest::new(self.contract.clone(), self.host_thread_identity, owners),
            execute,
        )
    }
}

impl NativeHostAdapterBinding {
    pub(super) fn for_current_thread(
        template: &NativeHostContractTemplate,
    ) -> Result<Self, Diagnostic> {
        let instance = NEXT_ADAPTER_BINDING_INSTANCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| host_error("native adapter binding identities are exhausted"))?;
        Ok(Self {
            module_abi_fingerprint: template.module_abi_fingerprint.clone(),
            identity: framed_identity(
                ADAPTER_IDENTITY_DOMAIN,
                &format!("{}:{instance}", template.module_abi_fingerprint),
            )?,
            bound_thread: std::thread::current().id(),
            host_thread_identity: instance,
        })
    }
}

/// Stage A: derive a deterministic, authority-free template from already
/// admitted cleanup and value proofs. This performs no classification or value
/// planning of its own.
pub(super) fn derive_from_admitted(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    resource_abi: &NativeResourceAbi,
    cleanup: &NativeCleanupIndex<'_>,
    values: &NativeValuePlan,
) -> Result<NativeHostContractTemplate, Diagnostic> {
    crate::hir::validate(program)?;
    if !program.function_templates.is_empty() || !program.function_instances.is_empty() {
        return Err(host_error(
            "native host contracts do not admit generic function templates or instances",
        ));
    }
    let mut matches = program
        .functions
        .iter()
        .filter(|candidate| candidate.id == *function_id);
    let function = matches
        .next()
        .ok_or_else(|| host_error(format!("function `{function_id}` is not in the program")))?;
    if matches.next().is_some() {
        return Err(host_error(format!(
            "function identity `{function_id}` resolves more than once"
        )));
    }
    let rebuilt_abi = super::native_resource::build_resource_abi(program)?;
    if &rebuilt_abi != resource_abi {
        return Err(host_error(
            "native resource ABI does not exactly match the validated program",
        ));
    }
    if !cleanup.belongs_to(function) || !values.belongs_to(function, cleanup) {
        return Err(host_error(format!(
            "admitted cleanup/value proof does not belong to function `{}`",
            function.id
        )));
    }
    let mut parameters = Vec::with_capacity(function.params.len());
    let mut owner_ordinal = 0;
    for (parameter_index, parameter) in function.params.iter().enumerate() {
        match &parameter.ty {
            ResolvedType::Unit => {
                return Err(host_error("unit is not an ordinary native host parameter"));
            }
            ResolvedType::I64 => {
                if parameter.ownership != OwnershipMode::Value {
                    return Err(host_error(format!(
                        "scalar parameter {} is not passed by value",
                        parameter_index
                    )));
                }
                parameters.push(NativeHostParameter::Scalar {
                    parameter_index,
                    value_id: parameter.id.clone(),
                    kind: NativeHostScalarKind::I64,
                });
            }
            ResolvedType::Bool => {
                if parameter.ownership != OwnershipMode::Value {
                    return Err(host_error(format!(
                        "scalar parameter {} is not passed by value",
                        parameter_index
                    )));
                }
                parameters.push(NativeHostParameter::Scalar {
                    parameter_index,
                    value_id: parameter.id.clone(),
                    kind: NativeHostScalarKind::Bool,
                });
            }
            ResolvedType::Char | ResolvedType::U8 | ResolvedType::F32 | ResolvedType::F64 => {
                return Err(host_error(format!(
                    "non-i64 scalar parameter {} is outside the native host slice",
                    parameter_index
                )));
            }
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                if parameter.ownership != OwnershipMode::Own || !arguments.is_empty() {
                    return Err(host_error(format!(
                        "resource parameter {} is not a direct owned value",
                        parameter_index
                    )));
                }
                let lifecycle = direct_trivial_lifecycle(program, resource_abi, declaration)?;
                parameters.push(NativeHostParameter::OwnedResource {
                    parameter_index,
                    value_id: parameter.id.clone(),
                    owner_ordinal,
                    resource_type: framed_identity(RESOURCE_IDENTITY_DOMAIN, declaration.as_str())?,
                    lifecycle: framed_identity(LIFECYCLE_IDENTITY_DOMAIN, lifecycle)?,
                });
                owner_ordinal += 1;
            }
            ResolvedType::TypeParameter { .. } => {
                return Err(host_error(format!(
                    "generic parameter {} is outside the native host slice",
                    parameter_index
                )));
            }
        }
    }

    let result = match values.result() {
        NativeValueResult::ScalarI64 if function.return_type == ResolvedType::I64 => {
            NativeHostResult::ScalarI64
        }
        NativeValueResult::OwnedInput {
            parameter_index,
            parameter,
            owner_ordinal,
        } => {
            let Some(NativeHostParameter::OwnedResource {
                parameter_index: proven_index,
                value_id,
                owner_ordinal: proven_ordinal,
                ..
            }) = parameters.get(*parameter_index)
            else {
                return Err(host_error(
                    "owned result proof does not select a resource parameter",
                ));
            };
            if proven_index != parameter_index
                || value_id != parameter
                || proven_ordinal != owner_ordinal
            {
                return Err(host_error(
                    "owned result proof disagrees with signature metadata",
                ));
            }
            NativeHostResult::OwnedInput {
                parameter_index: *parameter_index,
                value_id: parameter.clone(),
                owner_ordinal: *owner_ordinal,
            }
        }
        _ => {
            return Err(host_error(
                "value result proof disagrees with function result type",
            ))
        }
    };
    let mut template = NativeHostContractTemplate {
        semantic_module: program.module.clone(),
        module: framed_identity(MODULE_IDENTITY_DOMAIN, &program.module)?,
        module_abi_fingerprint: module_abi_fingerprint(program, resource_abi),
        semantic_function: function.id.as_str().to_owned(),
        function: framed_identity(FUNCTION_IDENTITY_DOMAIN, function.id.as_str())?,
        parameters,
        result,
        fingerprint: String::new(),
    };
    template.fingerprint = template_fingerprint(&template);
    Ok(template)
}

/// Stage B: attach one validated adapter instance and observed thread to an
/// authority-free template.
pub(super) fn bind(
    template: &NativeHostContractTemplate,
    adapter: &NativeHostAdapterBinding,
) -> Result<NativeBoundHostContract, Diagnostic> {
    if adapter.module_abi_fingerprint != template.module_abi_fingerprint {
        return Err(host_error(
            "adapter binding belongs to a different native module ABI",
        ));
    }
    if std::thread::current().id() != adapter.bound_thread {
        return Err(boundary_error(HostBoundaryRejection::WrongThread));
    }
    let requirements = template
        .parameters
        .iter()
        .filter_map(|parameter| match parameter {
            NativeHostParameter::Scalar { .. } => None,
            NativeHostParameter::OwnedResource {
                resource_type,
                lifecycle,
                ..
            } => Some(HostResourceRequirement::new(
                resource_type.clone(),
                lifecycle.clone(),
            )),
        })
        .collect();
    let result = match template.result {
        NativeHostResult::ScalarI64 => HostResultPlan::Scalar,
        NativeHostResult::OwnedInput { owner_ordinal, .. } => HostResultPlan::OwnedInput {
            input_index: owner_ordinal,
        },
    };
    let contract = HostCallContract::try_new(
        template.module.clone(),
        adapter.identity.clone(),
        framed_identity(BOUND_FUNCTION_IDENTITY_DOMAIN, &template.fingerprint)?,
        adapter.host_thread_identity,
        requirements,
        result,
    )
    .map_err(boundary_error)?;
    Ok(NativeBoundHostContract {
        contract,
        bound_thread: adapter.bound_thread,
        host_thread_identity: adapter.host_thread_identity,
    })
}

fn template_fingerprint(template: &NativeHostContractTemplate) -> String {
    let mut hasher = Sha256::new();
    hasher.update(TEMPLATE_FINGERPRINT_DOMAIN);
    hash_field(&mut hasher, template.module.as_str());
    hash_field(&mut hasher, &template.module_abi_fingerprint);
    hash_field(&mut hasher, template.function.as_str());
    hasher.update((template.parameters.len() as u64).to_be_bytes());
    for parameter in &template.parameters {
        match parameter {
            NativeHostParameter::Scalar {
                parameter_index,
                value_id,
                kind,
            } => {
                hasher.update([1]);
                hasher.update((*parameter_index as u64).to_be_bytes());
                hash_field(&mut hasher, value_id.as_str());
                hasher.update([match kind {
                    NativeHostScalarKind::I64 => 1,
                    NativeHostScalarKind::Bool => 2,
                }]);
            }
            NativeHostParameter::OwnedResource {
                parameter_index,
                value_id,
                owner_ordinal,
                resource_type,
                lifecycle,
            } => {
                hasher.update([2]);
                hasher.update((*parameter_index as u64).to_be_bytes());
                hash_field(&mut hasher, value_id.as_str());
                hasher.update((*owner_ordinal as u64).to_be_bytes());
                hash_field(&mut hasher, resource_type.as_str());
                hash_field(&mut hasher, lifecycle.as_str());
            }
        }
    }
    match &template.result {
        NativeHostResult::ScalarI64 => hasher.update([1]),
        NativeHostResult::OwnedInput {
            parameter_index,
            value_id,
            owner_ordinal,
        } => {
            hasher.update([2]);
            hasher.update((*parameter_index as u64).to_be_bytes());
            hash_field(&mut hasher, value_id.as_str());
            hasher.update((*owner_ordinal as u64).to_be_bytes());
        }
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(encoded, "{byte:02x}").expect("writing to a string cannot fail");
    }
    encoded
}

fn module_abi_fingerprint(program: &ResolvedProgram, abi: &NativeResourceAbi) -> String {
    let mut hasher = Sha256::new();
    hasher.update(MODULE_ABI_FINGERPRINT_DOMAIN);
    hash_field(&mut hasher, crate::host_ownership::HOST_OWNERSHIP_SCHEMA_V1);
    hash_field(&mut hasher, crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V2);
    hash_field(&mut hasher, &program.module);
    hasher.update((abi.resources.len() as u64).to_be_bytes());
    for resource in &abi.resources {
        hash_field(&mut hasher, resource.resource_id.as_str());
        hash_field(&mut hasher, &resource.c_type);
    }
    hasher.update((abi.lifecycles.len() as u64).to_be_bytes());
    for lifecycle in &abi.lifecycles {
        hash_field(&mut hasher, lifecycle.lifecycle_id.as_str());
        hash_field(&mut hasher, lifecycle.resource_id.as_str());
        hash_field(&mut hasher, &lifecycle.resource_c_type);
        match &lifecycle.kind {
            NativeFinalizerKind::Trivial => hasher.update([1]),
            NativeFinalizerKind::Imported(imported) => {
                hasher.update([2]);
                hash_field(&mut hasher, imported.import_id.as_str());
                hash_field(&mut hasher, &imported.import_key);
                hash_field(&mut hasher, &imported.callback_type);
                hash_field(&mut hasher, &imported.binding_field);
            }
        }
    }
    let mut functions = program.functions.iter().collect::<Vec<_>>();
    functions.sort_by(|left, right| left.id.cmp(&right.id));
    hasher.update((functions.len() as u64).to_be_bytes());
    for function in functions {
        hash_field(&mut hasher, function.id.as_str());
        hasher.update((function.params.len() as u64).to_be_bytes());
        for parameter in &function.params {
            hash_field(&mut hasher, parameter.id.as_str());
            hasher.update([ownership_tag(parameter.ownership)]);
            hash_field(&mut hasher, &parameter.ty.identity_key());
        }
        hash_field(&mut hasher, &function.return_type.identity_key());
    }
    encode_digest(hasher.finalize())
}

fn ownership_tag(ownership: OwnershipMode) -> u8 {
    match ownership {
        OwnershipMode::Value => 1,
        OwnershipMode::Own => 2,
        OwnershipMode::Borrow => 3,
        OwnershipMode::Shared => 4,
    }
}

fn encode_digest(digest: impl IntoIterator<Item = u8>) -> String {
    let digest = digest.into_iter();
    let (lower, _) = digest.size_hint();
    let mut encoded = String::with_capacity(lower * 2);
    for byte in digest {
        write!(encoded, "{byte:02x}").expect("writing to a string cannot fail");
    }
    encoded
}

fn hash_field(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn direct_trivial_lifecycle<'a>(
    program: &'a ResolvedProgram,
    resource_abi: &NativeResourceAbi,
    declaration: &crate::hir::DeclarationId,
) -> Result<&'a str, Diagnostic> {
    let resolved = program
        .types
        .iter()
        .find(|candidate| candidate.id == *declaration)
        .ok_or_else(|| host_error(format!("resource type `{declaration}` is not declared")))?;
    let ResolvedTypeDeclarationKind::Resource { drop } = &resolved.kind else {
        return Err(host_error(format!(
            "type `{declaration}` is not a resource"
        )));
    };
    if !matches!(drop.kind, ResolvedResourceDropKind::Trivial) {
        return Err(host_error(format!(
            "resource `{declaration}` does not have a trivial lifecycle"
        )));
    }
    let _ = resource_abi.c_type(
        program,
        &ResolvedType::Nominal {
            declaration: declaration.clone(),
            arguments: Vec::new(),
        },
    )?;
    let lifecycle = resource_abi
        .lifecycles
        .iter()
        .find(|candidate| candidate.resource_id == *declaration)
        .ok_or_else(|| host_error(format!("resource `{declaration}` has no native lifecycle")))?;
    if lifecycle.lifecycle_id != drop.id || !matches!(lifecycle.kind, NativeFinalizerKind::Trivial)
    {
        return Err(host_error(format!(
            "resource `{declaration}` lifecycle disagrees with the native ABI"
        )));
    }
    Ok(drop.id.as_str())
}

fn framed_identity(domain: &str, value: &str) -> Result<HostIdentity, Diagnostic> {
    HostIdentity::try_new(format!("{domain}:{}:{value}", value.len())).map_err(boundary_error)
}

fn boundary_error(rejection: HostBoundaryRejection) -> Diagnostic {
    host_error(format!("host ownership contract rejected: {rejection:?}"))
}

fn host_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        format!("native host contract: {}", message.into()),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;

    use crate::hir::{
        self, ExpressionId, OwnershipMode, ResolvedFunction, ResolvedResourceDropKind, ResolvedType,
    };
    use crate::parse;

    use super::super::native_resource;
    use super::*;

    const SOURCE: &str = r#"module test.native_host_contract;

@id("token.type")
resource Token { @id("token.drop") drop trivial; }

@id("other.type")
resource Other { @id("other.drop") drop trivial; }

@id("token.discard-two")
fn discard_two(first: own Token, count: i64, second: own Other) -> i64 { 0 }

@id("token.identity")
fn identity(count: i64, value: own Token) -> Token { value }

@id("token.choose-second")
fn choose_second(first: own Token, count: i64, second: own Token) -> Token { second }

@id("token.requires")
fn requires_guard(value: own Token, allowed: bool) -> i64
    requires allowed
{ 0 }

@id("token.checked")
fn checked(value: own Token, number: i64) -> i64
    requires number >= 0
{ number + 1 }

@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn program() -> ResolvedProgram {
        let parsed = parse(SOURCE, Path::new("native-host-contract.spx")).unwrap();
        hir::resolve(&parsed).unwrap()
    }

    fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
        program
            .functions
            .iter()
            .find(|candidate| candidate.id.as_str() == id)
            .unwrap()
    }

    fn admit(
        program: &ResolvedProgram,
        function_id: &DeclarationId,
        abi: &NativeResourceAbi,
        labels: &HashMap<ExpressionId, String>,
    ) -> Result<NativeHostContractTemplate, Diagnostic> {
        hir::validate(program)?;
        let function = program
            .functions
            .iter()
            .find(|candidate| candidate.id == *function_id)
            .ok_or_else(|| host_error("test function is missing"))?;
        let cleanup = super::super::native_cleanup::classify(program, function)?;
        let values = super::super::native_value::plan(program, function, &cleanup, abi, labels)?;
        derive_from_admitted(program, function_id, abi, &cleanup, &values)
    }

    #[test]
    fn template_is_deterministic_and_preserves_complete_signature_order() {
        let program = program();
        let abi = native_resource::build_resource_abi(&program).unwrap();
        let function = function(&program, "token.discard-two");
        let first = admit(&program, &function.id, &abi, &HashMap::new()).unwrap();
        let second = admit(&program, &function.id, &abi, &HashMap::new()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.fingerprint.len(), 64);
        assert_eq!(first.result, NativeHostResult::ScalarI64);
        assert_eq!(first.parameters.len(), 3);
        assert!(matches!(
            &first.parameters[0],
            NativeHostParameter::OwnedResource {
                parameter_index: 0,
                owner_ordinal: 0,
                value_id,
                ..
            } if *value_id == function.params[0].id
        ));
        assert!(matches!(
            &first.parameters[1],
            NativeHostParameter::Scalar {
                parameter_index: 1,
                value_id,
                kind: NativeHostScalarKind::I64,
            } if *value_id == function.params[1].id
        ));
        assert!(matches!(
            &first.parameters[2],
            NativeHostParameter::OwnedResource {
                parameter_index: 2,
                owner_ordinal: 1,
                value_id,
                ..
            } if *value_id == function.params[2].id
        ));
    }

    #[test]
    fn template_ignores_display_and_whitespace_but_tracks_scalar_abi_shape() {
        let original = program();
        let original_abi = native_resource::build_resource_abi(&original).unwrap();
        let original_function = function(&original, "token.identity");
        let original_template = admit(
            &original,
            &original_function.id,
            &original_abi,
            &HashMap::new(),
        )
        .unwrap();

        let renamed_source = format!(
            "\n{}",
            SOURCE.replace("fn identity(", "fn renamed_identity(")
        );
        let renamed_parsed = parse(
            &renamed_source,
            Path::new("native-host-contract-renamed.spx"),
        )
        .unwrap();
        let renamed = hir::resolve(&renamed_parsed).unwrap();
        let renamed_abi = native_resource::build_resource_abi(&renamed).unwrap();
        let renamed_function = function(&renamed, "token.identity");
        let renamed_template = admit(
            &renamed,
            &renamed_function.id,
            &renamed_abi,
            &HashMap::new(),
        )
        .unwrap();
        assert_eq!(original_template, renamed_template);

        let changed_source = SOURCE.replace(
            "fn identity(count: i64, value: own Token)",
            "fn identity(count: bool, value: own Token)",
        );
        let changed_parsed = parse(
            &changed_source,
            Path::new("native-host-contract-scalar-abi.spx"),
        )
        .unwrap();
        let changed = hir::resolve(&changed_parsed).unwrap();
        let changed_abi = native_resource::build_resource_abi(&changed).unwrap();
        let changed_function = function(&changed, "token.identity");
        let changed_template = admit(
            &changed,
            &changed_function.id,
            &changed_abi,
            &HashMap::new(),
        )
        .unwrap();
        assert_ne!(original_template.fingerprint, changed_template.fingerprint);
        assert_ne!(
            original_template.module_abi_fingerprint,
            changed_template.module_abi_fingerprint
        );
    }

    #[test]
    fn owned_result_maps_to_resource_ordinal_not_signature_index() {
        let program = program();
        let abi = native_resource::build_resource_abi(&program).unwrap();
        let function = function(&program, "token.identity");
        let actual = admit(&program, &function.id, &abi, &HashMap::new()).unwrap();
        assert_eq!(
            actual.result,
            NativeHostResult::OwnedInput {
                parameter_index: 1,
                value_id: function.params[1].id.clone(),
                owner_ordinal: 0,
            }
        );
    }

    #[test]
    fn owned_result_selects_the_exact_second_same_type_owner() {
        let program = program();
        let abi = native_resource::build_resource_abi(&program).unwrap();
        let function = function(&program, "token.choose-second");
        let actual = admit(&program, &function.id, &abi, &HashMap::new()).unwrap();
        assert_eq!(
            actual.result,
            NativeHostResult::OwnedInput {
                parameter_index: 2,
                value_id: function.params[2].id.clone(),
                owner_ordinal: 1,
            }
        );
    }

    #[test]
    fn contract_labels_flow_through_requires_and_checked_admission() {
        let program = program();
        let abi = native_resource::build_resource_abi(&program).unwrap();
        for id in ["token.requires", "token.checked"] {
            let function = function(&program, id);
            let labels =
                HashMap::from([(function.requires[0].id.clone(), format!("label for {id}"))]);
            let template = admit(&program, &function.id, &abi, &labels).unwrap();
            assert_eq!(template.result, NativeHostResult::ScalarI64);
            assert!(!template.fingerprint.is_empty());
        }
    }

    #[test]
    fn adapter_binding_is_stage_b_authority_and_instances_are_distinct() {
        let program = program();
        let abi = native_resource::build_resource_abi(&program).unwrap();
        let identity_function = function(&program, "token.identity");
        let template = admit(&program, &identity_function.id, &abi, &HashMap::new()).unwrap();
        let first = NativeHostAdapterBinding::for_current_thread(&template).unwrap();
        let second = NativeHostAdapterBinding::for_current_thread(&template).unwrap();
        let first_contract = bind(&template, &first).unwrap();
        let second_contract = bind(&template, &second).unwrap();
        assert_ne!(first_contract, second_contract);
        let mut wrong_thread_registry = HostOwnershipRegistry::try_new().unwrap();
        let wrong_request_thread = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    first_contract.execute_owned(&mut wrong_thread_registry, Vec::new(), |_| {
                        Ok::<(), NormalizedStatus>(())
                    })
                })
                .join()
                .unwrap()
        });
        assert_eq!(
            wrong_request_thread,
            HostBoundaryResult::Rejected(HostBoundaryRejection::WrongThread)
        );
        let wrong_thread = std::thread::scope(|scope| {
            scope
                .spawn(|| bind(&template, &first).unwrap_err().code)
                .join()
                .unwrap()
        });
        assert_eq!(wrong_thread, "SPX-B104");

        let changed_source = SOURCE.replace("token.drop", "token.drop.v2");
        let changed_parsed = parse(
            &changed_source,
            Path::new("native-host-contract-changed-abi.spx"),
        )
        .unwrap();
        let changed = hir::resolve(&changed_parsed).unwrap();
        let changed_abi = native_resource::build_resource_abi(&changed).unwrap();
        let changed_function = function(&changed, "token.identity");
        let changed_template = admit(
            &changed,
            &changed_function.id,
            &changed_abi,
            &HashMap::new(),
        )
        .unwrap();
        assert_eq!(
            bind(&changed_template, &first).unwrap_err().code,
            "SPX-B104"
        );

        let scalar_function = function(&program, "token.discard-two");
        let scalar_template = admit(&program, &scalar_function.id, &abi, &HashMap::new()).unwrap();
        let scalar_binding =
            NativeHostAdapterBinding::for_current_thread(&scalar_template).unwrap();
        let scalar_contract = bind(&scalar_template, &scalar_binding).unwrap();
        let mut scalar_registry = HostOwnershipRegistry::try_new().unwrap();
        assert_eq!(
            scalar_contract.execute_scalar(&mut scalar_registry, Vec::new(), |_| Ok(0)),
            HostBoundaryResult::Rejected(HostBoundaryRejection::InputCountMismatch)
        );
    }

    #[test]
    fn detached_and_mutated_admission_evidence_cannot_be_mixed() {
        let original = program();
        let original_abi = native_resource::build_resource_abi(&original).unwrap();
        let original_function = function(&original, "token.identity");
        let cleanup = super::super::native_cleanup::classify(&original, original_function).unwrap();
        let values = super::super::native_value::plan(
            &original,
            original_function,
            &cleanup,
            &original_abi,
            &HashMap::new(),
        )
        .unwrap();

        let detached = original.clone();
        let detached_abi = native_resource::build_resource_abi(&detached).unwrap();
        assert_eq!(
            derive_from_admitted(
                &detached,
                &function(&detached, "token.identity").id,
                &detached_abi,
                &cleanup,
                &values,
            )
            .unwrap_err()
            .code,
            "SPX-B104"
        );

        let mismatched_cleanup =
            super::super::native_cleanup::classify(&original, original_function).unwrap();
        assert_eq!(
            derive_from_admitted(
                &original,
                &original_function.id,
                &original_abi,
                &mismatched_cleanup,
                &values,
            )
            .unwrap_err()
            .code,
            "SPX-B104"
        );
    }

    #[test]
    fn detached_function_mutations_fail_closed() {
        let program = program();
        let abi = native_resource::build_resource_abi(&program).unwrap();
        let canonical = function(&program, "token.identity").clone();

        let mut ownership = program.clone();
        function_mut(&mut ownership, "token.identity").params[1].ownership = OwnershipMode::Borrow;
        assert!(admit(&ownership, &canonical.id, &abi, &HashMap::new()).is_err());

        let mut parameter_type = program.clone();
        function_mut(&mut parameter_type, "token.identity").params[1].ty = ResolvedType::I64;
        assert!(admit(&parameter_type, &canonical.id, &abi, &HashMap::new()).is_err());

        let mut result = program.clone();
        function_mut(&mut result, "token.identity").return_type = ResolvedType::I64;
        assert!(admit(&result, &canonical.id, &abi, &HashMap::new()).is_err());

        assert_eq!(
            admit(
                &program,
                &crate::hir::DeclarationId::new("other.function"),
                &abi,
                &HashMap::new(),
            )
            .unwrap_err()
            .code,
            "SPX-B104"
        );
    }

    #[test]
    fn lifecycle_and_abi_mutations_fail_closed() {
        let mut hostile = program();
        let type_index = hostile
            .types
            .iter()
            .position(|candidate| candidate.id.as_str() == "token.type")
            .unwrap();
        let ResolvedTypeDeclarationKind::Resource { drop } = &mut hostile.types[type_index].kind
        else {
            panic!("token is a resource");
        };
        drop.id = crate::hir::DeclarationId::new("hostile.drop");
        let abi = native_resource::build_resource_abi(&program()).unwrap();
        let hostile_function = hostile
            .functions
            .iter()
            .find(|candidate| candidate.id.as_str() == "token.identity")
            .unwrap();
        assert!(admit(&hostile, &hostile_function.id, &abi, &HashMap::new()).is_err());

        let clean = program();
        let clean_function = function(&clean, "token.identity");
        let clean_cleanup = super::super::native_cleanup::classify(&clean, clean_function).unwrap();
        let clean_values = super::super::native_value::plan(
            &clean,
            clean_function,
            &clean_cleanup,
            &native_resource::build_resource_abi(&clean).unwrap(),
            &HashMap::new(),
        )
        .unwrap();
        let mut wrong_abi = native_resource::build_resource_abi(&clean).unwrap();
        wrong_abi.lifecycles[0].kind =
            NativeFinalizerKind::Imported(super::super::native_resource::NativeImportedFinalizer {
                import_id: crate::hir::DeclarationId::new("hostile.import"),
                import_key: "hostile.import".to_owned(),
                callback_type: "hostile_callback".to_owned(),
                binding_field: "hostile_binding".to_owned(),
            });
        assert_eq!(
            derive_from_admitted(
                &clean,
                &clean_function.id,
                &wrong_abi,
                &clean_cleanup,
                &clean_values,
            )
            .unwrap_err()
            .code,
            "SPX-B104"
        );
    }

    #[test]
    fn imported_lifecycle_remains_rejected() {
        let clean_program = program();
        let abi = native_resource::build_resource_abi(&clean_program).unwrap();

        let mut imported = program();
        let token = imported
            .types
            .iter_mut()
            .find(|candidate| candidate.id.as_str() == "token.type")
            .unwrap();
        let ResolvedTypeDeclarationKind::Resource { drop } = &mut token.kind else {
            panic!("token is a resource");
        };
        drop.kind = ResolvedResourceDropKind::Imported {
            import: crate::hir::DeclarationId::new("missing.import"),
            import_key: "missing.import".to_owned(),
        };
        let function = imported
            .functions
            .iter()
            .find(|candidate| candidate.id.as_str() == "token.identity")
            .unwrap();
        assert!(admit(&imported, &function.id, &abi, &HashMap::new()).is_err());
    }

    fn function_mut<'a>(program: &'a mut ResolvedProgram, id: &str) -> &'a mut ResolvedFunction {
        program
            .functions
            .iter_mut()
            .find(|candidate| candidate.id.as_str() == id)
            .unwrap()
    }
}
