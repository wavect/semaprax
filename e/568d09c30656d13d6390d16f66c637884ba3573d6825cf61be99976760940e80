//! Whole constructors and later validation, including receiving Drop.
use super::*;
use crate::test_allocation_observer::measure;
use crate::workspace_graph::expected_projection::{call_carriers, shared_local_callees};

const SCALAR: &str = r#"module covered;
@id("covered.add") fn add(x:i64)->i64 {x+1}
@id("covered.main") fn main()->i64 ensures result >= 0 {
    let mut value=0; while value<16 {value=add(value);} value
}
"#;
fn counts(programs: &[Program]) -> super::super::retained_scratch::Counts {
    let authored = crate::workspace_graph::index_authored(programs).unwrap();
    call_carriers::count_modular(
        shared_local_callees::Context {
            program: &programs[0],
            authored: &authored,
            programs,
        },
        true,
    )
    .unwrap()
    .0
    .retained
}

#[test]
fn scalar_statements_loops_contracts_and_authenticated_calls_match_real_cfg_points() {
    for text in [
        SCALAR.to_owned(),
        SCALAR.replace("let mut value=0;", "let callable=add;let mut value=0;"),
    ] {
        let source = crate::check(&text, "covered.spx").unwrap();
        let census = counts(std::slice::from_ref(&source));
        assert_eq!(census.functions, 2);
        let resolved = crate::hir::resolve(&source).unwrap();
        crate::hir::validate(&resolved).unwrap();
        let mut actual = super::super::retained_scratch::Counts::default();
        let mut function_reference = false;
        let programs = std::slice::from_ref(&source);
        let indexed = crate::workspace_graph::index_authored(programs).unwrap();
        let maximum = Some(dependency_identity_max(&source, &indexed, programs).unwrap());
        let (full, retained) = shared_local_callees::modular_pair::selected_estimates(
            &source, &indexed, programs, maximum, 6, true, None,
        )
        .unwrap();
        assert_eq!(full.retained_scratch, census);
        assert_eq!(retained.retained_scratch, census);
        for (authored, function) in source.functions.iter().zip(&resolved.functions) {
            let mut spans = Vec::new();
            for root in super::super::call_identity::roots(authored) {
                assert!(super::super::call_identity::walk(root, &mut |expression| {
                    spans.push(expression.span);
                    true
                }));
            }
            let mut observed = Vec::new();
            let (nodes, points) =
                crate::loan_plan::owned_capacity::inspect_retention_cfg(function, |expression| {
                    observed.push(expression.span);
                    function_reference |= matches!(
                        expression.kind,
                        crate::hir::ResolvedExprKind::FunctionReference { .. }
                    );
                })
                .unwrap();
            assert_eq!(nodes, spans.len());
            assert_eq!(points, nodes.checked_mul(2).unwrap());
            for span in spans {
                assert_eq!(observed.iter().filter(|actual| **actual == span).count(), 1);
            }
            let independent = crate::loan_plan::build_plan(&resolved, function).unwrap();
            assert!(
                independent.loans.is_empty()
                    && independent.endpoints.is_empty()
                    && independent.edges.is_empty()
            );
            actual.functions += 1;
            actual.expressions += nodes;
            actual.maximum_function = actual.maximum_function.max(nodes);
        }
        assert_eq!(function_reference, text.contains("let callable=add;"));
        assert_eq!(actual, census);
        assert_eq!(
            census.bytes().unwrap(),
            actual.expressions * crate::loan_plan::REACHABILITY_BYTES_PER_EXPRESSION
        );
    }
}

#[test]
fn incomplete_function_generic_default_borrow_and_unknown_constructor_keep_whole_tariff() {
    for body in [
        "bytes_zeroed(1usize)",
        "{let x=bytes_zeroed(1usize);0}",
        "helper<i64>(0)",
    ] {
        let text = format!("module fallback; @id(\"fallback.main\") fn main()->i64 {{{body}}}");
        let source = crate::parse(&text, "fallback.spx").unwrap();
        assert_eq!(counts(&[source]).functions, 0, "{body}");
    }
    let source = crate::parse(
        "module fallback; @id(\"fallback.main\") fn main(x:borrow Slice<u8>)->i64 {0}",
        "borrow.spx",
    )
    .unwrap();
    assert_eq!(counts(&[source]).functions, 0);
    let generic = crate::parse(
        "module generic; @id(\"generic.main\") fn main<T>(x:i64)->i64 {x}",
        "generic.spx",
    )
    .unwrap();
    assert_eq!(counts(&[generic]).functions, 0);
    let text="module mixed; @id(\"mixed.safe\") fn safe()->i64 {1} @id(\"mixed.unknown\") fn unknown()->i64 {let x=bytes_zeroed(1usize);0}";
    let source = crate::parse(text, "mixed.spx").unwrap();
    assert_eq!(counts(&[source]).functions, 1);
}

