use std::fmt::Write as _;
use std::path::Path;

use super::{validate, DeclarationId, ResolvedType, ResolvedTypeDeclarationKind};
use crate::{hir, parse};

fn record_program() -> hir::ResolvedProgram {
    let source = r#"
module test.hostile_record_hir;
@id("node.type")
record Node {
    @id("node.value")
    value: i64,
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    hir::resolve(&parse(source, Path::new("hostile-record-hir.spx")).unwrap()).unwrap()
}

#[cfg(test)]
mod identity_nul_tests {
    use std::path::Path;

    use super::super::{
        validate, DeclarationId, ExpressionId, ResolvedResourceDropKind,
        ResolvedTypeDeclarationKind, ValueId,
    };
    use crate::{codegen, hir, parse, wasm};

    fn identity_program() -> hir::ResolvedProgram {
        let source = r#"
module test.hostile_identity_nul;
@id("token.type")
resource Token {
    @id("token.drop")
    drop import "host.dispose";
}
@id("pair.type")
record Pair { @id("pair.value") value: i64, }
@id("host.interface")
interface Host permits {} {
    @id("host.dispose")
    import fn dispose(token: own Token) -> unit
        effects {}
        failure infallible
        consumes token always;
}
@id("helper.function")
fn helper(value: i64) -> i64 { value }
@id("pair.make")
fn make_pair(value: i64) -> Pair { Pair { value: value } }
@id("pair.read")
fn read_pair(pair: Pair) -> i64 { pair.value }
@id("pair.read-temporary")
fn read_temporary() -> i64 { Pair { value: 1 }.value }
@id("token.discard")
fn discard(token: own Token) -> i64 { 0 }
@id("app.main")
fn main() -> i64 { helper(1) }
"#;
        hir::resolve(&parse(source, Path::new("hostile-identity-nul.spx")).unwrap()).unwrap()
    }

    fn assert_nul_rejected(program: &hir::ResolvedProgram, kind: &str) {
        let diagnostic = validate(program).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-H006", "wrong code for {kind}");
        assert!(
            diagnostic.message.contains("contains NUL"),
            "wrong diagnostic for {kind}: {}",
            diagnostic.message
        );
    }

    fn function_index(program: &hir::ResolvedProgram, name: &str) -> usize {
        program
            .functions
            .iter()
            .position(|function| function.name == name)
            .unwrap()
    }

    fn tail(expression: &super::super::ResolvedExpr) -> &super::super::ResolvedExpr {
        match &expression.kind {
            super::super::ResolvedExprKind::Block { tail, .. } => tail,
            _ => expression,
        }
    }

    fn tail_mut(expression: &mut super::super::ResolvedExpr) -> &mut super::super::ResolvedExpr {
        if matches!(
            &expression.kind,
            super::super::ResolvedExprKind::Block { .. }
        ) {
            let super::super::ResolvedExprKind::Block { tail, .. } = &mut expression.kind else {
                unreachable!()
            };
            tail
        } else {
            expression
        }
    }

    #[test]
    fn validator_rejects_nul_in_every_persistent_hir_identity_carrier() {
        let original = identity_program();

        let mut program = original.clone();
        program.entrypoint = DeclarationId::new("app.main\0forged");
        assert_nul_rejected(&program, "entry point");

        let mut program = original.clone();
        program.types[0].id = DeclarationId::new("token.type\0forged");
        assert_nul_rejected(&program, "resource");

        let mut program = original.clone();
        let ResolvedTypeDeclarationKind::Resource { drop } = &mut program.types[0].kind else {
            panic!("Token must be a resource")
        };
        drop.id = DeclarationId::new("token.drop\0forged");
        assert_nul_rejected(&program, "resource lifecycle");

        let mut program = original.clone();
        let record = program
            .types
            .iter_mut()
            .find(|declaration| declaration.name == "Pair")
            .unwrap();
        record.id = DeclarationId::new("pair.type\0forged");
        assert_nul_rejected(&program, "record");

        let mut program = original.clone();
        let record = program
            .types
            .iter_mut()
            .find(|declaration| declaration.name == "Pair")
            .unwrap();
        let ResolvedTypeDeclarationKind::Record { fields } = &mut record.kind else {
            panic!("Pair must be a record")
        };
        fields[0].id = DeclarationId::new("pair.value\0forged");
        assert_nul_rejected(&program, "field");

        let mut program = original.clone();
        program.interfaces[0].id = DeclarationId::new("host.interface\0forged");
        assert_nul_rejected(&program, "interface");

        let mut program = original.clone();
        program.interfaces[0].imports[0].id = DeclarationId::new("host.dispose\0forged");
        assert_nul_rejected(&program, "import");

        let mut program = original.clone();
        program.interfaces[0].imports[0].import_key = "host.dispose\0forged".to_owned();
        assert_nul_rejected(&program, "logical import key");

        let mut program = original;
        program.functions[0].id = DeclarationId::new("helper.function\0forged");
        assert_nul_rejected(&program, "function");
    }

    #[test]
    fn validator_rejects_nul_in_derived_expression_and_value_identities() {
        let original = identity_program();
        let helper_index = original
            .functions
            .iter()
            .position(|function| function.name == "helper")
            .unwrap();

        let mut program = original.clone();
        program.functions[helper_index].body.id = ExpressionId("expression\0forged".to_owned());
        assert_nul_rejected(&program, "expression");

        let mut program = original.clone();
        program.functions[helper_index].params[0].id = ValueId("value\0forged".to_owned());
        assert_nul_rejected(&program, "parameter value");

        let mut program = original;
        program.functions[helper_index].result_id = ValueId("result\0forged".to_owned());
        assert_nul_rejected(&program, "result value");
    }

    #[test]
    fn validator_normalizes_nul_across_core_hir_reference_carriers() {
        let original = identity_program();

        let mut program = original.clone();
        let helper = function_index(&program, "helper");
        program.functions[helper].params[0].ty = super::super::ResolvedType::Nominal {
            declaration: DeclarationId::new("type\0forged"),
            arguments: vec![super::super::ResolvedType::TypeParameter {
                owner: DeclarationId::new("owner.safe"),
                index: 0,
            }],
        };
        assert_nul_rejected(&program, "nominal type declaration");

        let mut program = original.clone();
        let helper = function_index(&program, "helper");
        program.functions[helper].params[0].ty = super::super::ResolvedType::TypeParameter {
            owner: DeclarationId::new("owner\0forged"),
            index: 0,
        };
        assert_nul_rejected(&program, "type-parameter owner");

        let mut program = original.clone();
        let main = function_index(&program, "main");
        let super::super::ResolvedExprKind::Call { callee, .. } =
            &mut tail_mut(&mut program.functions[main].body).kind
        else {
            panic!("main must call helper")
        };
        *callee = DeclarationId::new("callee\0forged");
        assert_nul_rejected(&program, "call target");

        let mut program = original.clone();
        let helper = function_index(&program, "helper");
        let super::super::ResolvedExprKind::Place(place) =
            &mut tail_mut(&mut program.functions[helper].body).kind
        else {
            panic!("helper must return a place")
        };
        place.root = ValueId("place\0forged".to_owned());
        assert_nul_rejected(&program, "place root");

        let mut program = original.clone();
        let reader = function_index(&program, "read_pair");
        let super::super::ResolvedExprKind::Place(place) =
            &mut tail_mut(&mut program.functions[reader].body).kind
        else {
            panic!("record parameter projection must remain a place")
        };
        place.projections[0] =
            super::super::PlaceProjection::Field(DeclarationId::new("field\0forged"));
        assert_nul_rejected(&program, "place projection");

        let mut program = original.clone();
        let maker = function_index(&program, "make_pair");
        let super::super::ResolvedExprKind::ConstructRecord { record, .. } =
            &mut tail_mut(&mut program.functions[maker].body).kind
        else {
            panic!("make_pair must construct a record")
        };
        *record = DeclarationId::new("record\0forged");
        assert_nul_rejected(&program, "record constructor");

        let mut program = original.clone();
        let maker = function_index(&program, "make_pair");
        let super::super::ResolvedExprKind::ConstructRecord { fields, .. } =
            &mut tail_mut(&mut program.functions[maker].body).kind
        else {
            panic!("make_pair must construct a record")
        };
        fields[0].field = DeclarationId::new("initializer\0forged");
        assert_nul_rejected(&program, "record initializer field");

        let mut program = original;
        let reader = function_index(&program, "read_temporary");
        let super::super::ResolvedExprKind::Project { field, .. } =
            &mut tail_mut(&mut program.functions[reader].body).kind
        else {
            panic!("temporary record projection must remain explicit")
        };
        *field = DeclarationId::new("projected\0forged");
        assert_nul_rejected(&program, "projected field");
    }

    #[test]
    fn validator_normalizes_nul_across_cleanup_inventory_and_plan_references() {
        let original = identity_program();
        let discard = function_index(&original, "discard");

        let mut program = original.clone();
        let crate::cleanup::CleanupStorageOrigin::Parameter { value, .. } =
            &mut program.functions[discard].cleanup.slots[0].origin
        else {
            panic!("discard must own parameter storage")
        };
        *value = ValueId("inventory\0forged".to_owned());
        assert_nul_rejected(&program, "inventory value");

        let mut program = original.clone();
        program.functions[discard].cleanup.flags[0]
            .place
            .projections
            .push(DeclarationId::new("inventory.projection\0forged"));
        assert_nul_rejected(&program, "inventory projection");

        let mut program = original.clone();
        program.functions[discard].cleanup_plan.slots[0].storage =
            crate::cleanup_plan::StorageId::CallArgument {
                call: ExpressionId("plan.call\0forged".to_owned()),
                parameter_index: 0,
                value_expression: ExpressionId("plan.value".to_owned()),
            };
        assert_nul_rejected(&program, "plan call-argument storage");

        let mut program = original.clone();
        program.functions[discard]
            .cleanup_plan
            .entry_state
            .live_owned_parameters[0]
            .projections
            .push(DeclarationId::new("plan.projection\0forged"));
        assert_nul_rejected(&program, "plan place projection");

        let mut program = original.clone();
        let finalizer = program.functions[discard]
            .cleanup_plan
            .exits
            .iter_mut()
            .find_map(|exit| exit.finalize_in_order.first_mut())
            .expect("discard must finalize its parameter");
        finalizer.lifecycle_id = DeclarationId::new("plan.lifecycle\0forged");
        assert_nul_rejected(&program, "plan finalizer lifecycle");

        let mut program = original;
        let main = function_index(&program, "main");
        let source = program.functions[main]
            .cleanup_plan
            .status_sources
            .iter_mut()
            .find(|source| {
                matches!(
                    &source.producer,
                    crate::cleanup_plan::StatusProducer::PropagatedCall { .. }
                )
            })
            .expect("main call must have a propagated status source");
        let crate::cleanup_plan::StatusProducer::PropagatedCall { callee } = &mut source.producer
        else {
            unreachable!()
        };
        *callee = DeclarationId::new("plan.callee\0forged");
        assert_nul_rejected(&program, "plan propagated callee");
    }

    #[test]
    fn native_and_wasm_reject_nul_before_backend_feature_gates() {
        let mut program = identity_program();
        let main = function_index(&program, "main");
        let super::super::ResolvedExprKind::Call { callee, .. } =
            &mut tail_mut(&mut program.functions[main].body).kind
        else {
            panic!("main must call helper")
        };
        *callee = DeclarationId::new("helper.function\0forged");

        let native = codegen::emit_hir_c(&program).unwrap_err();
        assert_eq!(native.code, "SPX-H006");
        assert!(native.message.contains("contains NUL"));

        let wasm = wasm::emit_resolved_module(&program).unwrap_err();
        assert_eq!(wasm.code, "SPX-H006");
        assert!(wasm.message.contains("contains NUL"));

        let mut cleanup_program = identity_program();
        let discard = function_index(&cleanup_program, "discard");
        let finalizer = cleanup_program.functions[discard]
            .cleanup_plan
            .exits
            .iter_mut()
            .find_map(|exit| exit.finalize_in_order.first_mut())
            .expect("discard must finalize its parameter");
        finalizer.lifecycle_id = DeclarationId::new("cleanup.lifecycle\0forged");

        let native = codegen::emit_hir_c(&cleanup_program).unwrap_err();
        assert_eq!(native.code, "SPX-H006");
        assert!(native.message.contains("contains NUL"));

        let wasm = wasm::emit_resolved_module(&cleanup_program).unwrap_err();
        assert_eq!(wasm.code, "SPX-H006");
        assert!(wasm.message.contains("contains NUL"));
    }

    #[test]
    fn valid_identity_program_keeps_its_existing_validation_result() {
        let program = identity_program();
        validate(&program).unwrap();
        let helper = function_index(&program, "helper");
        assert!(matches!(
            &tail(&program.functions[helper].body).kind,
            super::super::ResolvedExprKind::Place(_)
        ));
        let ResolvedTypeDeclarationKind::Resource { drop } = &program.types[0].kind else {
            panic!("Token must be a resource")
        };
        assert!(matches!(
            drop.kind,
            ResolvedResourceDropKind::Imported { .. }
        ));
    }
}

#[test]
fn validator_rejects_a_forged_by_value_recursive_record_index() {
    let mut program = record_program();
    let recursive = ResolvedType::Nominal {
        declaration: DeclarationId::new("node.type"),
        arguments: Vec::new(),
    };
    let ResolvedTypeDeclarationKind::Record { fields } = &mut program.types[0].kind else {
        panic!("Node must be a record");
    };
    fields[0].ty = recursive.clone();
    program
        .declarations
        .record_fields
        .get_mut(&DeclarationId::new("node.type"))
        .unwrap()[0]
        .ty = recursive;

    assert_eq!(validate(&program).unwrap_err().code, "SPX-H006");
}

#[test]
fn validator_rejects_unit_in_an_ordinary_record_field_and_index() {
    let mut program = record_program();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut program.types[0].kind else {
        panic!("Node must be a record");
    };
    fields[0].ty = ResolvedType::Unit;
    program
        .declarations
        .record_fields
        .get_mut(&DeclarationId::new("node.type"))
        .unwrap()[0]
        .ty = ResolvedType::Unit;

    let error = validate(&program).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert!(error
        .message
        .contains("uses Unit outside a native Rust import result"));
}

#[test]
fn validator_rejects_a_field_owned_by_the_wrong_record() {
    let mut program = record_program();
    program
        .declarations
        .declarations
        .get_mut(&DeclarationId::new("node.value"))
        .unwrap()
        .owner = Some(DeclarationId::new("forged.owner"));

    assert_eq!(validate(&program).unwrap_err().code, "SPX-H006");
}

#[test]
fn iterative_resolver_and_validator_report_allocated_vec_capacity() {
    let source = "module capacity.hir; @id(\"capacity.choose\") fn choose(value: i64) -> i64 { if value == 0 { value } else { value + 1 } } @id(\"app.main\") fn main() -> i64 { choose(0) }";
    let parsed = crate::parse(source, std::path::Path::new("capacity-hir.spx")).unwrap();
    crate::source_verify::reset_capacity_high_water();
    super::reset_iterative_phase_capacity_high_water();
    let resolved = super::resolve(&parsed).unwrap();
    validate(&resolved).unwrap();
    let water = super::iterative_phase_capacity_high_water();
    assert!(water[0] >= std::mem::size_of::<super::ResolvedExpr>());
    assert!(water[1] > 0);
    assert!(water[2] > 0);
    assert!(crate::source_verify::capacity_high_water() > 0);
}

#[test]
fn type_facts_capacity_high_water_covers_layered_and_wide_hostiles() {
    use sha2::{Digest, Sha256};

    fn layered(resource: bool, levels: usize) -> String {
        let mut source = String::from("module capacity.typefacts.layers;\n\n");
        if resource {
            source.push_str(
                    "@id(\"layer.r0\")\nresource R0 {\n    @id(\"layer.r0.drop\")\n    drop trivial;\n}\n\n",
                );
        } else {
            source.push_str(
                    "@id(\"layer.r0\")\nrecord R0 {\n    @id(\"layer.r0.value\")\n    value: i64,\n}\n\n",
                );
        }
        for level in 1..=levels {
            writeln!(
                    source,
                    "@id(\"layer.r{level}\")\nrecord R{level} {{\n    @id(\"layer.r{level}.a\")\n    a: R{},\n    @id(\"layer.r{level}.b\")\n    b: R{},\n}}\n",
                    level - 1,
                    level - 1
                )
                .unwrap();
        }
        source.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");
        source
    }

    fn resolve_type_facts_peak(source: &str, name: &str) -> (String, usize) {
        let parsed = crate::parse(source, std::path::Path::new(name)).unwrap();
        let canonical = crate::format::canonical(&parsed);
        super::reset_iterative_phase_capacity_high_water();
        super::resolve(&parsed).unwrap();
        (
            format!(
                "sha256:{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(canonical.as_bytes()))
            ),
            super::iterative_phase_capacity_high_water()[2],
        )
    }

    let scalar = layered(false, 12);
    let resource = layered(true, 12);
    let mut wide = String::from("module capacity.typefacts.wide;\n\n");
    for index in 0..514 {
        writeln!(
                wide,
                "@id(\"wide.r{index}\")\nrecord R{index} {{\n    @id(\"wide.r{index}.value\")\n    value: i64,\n}}\n"
            )
            .unwrap();
    }
    wide.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");
    let mut chain = String::from(
            "module capacity.typefacts.chain;\n\n@id(\"chain.r0\")\nrecord R0 {\n    @id(\"chain.r0.value\")\n    value: i64,\n}\n\n",
        );
    for index in 1..514 {
        writeln!(
                chain,
                "@id(\"chain.r{index}\")\nrecord R{index} {{\n    @id(\"chain.r{index}.next\")\n    next: R{},\n}}\n",
                index - 1
            )
            .unwrap();
    }
    chain.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");

    let observed = [
        resolve_type_facts_peak(&scalar, "typefacts-layered-scalar.spx"),
        resolve_type_facts_peak(&resource, "typefacts-layered-resource.spx"),
        resolve_type_facts_peak(&wide, "typefacts-wide.spx"),
        resolve_type_facts_peak(&chain, "typefacts-chain.spx"),
    ];
    let expected = [
        (
            "sha256:cfa16985be87d169c3fb81d5958126347ec82b4c1afed878e2d98d1fbfe72c80",
            1_741_515,
            669_965_618,
        ),
        (
            "sha256:461611e4315e312330af0285273568e5d09cd8e5770a35dcf66a82783aa15ae6",
            1_397_458,
            2_886_293_140,
        ),
        (
            "sha256:dc19474b86def3eaf6e3c60cc2224694e6aa7cf2811cca6115943c11102f95fc",
            96_838,
            122_429_248,
        ),
        (
            "sha256:d2692d4883957575ee95df8f9ee7057343599e1da945c386cedea714c716f66d",
            6_273_598,
            31_588_832_202,
        ),
    ];
    for ((digest, actual), (expected_digest, expected_actual, envelope)) in
        observed.into_iter().zip(expected)
    {
        assert_eq!(digest, expected_digest, "canonical hostile fixture drifted");
        assert_eq!(
            actual, expected_actual,
            "TypeFacts owned-capacity peak drifted"
        );
        assert!(
            actual <= envelope,
            "TypeFacts observed total exceeded retained_upper + TypeFacts phase"
        );
    }
}

#[test]
fn useful_data_workspace_linker_reconstructs_and_rejects_hostile_slice_provenance() {
    let source = r#"
module test.useful_data_link;

@id("data.length")
fn length(value: borrow Slice<u8>) -> usize {
    let alias = value;
    byte_len(alias)
}

@id("data.count")
fn count(value: borrow Slice<u8>) -> usize {
    let mut index = 0usize;
    while index < byte_len(value) {
        index = index + 1usize;
        index < byte_len(value)
    }
    match byte_get(value, 0usize) {
        Option::Some { value: _ } => index,
        Option::None {} => index,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let parsed = crate::parse(source, "useful-data-link.spx").unwrap();
    let resolved = super::resolve(&parsed).unwrap();
    let entrypoint = resolved.entrypoint.clone();
    let linked_functions = resolved
        .functions
        .iter()
        .cloned()
        .map(|function| super::LinkedScalarFunction {
            function,
            origin: super::IdentityOrigin::Explicit,
        })
        .collect::<Vec<_>>();
    let linked = super::link_useful_data_workspace(
        resolved.module.clone(),
        entrypoint.clone(),
        resolved
            .functions
            .iter()
            .cloned()
            .map(|function| super::LinkedScalarFunction {
                function,
                origin: super::IdentityOrigin::Explicit,
            })
            .collect(),
    )
    .unwrap();
    let length = linked
        .functions
        .iter()
        .find(|function| function.id.as_str() == "data.length")
        .unwrap();
    let parameter = &length.params[0].id;
    let provenance = linked
        .declarations
        .byte_slice_provenance(parameter)
        .unwrap();
    assert_eq!(provenance.root, *parameter);
    assert_eq!(
        provenance.root_kind,
        super::ByteSliceRootKind::FunctionParameter
    );
    assert!(linked
        .functions
        .iter()
        .any(|function| function.id.as_str() == "data.count"));

    let mut hostile = linked_functions;
    let length = hostile
        .iter_mut()
        .find(|linked| linked.function.id.as_str() == "data.length")
        .unwrap();
    let super::ResolvedExprKind::Block { statements, .. } = &mut length.function.body.kind else {
        panic!("fixture function body is a block");
    };
    let super::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("fixture first statement is a let");
    };
    let super::ResolvedExprKind::Place(place) = &mut value.kind else {
        panic!("fixture slice alias is a place");
    };
    place.root = super::ValueId("hostile.missing-root".to_owned());
    let error =
        super::link_useful_data_workspace(resolved.module, entrypoint, hostile).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert!(error
        .message
        .contains("byte-slice alias lacks a canonical symbolic parameter root"));
}
