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
        let labels = HashMap::from([(function.requires[0].id.clone(), format!("label for {id}"))]);
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
    let scalar_binding = NativeHostAdapterBinding::for_current_thread(&scalar_template).unwrap();
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
    let ResolvedTypeDeclarationKind::Resource { drop } = &mut hostile.types[type_index].kind else {
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