#[test]
fn cross_module_peak_retains_later_max_directory_and_whole_drop_bounds() {
    let programs=(0..4).map(|index| {
        let body=(0..96).map(|_|"value=value+1;").collect::<String>();
        let text=format!("module m{index}; @id(\"m{index}.main\") fn main()->i64 {{let mut value=0;{body}value}}");
        crate::parse(&text,format!("m{index}.spx")).unwrap()
    }).collect::<Vec<_>>();
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let old =
        super::super::before_uncached_peak::uncached_output_layout(&programs, &authored).unwrap();
    let current = uncached_output_layout(&programs, &authored).unwrap();
    assert!(
        current.forecast < old.forecast,
        "old={} current={}",
        old.forecast,
        current.forecast
    );
    let mut retained = 0usize;
    let mut later = 0usize;
    let mut functions = 0usize;
    for program in &programs {
        let maximum = Some(dependency_identity_max(program, &authored, &programs).unwrap());
        let (_, cost) = shared_local_callees::modular_pair::selected_estimates(
            program, &authored, &programs, maximum, 6, true, None,
        )
        .unwrap();
        retained += cost.retained_hir - cost.retained_scratch.bytes().unwrap()
            + super::super::retained_scratch::RECEIVING;
        later = later.max(cost.retained_scratch.later().unwrap());
        functions += cost.retained_scratch.functions;
    }
    assert!(
        current.forecast
            >= retained + later + crate::hir::retained_scalar_directory_bound(functions).unwrap()
    );
    let ((retained_live, later_peak), allocation) = measure(|| {
        let all = programs
            .iter()
            .map(|program| crate::hir::resolve(program).unwrap())
            .collect::<Vec<_>>();
        let retained_live = crate::test_allocation_observer::current_metrics().live;
        for hir in &all {
            crate::hir::validate(hir).unwrap();
        }
        let later_peak = crate::test_allocation_observer::current_metrics().highwater;
        drop(all);
        (retained_live, later_peak)
    });
    assert!(retained_live > 0 && later_peak >= retained_live);
    assert_eq!(allocation.live, 0);
    assert!(allocation.highwater <= current.forecast);
    eprintln!("903_CLOSED_CONSTRUCTOR_PEAK old={} current={} retained={retained} later={later} directory={} whole_drop={allocation:?}; no application admission claim",old.forecast,current.forecast,crate::hir::retained_scalar_directory_bound(functions).unwrap());
}

#[test]
fn exact_short_zero_static_caps_preserve_g171_and_no_partial_layout() {
    let programs = (0..3)
        .map(|index| {
            let source = SCALAR
                .replace("module covered;", &format!("module cap{index};"))
                .replace("covered.", &format!("cap{index}."));
            crate::parse(&source, format!("cap{index}.spx")).unwrap()
        })
        .collect::<Vec<_>>();
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let expected = uncached_output_layout(&programs, &authored)
        .unwrap()
        .forecast;
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(self.0));
        }
    }
    let _restore =
        Restore(crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(expected)));
    assert_eq!(
        uncached_output_layout(&programs, &authored)
            .unwrap()
            .forecast,
        expected
    );
    for cap in [expected - 1, 0] {
        crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(cap));
        let failures = match uncached_output_layout(&programs, &authored) {
            Ok(_) => panic!("short cap published a layout"),
            Err(failures) => failures,
        };
        assert!(!failures.is_empty());
        assert!(failures.iter().all(|failure| failure.code == "SPX-G171"));
        if cap == expected - 1 {
            assert!(failures[0]
                .help
                .as_deref()
                .unwrap()
                .contains("final retained-validation forecast"));
        }
    }
}

#[test]
fn actual_generic_instances_and_imported_default_stubs_keep_complete_original_terms() {
    let generic = crate::check("module generic; @id(\"generic.id\") fn id<T>(value:T)->T {value} @id(\"generic.main\") fn main()->i64 {id<i64>(1)}", "generic.spx").unwrap();
    let resolved = crate::hir::resolve(&generic).unwrap();
    crate::hir::validate(&resolved).unwrap();
    assert_eq!(resolved.function_instances.len(), 1);
    assert_eq!(counts(&[generic]).functions, 0);
    let programs = vec![
        crate::parse("module consumer; use function @id(\"provider.f\") from provider as remote; @id(\"consumer.main\") fn main()->i64 {remote(1)}", "consumer.spx").unwrap(),
        crate::parse("module provider; @id(\"provider.f\") fn f(x:i64)->i64 requires x>=0 ensures result>=0 {x+1}", "provider.spx").unwrap(),
    ];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let synthetic = super::super::synthetic_program(&programs[0], &authored, &programs).unwrap();
    let stub = synthetic
        .functions
        .iter()
        .find(|function| function.name == "remote")
        .unwrap();
    assert!(matches!(stub.body.kind, crate::ast::ExprKind::Int(0)));
    assert!(stub.requires.is_empty() && stub.ensures.is_empty());
    let actual = crate::hir::resolve(&synthetic).unwrap();
    crate::hir::validate(&actual).unwrap();
    assert!(actual
        .functions
        .iter()
        .any(|function| function.name == "remote"));
    assert_eq!(
        counts(&programs).functions,
        0,
        "neither imported default nor its caller is a covered local constructor"
    );
}

#[test]
fn immediate_old_producer_has_its_own_original_proof_and_cost_abis() {
    use super::super::shared_local_callees::before903;
    assert_eq!(before903::OLD_PROOF_BYTES, 2 * std::mem::size_of::<usize>());
    assert_eq!(before903::OLD_COST_BYTES, 6 * std::mem::size_of::<usize>());
    assert!(std::mem::size_of::<call_carriers::Proof>() > before903::OLD_PROOF_BYTES);
    assert!(std::mem::size_of::<super::super::SyntheticBuilderCosts>() > before903::OLD_COST_BYTES);
}

#[test]
fn expression_scratch_term_is_the_existing_typed_two_point_ceiling() {
    let words = crate::loan_plan::MAX_LOANS_PER_FUNCTION_V1.div_ceil(u64::BITS as usize);
    let point = words * std::mem::size_of::<u64>()
        + 2 * std::mem::size_of::<u16>()
        + std::mem::size_of::<bool>();
    assert_eq!(
        crate::loan_plan::REACHABILITY_BYTES_PER_EXPRESSION,
        2 * point
    );
    assert_eq!(2 * point, 74);
}
