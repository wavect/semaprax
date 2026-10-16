use super::*;
use crate::ast::{Function, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::{Availability, Binding};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

fn source(declaration: &str) -> String {
    format!("module t; {declaration} @id(\"t.main\") fn main()->i64{{0}}")
}
fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}
fn variables(function: &Function) -> HashMap<String, Binding> {
    function
        .params
        .iter()
        .map(|parameter| {
            (
                parameter.name.clone(),
                Binding {
                    contract_local: false,
                    traversal_row: None,
                    ty: parameter.ty.clone(),
                    mode: parameter.mode,
                    availability: Availability::Available,
                    moved_places: HashMap::new(),
                    definitely_partial: HashSet::new(),
                    native_unit_discard: false,
                    mutable: false,
                    active_loans: BTreeSet::new(),
                    borrow_origin: None,
                },
            )
        })
        .collect()
}

#[test]
fn complete_prior_public_four_diagnostics_become_exact_unknown_and_independent_sibling() {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing,c:bool)->i64{b+missing_sibling}"),
        Path::new("public-template-recovery.spx"),
    )
    .unwrap();
    let old = crate::source_verify::declaration::public_signature_before::verify(&program);
    assert_eq!(
        codes(&old),
        ["SPX-T224", "SPX-T220", "SPX-T202", "SPX-T208"]
    );
    // Complete preceding public producer, not a hand-written receipt route.
    for repeated in [
        crate::source_verify::declaration::public_signature_before::verify_presenting(
            &program, None,
        ),
        crate::source_verify::declaration::public_signature_before::verify_selected(
            &program, None, None,
        ),
        crate::source_verify::declaration::public_signature_before::verify_partition(&program),
    ] {
        assert_eq!(repeated, old);
    }
    let current = crate::source_verify::verify(&program);
    assert_eq!(current, vec![old[1].clone(), old[2].clone()]);
}

#[test]
fn known_sibling_generic_profile_refusal_and_all_admitted_validation_stay_independent() {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing,p:Bytes)->i64{b+missing_sibling}"),
        Path::new("public-template-recovery.spx"),
    )
    .unwrap();
    let old = crate::source_verify::declaration::public_signature_before::verify(&program);
    let current = crate::source_verify::verify(&program);
    let known = old
        .iter()
        .find(|diagnostic| diagnostic.code == "SPX-T224" && diagnostic.message.contains("`f.p`"))
        .unwrap();
    assert!(current.contains(known));
    for diagnostic in &old {
        let dependent_profile =
            diagnostic.code == "SPX-T224" && diagnostic.message.contains("`f.b`");
        let dependent_operator = diagnostic.code == "SPX-T208";
        if !dependent_profile && !dependent_operator {
            assert!(
                current.contains(diagnostic),
                "independent full diagnostic lost: {diagnostic:?}"
            );
        }
    }
    for declaration in [
        "@id(\"t.f\") fn f<T>(b:T)->T{b}",
        "@id(\"t.f\") fn f<T,U,V>(b:i64)->i64{b}",
    ] {
        let program = crate::parse(
            &source(declaration),
            Path::new("public-template-recovery.spx"),
        )
        .unwrap();
        assert_eq!(
            crate::source_verify::verify(&program),
            crate::source_verify::declaration::public_signature_before::verify(&program)
        );
    }
}

