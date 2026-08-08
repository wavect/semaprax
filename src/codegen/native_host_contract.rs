//! Compiler-derived native host ownership contracts for the gated resource slice.
//!
//! This module is private groundwork. It derives every semantic identity and
//! ownership requirement from validated HIR plus the validated native resource
//! ABI. A trusted adapter supplies only its bound thread identity. Public native
//! resource lowering remains closed behind `SPX-B104`.

#![cfg_attr(
    not(test),
    allow(dead_code, reason = "native resource host entry points remain gated")
)]

use std::collections::HashMap;

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, OwnershipMode, ResolvedExprKind, ResolvedProgram,
    ResolvedResourceDropKind, ResolvedType, ResolvedTypeDeclarationKind,
};
use crate::host_ownership::{
    HostBoundaryRejection, HostCallContract, HostIdentity, HostResourceRequirement, HostResultPlan,
};

use super::native_resource::{NativeFinalizerKind, NativeResourceAbi};
use super::{native_cleanup, native_resource, native_value};

const MODULE_IDENTITY_DOMAIN: &str = "semaprax.native-host-module.v1";
const ADAPTER_IDENTITY_DOMAIN: &str = "semaprax.native-host-adapter.v1";
const FUNCTION_IDENTITY_DOMAIN: &str = "semaprax.native-host-function.v1";
const RESOURCE_IDENTITY_DOMAIN: &str = "semaprax.native-host-resource.v1";
const LIFECYCLE_IDENTITY_DOMAIN: &str = "semaprax.native-host-lifecycle.v1";

/// Validated binding policy for the current import-free trivial-resource host.
///
/// There is no caller-provided adapter identity. Until real adapter manifests
/// exist, exactly one logical trivial binding is derived from the validated
/// module and its trusted thread policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeHostAdapterBinding {
    module: String,
    identity: HostIdentity,
    bound_thread: u64,
}

impl NativeHostAdapterBinding {
    pub(super) fn for_trivial_slice(
        program: &ResolvedProgram,
        bound_thread: u64,
    ) -> Result<Self, Diagnostic> {
        hir::validate(program)?;
        if bound_thread == 0 {
            return Err(boundary_error(HostBoundaryRejection::WrongThread));
        }
        Ok(Self {
            module: program.module.clone(),
            identity: framed_identity(ADAPTER_IDENTITY_DOMAIN, &program.module)?,
            bound_thread,
        })
    }
}