#[test]
fn borrowed_producer_origin_rejects_foreign_stale_and_malformed_templates_without_allocation() {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing,c:bool)->i64{b+missing_sibling}"),
        Path::new("public-template-recovery.spx"),
    )
    .unwrap();
    let raw = &program.functions[0];
    let raw_view = FunctionRef::ordinary(raw);
    let current =
        crate::source_verify::declared_type::validation_specialize_function(raw, &[Type::I64])
            .unwrap();
    let foreign = program.clone();
    let copy = current.clone();
    let ((), metrics) = crate::test_allocation_observer::measure(|| {
        let held = origin(&program, &raw_view, &current).unwrap();
        assert!(std::ptr::eq(
            held.template_for(&program, &current).unwrap(),
            raw
        ));
        assert!(held.template_for(&foreign, &current).is_none());
        assert!(held.template_for(&program, &copy).is_none());
        assert!(origin(
            &program,
            &FunctionRef::ordinary(&foreign.functions[0]),
            &current
        )
        .is_none());
        assert!(origin(&program, &raw_view, raw).is_none());
    });
    assert!(!metrics.invalid);
    assert_eq!(
        (
            metrics.allocations,
            metrics.reallocations,
            metrics.allocated,
            metrics.freed,
            metrics.live
        ),
        (0, 0, 0, 0, 0)
    );
    for field in 0..4 {
        let mut malformed = program.clone();
        match field {
            0 => malformed.functions[0].type_parameters.clear(),
            1 => malformed.functions[0].type_parameters[0].name = "bad-name".to_owned(),
            2 => {
                let duplicate = malformed.functions[0].type_parameters[0].clone();
                malformed.functions[0].type_parameters.push(duplicate);
            }
            3 => malformed.functions[0].params[0].span.start += 1,
            _ => unreachable!(),
        }
        assert!(origin(
            &malformed,
            &FunctionRef::ordinary(&malformed.functions[0]),
            &current
        )
        .is_none());
    }
}

#[test]
fn specialized_public_receiver_keeps_exact_six_field_authentication() {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing,c:bool)->i64{b+missing_sibling}"),
        Path::new("public-template-recovery.spx"),
    )
    .unwrap();
    let raw = &program.functions[0];
    let raw_view = FunctionRef::ordinary(raw);
    let current =
        crate::source_verify::declared_type::validation_specialize_function(raw, &[Type::I64])
            .unwrap();
    let types = TypeTable::new(&program);
    let function_cells = crate::source_verify::capacity::source_capacity_functions(&program);
    let functions = function_cells
        .iter()
        .take(program.functions.len())
        .map(|(_, function)| (function.name(), function))
        .collect::<crate::source_verify::function_map::FunctionMap<'_>>();
    let mut receipt = Vec::new();
    crate::source_verify::declared_type::check_declared_type(
        &program,
        &raw.params[0].ty,
        raw.params[0].span,
        &types,
        &HashSet::from(["T"]),
        &mut receipt,
    );
    assert_eq!(codes(&receipt), ["SPX-T220"]);
    for field in 0..7 {
        let mut diagnostics = receipt.clone();
        match field {
            0 => diagnostics[0].code = "SPX-T001",
            1 => diagnostics[0].severity = crate::diagnostic::Severity::Warning,
            2 => diagnostics[0].message = "forged".into(),
            3 => diagnostics[0].path = Some("foreign.spx".to_owned()),
            4 => diagnostics[0].span = None,
            5 => diagnostics[0].help = Some("forged".to_owned()),
            6 => {}
            _ => unreachable!(),
        }
        let mut bindings = variables(&current);
        let (value, _) = crate::source_verify::iterative::check_expr_with_signature_view(
            &program,
            &current,
            &current.body,
            &mut bindings,
            &functions,
            &types,
            None,
            true,
            &mut diagnostics,
            None,
            None,
            origin(&program, &raw_view, &current),
        );
        assert!(value.is_none());
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T202"
                && diagnostic.message.contains("missing_sibling")));
        assert_eq!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-T208"),
            field != 6
        );
    }
}

#[test]
fn full_production_sources_and_complete_public_predecessors_keep_exact_inverses() {
    use sha2::{Digest, Sha256};
    let inverse: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/public-template-recovery-799/whole-source-inverses.json"
    )))
    .unwrap();
    let sources = [
        (ci_str!("../../../declaration.rs"), Some(ci_str!("../../public_signature_before.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/public-template-recovery-799/src__source_verify__declaration.rs.before.txt"))),
        (ci_str!("../../functions.rs"), Some(ci_str!("../public_signature_before.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/public-template-recovery-799/src__source_verify__declaration__functions.rs.before.txt"))),
        (ci_str!("../../../iterative.rs"), None, ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/public-template-recovery-799/src__source_verify__iterative.rs.before.txt"))),
        (ci_str!("../../../diagnostics.rs"), None, ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/public-template-recovery-799/src__source_verify__diagnostics.rs.before.txt"))),
        (ci_str!("../../../recovery.rs"), None, ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/public-template-recovery-799/src__source_verify__recovery.rs.before.txt"))),
        (ci_str!("../../../iterative/enter.rs"), None, ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/public-template-recovery-799/src__source_verify__iterative__enter.rs.before.txt"))),
        (ci_str!("../../../recovery_tests/extended_components.rs"), None, ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/public-template-recovery-799/src__source_verify__recovery_tests__extended_components.rs.before.txt"))),
    ];
    let fixed_original_sha256 = [
        "5c4987a9e83eb0f80c7c3d6211910e3c1f62cb91e60b9cc3b9d8f87c8000351f",
        "f8de677d0ee06872c4ddae8415eb84ca145c8ecda961a5379714451f55e3e5f2",
        "8bc16b846d6ce4b01d9bdbe73c15efca61b3d8c9b1ed13afc233f50ca748c241",
        "8fccfc4e16d3296712735a71ea5811a0d555e542fbd8719c208a24222b8c240e",
        "0e31278d627351d403bd616a2592572f4fc8c27bb38fcf799a09163adc172c14",
        "7566025fca09d0cec7839f7754a0d54ebbb3313b902617cc3a1d9d22b7516227",
        "b5dfa7ee66dd7d775b334184a0c6a570679641301d8687fbb2c1970d8c065c0a",
    ];
    for (index, (current, compiled_old, original)) in sources.into_iter().enumerate() {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            fixed_original_sha256[index]
        );
        let record = &inverse["files"][index];
        for (source, key) in [
            (Some(current), "current_splices"),
            (compiled_old, "compiled_old_splices"),
        ] {
            let Some(source) = source else { continue };
            let mut restored = source.to_owned();
            for splice in record[key].as_array().unwrap().iter().rev() {
                let start = splice["start"].as_u64().unwrap() as usize;
                let after = splice["after"].as_str().unwrap();
                assert_eq!(restored.get(start..start + after.len()), Some(after));
                restored.replace_range(
                    start..start + after.len(),
                    splice["before"].as_str().unwrap(),
                );
            }
            assert_eq!(restored, original);
        }
    }
}

#[test]
fn public_error_receipts_and_complete_drop_keep_zero_and_sticky_unpaid_boundary_explicit() {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing,c:bool)->i64{b+missing_sibling}"),
        Path::new("public-template-recovery.spx"),
    )
    .unwrap();
    let expected = crate::source_verify::verify(&program);
    assert_eq!(codes(&expected), ["SPX-T220", "SPX-T202"]);
    let old_expected = crate::source_verify::declaration::public_signature_before::verify(&program);
    assert_eq!(
        codes(&old_expected),
        ["SPX-T224", "SPX-T220", "SPX-T202", "SPX-T208"]
    );
    for poisoned in [false, true] {
        let (((), old_metrics), old_refused, old_paid, old_first) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                if poisoned {
                    assert!(!crate::bounded_output::reserve_active_required(1));
                }
                crate::test_allocation_observer::measure(|| {
                    let actual = crate::source_verify::declaration::public_signature_before::verify(
                        &program,
                    );
                    assert_eq!(actual, old_expected);
                    drop(actual);
                })
            });
        assert!(!old_metrics.invalid);
        assert!(old_metrics.allocated > 0);
        assert_eq!(old_metrics.allocated, old_metrics.freed);
        assert_eq!(old_metrics.live, 0);
        let (((), metrics), refused, paid, first) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                if poisoned {
                    assert!(!crate::bounded_output::reserve_active_required(1));
                }
                crate::test_allocation_observer::measure(|| {
                    let current = crate::source_verify::verify(&program);
                    assert_eq!(current, expected);
                    drop(current);
                })
            });
        assert_eq!((refused, paid, first), (old_refused, old_paid, old_first));
        assert_eq!(paid, 0);
        if poisoned {
            assert!(refused);
            assert_eq!(first.unwrap().requested, 1);
        }
        eprintln!("public-template-budget poisoned={poisoned} old_refused={old_refused} current_refused={refused} old_paid={old_paid} current_paid={paid} old_first={old_first:?} current_first={first:?}");
        assert!(!metrics.invalid);
        assert!(
            metrics.allocated > 0,
            "ordinary verifier storage remains unpaid"
        );
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(metrics.live, 0);
    }
}