/// Derive one immutable host ownership contract from the exact admitted native
/// value/cleanup slice. Caller-authored type, lifecycle, ownership, or result
/// metadata never crosses this boundary.
pub(super) fn derive(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    resource_abi: &NativeResourceAbi,
    adapter: &NativeHostAdapterBinding,
) -> Result<HostCallContract, Diagnostic> {
    hir::validate(program)?;
    if adapter.module != program.module {
        return Err(host_error(format!(
            "adapter binding belongs to module `{}`, not `{}`",
            adapter.module, program.module
        )));
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

    let rebuilt_abi = native_resource::build_resource_abi(program)?;
    if &rebuilt_abi != resource_abi {
        return Err(host_error(
            "native resource ABI does not exactly match the validated program",
        ));
    }

    let cleanup = native_cleanup::classify(program, function)?;
    let _values = native_value::plan(program, function, &cleanup, resource_abi, &HashMap::new())?;

    let mut requirements = Vec::new();
    let mut resource_parameter_ordinals = HashMap::new();
    for (parameter_index, parameter) in function.params.iter().enumerate() {
        match &parameter.ty {
            ResolvedType::I64 | ResolvedType::Bool => {
                if parameter.ownership != OwnershipMode::Value {
                    return Err(host_error(format!(
                        "scalar parameter {} is not passed by value",
                        parameter_index
                    )));
                }
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
                let ordinal = requirements.len();
                if resource_parameter_ordinals
                    .insert(parameter.id.clone(), ordinal)
                    .is_some()
                {
                    return Err(host_error("resource parameter identity is duplicated"));
                }
                requirements.push(HostResourceRequirement::new(
                    framed_identity(RESOURCE_IDENTITY_DOMAIN, declaration.as_str())?,
                    framed_identity(LIFECYCLE_IDENTITY_DOMAIN, lifecycle)?,
                ));
            }
            ResolvedType::TypeParameter { .. } => {
                return Err(host_error(format!(
                    "generic parameter {} is outside the native host slice",
                    parameter_index
                )));
            }
        }
    }

    let result = match &function.return_type {
        ResolvedType::I64 => HostResultPlan::Scalar,
        ResolvedType::Nominal { .. } => {
            let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
                return Err(host_error(
                    "owned result body is not the canonical root block",
                ));
            };
            let ResolvedExprKind::Place(place) = &tail.kind else {
                return Err(host_error("owned result is not an exact input place"));
            };
            if !place.projections.is_empty() {
                return Err(host_error("owned result uses a projected input place"));
            }
            let input_index = resource_parameter_ordinals
                .get(&place.root)
                .copied()
                .ok_or_else(|| {
                    host_error("owned result does not map to an owned resource input")
                })?;
            HostResultPlan::OwnedInput { input_index }
        }
        ResolvedType::Bool | ResolvedType::TypeParameter { .. } => {
            return Err(host_error("result type is outside the native host slice"));
        }
    };

    HostCallContract::try_new(
        framed_identity(MODULE_IDENTITY_DOMAIN, &program.module)?,
        adapter.identity.clone(),
        framed_identity(FUNCTION_IDENTITY_DOMAIN, function.id.as_str())?,
        adapter.bound_thread,
        requirements,
        result,
    )
    .map_err(boundary_error)
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
    use std::path::Path;

    use crate::hir::{
        self, OwnershipMode, ResolvedFunction, ResolvedResourceDropKind, ResolvedType,
    };
    use crate::parse;

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

    #[test]
    fn contract_is_deterministic_and_uses_resource_parameter_order() {
        let program = program();
        let abi = native_resource::build_resource_abi(&program).unwrap();
        let function = function(&program, "token.discard-two");
        let adapter = NativeHostAdapterBinding::for_trivial_slice(&program, 17).unwrap();
        let first = derive(&program, &function.id, &abi, &adapter).unwrap();
        let second = derive(&program, &function.id, &abi, &adapter).unwrap();
        assert_eq!(first, second);

        let expected = HostCallContract::try_new(
            framed_identity(MODULE_IDENTITY_DOMAIN, &program.module).unwrap(),
            framed_identity(ADAPTER_IDENTITY_DOMAIN, &program.module).unwrap(),
            framed_identity(FUNCTION_IDENTITY_DOMAIN, function.id.as_str()).unwrap(),
            17,
            vec![
                HostResourceRequirement::new(
                    framed_identity(RESOURCE_IDENTITY_DOMAIN, "token.type").unwrap(),
                    framed_identity(LIFECYCLE_IDENTITY_DOMAIN, "token.drop").unwrap(),
                ),
                HostResourceRequirement::new(
                    framed_identity(RESOURCE_IDENTITY_DOMAIN, "other.type").unwrap(),
                    framed_identity(LIFECYCLE_IDENTITY_DOMAIN, "other.drop").unwrap(),
                ),
            ],
            HostResultPlan::Scalar,
        )
        .unwrap();
        assert_eq!(first, expected);
    }

    #[test]
    fn owned_result_maps_to_resource_ordinal_not_signature_index() {
        let program = program();
        let abi = native_resource::build_resource_abi(&program).unwrap();
        let function = function(&program, "token.identity");
        let adapter = NativeHostAdapterBinding::for_trivial_slice(&program, 23).unwrap();
        let actual = derive(&program, &function.id, &abi, &adapter).unwrap();
        let expected = HostCallContract::try_new(
            framed_identity(MODULE_IDENTITY_DOMAIN, &program.module).unwrap(),
            framed_identity(ADAPTER_IDENTITY_DOMAIN, &program.module).unwrap(),
            framed_identity(FUNCTION_IDENTITY_DOMAIN, function.id.as_str()).unwrap(),
            23,
            vec![HostResourceRequirement::new(
                framed_identity(RESOURCE_IDENTITY_DOMAIN, "token.type").unwrap(),
                framed_identity(LIFECYCLE_IDENTITY_DOMAIN, "token.drop").unwrap(),
            )],
            HostResultPlan::OwnedInput { input_index: 0 },
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn owned_result_selects_the_exact_second_same_type_owner() {
        let program = program();
        let abi = native_resource::build_resource_abi(&program).unwrap();
        let adapter = NativeHostAdapterBinding::for_trivial_slice(&program, 29).unwrap();
        let function = function(&program, "token.choose-second");
        let actual = derive(&program, &function.id, &abi, &adapter).unwrap();
        let expected = HostCallContract::try_new(
            framed_identity(MODULE_IDENTITY_DOMAIN, &program.module).unwrap(),
            framed_identity(ADAPTER_IDENTITY_DOMAIN, &program.module).unwrap(),
            framed_identity(FUNCTION_IDENTITY_DOMAIN, function.id.as_str()).unwrap(),
            29,
            vec![
                HostResourceRequirement::new(
                    framed_identity(RESOURCE_IDENTITY_DOMAIN, "token.type").unwrap(),
                    framed_identity(LIFECYCLE_IDENTITY_DOMAIN, "token.drop").unwrap(),
                ),
                HostResourceRequirement::new(
                    framed_identity(RESOURCE_IDENTITY_DOMAIN, "token.type").unwrap(),
                    framed_identity(LIFECYCLE_IDENTITY_DOMAIN, "token.drop").unwrap(),
                ),
            ],
            HostResultPlan::OwnedInput { input_index: 1 },
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn detached_function_mutations_fail_closed() {
        let program = program();
        let abi = native_resource::build_resource_abi(&program).unwrap();
        let canonical = function(&program, "token.identity").clone();
        let adapter = NativeHostAdapterBinding::for_trivial_slice(&program, 31).unwrap();

        let mut ownership = program.clone();
        function_mut(&mut ownership, "token.identity").params[1].ownership = OwnershipMode::Borrow;
        assert!(derive(&ownership, &canonical.id, &abi, &adapter).is_err());

        let mut parameter_type = program.clone();
        function_mut(&mut parameter_type, "token.identity").params[1].ty = ResolvedType::I64;
        assert!(derive(&parameter_type, &canonical.id, &abi, &adapter).is_err());

        let mut result = program.clone();
        function_mut(&mut result, "token.identity").return_type = ResolvedType::I64;
        assert!(derive(&result, &canonical.id, &abi, &adapter).is_err());

        assert_eq!(
            derive(
                &program,
                &crate::hir::DeclarationId::new("other.function"),
                &abi,
                &adapter,
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
        let adapter = NativeHostAdapterBinding::for_trivial_slice(&program(), 37).unwrap();
        assert!(derive(&hostile, &hostile_function.id, &abi, &adapter).is_err());

        let clean = program();
        let mut wrong_abi = native_resource::build_resource_abi(&clean).unwrap();
        wrong_abi.lifecycles[0].kind =
            NativeFinalizerKind::Imported(super::super::native_resource::NativeImportedFinalizer {
                import_id: crate::hir::DeclarationId::new("hostile.import"),
                import_key: "hostile.import".to_owned(),
                callback_type: "hostile_callback".to_owned(),
                binding_field: "hostile_binding".to_owned(),
            });
        assert_eq!(
            derive(
                &clean,
                &function(&clean, "token.identity").id,
                &wrong_abi,
                &NativeHostAdapterBinding::for_trivial_slice(&clean, 37).unwrap(),
            )
            .unwrap_err()
            .code,
            "SPX-B104"
        );
    }

    #[test]
    fn imported_lifecycle_and_zero_thread_remain_rejected() {
        let clean_program = program();
        let abi = native_resource::build_resource_abi(&clean_program).unwrap();
        assert_eq!(
            NativeHostAdapterBinding::for_trivial_slice(&clean_program, 0)
                .unwrap_err()
                .code,
            "SPX-B104"
        );

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
        let adapter = NativeHostAdapterBinding::for_trivial_slice(&clean_program, 41).unwrap();
        assert!(derive(&imported, &function.id, &abi, &adapter).is_err());
    }

    fn function_mut<'a>(program: &'a mut ResolvedProgram, id: &str) -> &'a mut ResolvedFunction {
        program
            .functions
            .iter_mut()
            .find(|candidate| candidate.id.as_str() == id)
            .unwrap()
    }
}