#[test]
fn known_outer_generic_parameter_profile_survives_a_missing_nested_component() {
    for declaration in [
        "@id(\"t.pair\") record Pair<A>{@id(\"t.pair.value\") value:A,} @id(\"t.f\") fn f<T>(b:Pair<Missing>)->i64{0}",
        "@id(\"t.f\") fn f<T>(b:fn(Missing)->i64)->i64{0}",
    ] {
        let program = crate::parse(&source(declaration), Path::new("public-template-recovery.spx")).unwrap();
        let old = crate::source_verify::declaration::public_signature_before::verify(&program);
        let current = crate::source_verify::verify(&program);
        assert!(old.iter().any(|diagnostic| diagnostic.code == "SPX-T224"
            && diagnostic.message.contains("`f.b`")));
        assert!(old.iter().any(|diagnostic| diagnostic.code == "SPX-T220"));
        assert_eq!(current, old);
    }
}

#[test]
fn actual_generic_contract_entries_share_exact_template_origin_and_keep_independent_roots() {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing)->i64 requires b requires missing_req ensures b ensures missing_post {0}"),
        Path::new("public-template-recovery.spx"),
    ).unwrap();
    let old = crate::source_verify::declaration::public_signature_before::verify(&program);
    assert_eq!(
        codes(&old),
        ["SPX-T224", "SPX-T220", "SPX-C101", "SPX-T202", "SPX-C101", "SPX-T202"]
    );
    let current = crate::source_verify::verify(&program);
    assert_eq!(
        current,
        vec![old[1].clone(), old[3].clone(), old[5].clone()]
    );

    let program = crate::parse(
        &source(
            "@id(\"t.f\") fn f<T>(b:Missing)->i64 requires b requires 1 ensures b ensures 2 {0}",
        ),
        Path::new("public-template-recovery.spx"),
    )
    .unwrap();
    let raw = &program.functions[0];
    let old = crate::source_verify::declaration::public_signature_before::verify(&program);
    let expected = old
        .iter()
        .filter(|diagnostic| {
            !(diagnostic.code == "SPX-T224" && diagnostic.message.contains("`f.b`"))
                && !(diagnostic.code == "SPX-C101"
                    && (diagnostic.span == Some(raw.requires[0].span)
                        || diagnostic.span == Some(raw.ensures[0].span)))
        })
        .cloned()
        .collect::<Vec<_>>();
    let current = crate::source_verify::verify(&program);
    assert_eq!(current, expected);
    assert_eq!(
        current
            .iter()
            .filter(|diagnostic| diagnostic.code == "SPX-C101")
            .count(),
        2
    );
}

#[test]
fn actual_contract_receiver_rejects_six_field_forgeries_and_foreign_body_origins() {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing)->i64 requires b {0}"),
        Path::new("public-template-recovery.spx"),
    )
    .unwrap();
    let raw = &program.functions[0];
    let raw_view = FunctionRef::ordinary(raw);
    let current =
        crate::source_verify::declared_type::validation_specialize_function(raw, &[Type::I64])
            .unwrap();
    let types = TypeTable::new(&program);
    let function_cells = crate::source_verify::capacity::source_capacity_functions(&program);
    let functions = function_cells
        .iter()
        .take(program.functions.len())
        .map(|(_, function)| (function.name(), function))
        .collect::<crate::source_verify::function_map::FunctionMap<'_>>();
    let bindings = variables(&current);
    let mut receipt = Vec::new();
    crate::source_verify::declared_type::check_declared_type(
        &program,
        &raw.params[0].ty,
        raw.params[0].span,
        &types,
        &HashSet::from(["T"]),
        &mut receipt,
    );
    assert_eq!(codes(&receipt), ["SPX-T220"]);
    for field in 0..7 {
        let mut diagnostics = receipt.clone();
        match field {
            0 => diagnostics[0].code = "SPX-T001",
            1 => diagnostics[0].severity = crate::diagnostic::Severity::Warning,
            2 => diagnostics[0].message = "forged".into(),
            3 => diagnostics[0].path = Some("foreign.spx".to_owned()),
            4 => diagnostics[0].span = None,
            5 => diagnostics[0].help = Some("forged".to_owned()),
            6 => {}
            _ => unreachable!(),
        }
        crate::source_verify::diagnostics::require_bool_with_signature(
            &program,
            &current,
            &current.requires[0],
            &bindings,
            &functions,
            &types,
            None,
            &mut diagnostics,
            "precondition",
            origin(&program, &raw_view, &current),
        );
        assert_eq!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-C101"),
            field != 6
        );
    }
    let foreign = program.clone();
    let foreign_body = crate::source_verify::declared_type::validation_specialize_function(
        &foreign.functions[0],
        &[Type::I64],
    )
    .unwrap();
    let stale_body = current.clone();
    let foreign_view = FunctionRef::ordinary(&foreign.functions[0]);
    for (body, held) in [
        (&current, origin(&foreign, &foreign_view, &foreign_body)),
        (&stale_body, origin(&program, &raw_view, &current)),
    ] {
        let mut diagnostics = receipt.clone();
        crate::source_verify::diagnostics::require_bool_with_signature(
            &program,
            body,
            &body.requires[0],
            &bindings,
            &functions,
            &types,
            None,
            &mut diagnostics,
            "precondition",
            held,
        );
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-C101"));
    }
}

#[test]
fn actual_public_missing_generic_argument_keeps_all_primary_and_independent_contract_roots() {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing<T>)->i64 requires b requires missing_req ensures b ensures missing_post {b+missing_body}"),
        Path::new("public-template-recovery.spx"),
    ).unwrap();
    let old = crate::source_verify::declaration::public_signature_before::verify(&program);
    assert!(old.iter().any(|diagnostic| diagnostic.code == "SPX-T224"));
    assert!(old.iter().any(|diagnostic| diagnostic.code == "SPX-T208"));
    assert_eq!(
        old.iter()
            .filter(|diagnostic| diagnostic.code == "SPX-C101")
            .count(),
        2
    );
    let expected = old
        .iter()
        .filter(|diagnostic| matches!(diagnostic.code, "SPX-T220" | "SPX-T202"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        expected
            .iter()
            .filter(|diagnostic| diagnostic.code == "SPX-T220")
            .count(),
        1
    );
    assert_eq!(
        expected
            .iter()
            .filter(|diagnostic| diagnostic.code == "SPX-T202")
            .count(),
        3
    );
    assert_eq!(crate::source_verify::verify(&program), expected);
}

#[test]
fn actual_public_generic_return_origin_removes_only_dependent_profile_body_and_result_failures() {
    for result in ["Missing", "Missing<T>"] {
        let program = crate::parse(
            &source(&format!("@id(\"t.f\") fn f<T>()->{result} ensures result ensures 1 ensures missing_post {{1}}")),
            Path::new("public-template-recovery.spx"),
        ).unwrap();
        let raw = &program.functions[0];
        let old = crate::source_verify::declaration::public_signature_before::verify(&program);
        assert!(old.iter().any(|diagnostic| diagnostic.code == "SPX-T224"));
        assert!(old.iter().any(|diagnostic| diagnostic.code == "SPX-T103"));
        assert!(old.iter().any(|diagnostic| diagnostic.code == "SPX-C101"
            && diagnostic.span == Some(raw.ensures[0].span)));
        let expected = old
            .iter()
            .filter(|diagnostic| {
                diagnostic.code != "SPX-T224"
                    && diagnostic.code != "SPX-T103"
                    && !(diagnostic.code == "SPX-C101"
                        && diagnostic.span == Some(raw.ensures[0].span))
            })
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(codes(&expected), ["SPX-T220", "SPX-C101", "SPX-T202"]);
        assert_eq!(crate::source_verify::verify(&program), expected);
    }
    let program = crate::parse(
        &source("@id(\"t.pair\") record Pair<A>{@id(\"t.pair.value\") value:A,} @id(\"t.f\") fn f<T>()->Pair<Missing> ensures result {1}"),
        Path::new("public-template-recovery.spx"),
    ).unwrap();
    let old = crate::source_verify::declaration::public_signature_before::verify(&program);
    assert!(old.iter().any(|diagnostic| diagnostic.code == "SPX-T224"));
    assert_eq!(crate::source_verify::verify(&program), old);
}

#[test]
fn paired_origin_requires_actual_binding_and_exact_return_receipt_including_foreign_and_stale_cases(
) {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing<T>)->Missing<T> ensures result {b}"),
        Path::new("public-template-recovery.spx"),
    )
    .unwrap();
    let raw = &program.functions[0];
    let raw_view = FunctionRef::ordinary(raw);
    let current =
        crate::source_verify::declared_type::validation_specialize_function(raw, &[Type::I64])
            .unwrap();
    let types = TypeTable::new(&program);
    let mut receipt = Vec::new();
    crate::source_verify::declared_type::check_declared_type(
        &program,
        &raw.return_type,
        raw.span,
        &types,
        &HashSet::from(["T"]),
        &mut receipt,
    );
    assert_eq!(codes(&receipt), ["SPX-T220"]);
    for field in 0..7 {
        let mut forged = receipt.clone();
        match field {
            0 => forged[0].code = "SPX-T001",
            1 => forged[0].severity = crate::diagnostic::Severity::Warning,
            2 => forged[0].message = "forged".into(),
            3 => forged[0].path = Some("foreign.spx".to_owned()),
            4 => forged[0].span = None,
            5 => forged[0].help = Some("forged".to_owned()),
            6 => {}
            _ => unreachable!(),
        }
        assert_eq!(
            origin(&program, &raw_view, &current)
                .unwrap()
                .return_refusal(&program, &current, &types, &forged)
                .is_some(),
            field == 6
        );
    }
    let foreign = program.clone();
    let stale = current.clone();
    let held = origin(&program, &raw_view, &current).unwrap();
    assert!(held
        .return_refusal(&foreign, &current, &types, &receipt)
        .is_none());
    assert!(held
        .return_refusal(&program, &stale, &types, &receipt)
        .is_none());
    let mut parameter_receipt = Vec::new();
    crate::source_verify::declared_type::check_declared_type(
        &program,
        &raw.params[0].ty,
        raw.params[0].span,
        &types,
        &HashSet::from(["T"]),
        &mut parameter_receipt,
    );
    for altered in [false, true] {
        let mut bindings = variables(&current);
        if altered {
            bindings.get_mut("b").unwrap().ty = Type::I64;
        }
        let mut recovery = crate::source_verify::recovery::Recovery::signature_origin(
            &program,
            &current,
            &bindings,
            &types,
            &parameter_receipt,
            &held,
            None,
        )
        .unwrap();
        assert_eq!(recovery.reads_known_refused(0, "b"), !altered);
    }
}
