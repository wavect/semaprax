use super::*;
use crate::test_allocation_observer::measure;

fn fixture() -> ResolvedProgram {
    let mut source = String::from("module invocation.facts; ");
    for index in 0..16 {
        source.push_str(&format!("@id(\"large{index}\") fn large{index}()->i64 {{"));
        for local in 0..64 {
            source.push_str(&format!("let value{local}={local};"));
        }
        source.push_str("value63} ");
        source.push_str(&format!(
            "@id(\"small{index}\") fn small{index}()->i64 {{0}} "
        ));
    }
    crate::hir::resolve(&crate::check(&source, "invocation-facts.spx").unwrap()).unwrap()
}
fn run(program: &ResolvedProgram, old: bool, cap: usize, success: bool) -> usize {
    let (record, exhausted, paid, refusal) = crate::bounded_output::with_limit_usage_refusal(
        cap,
        || {
            measure(|| {
                let actual = if old {
                    crate::cleanup_plan::replay::fact_preflight::before_invocation_validate_program_context(program)
                } else {
                    crate::cleanup_plan::replay::validate_program_context(program)
                };
                assert_eq!(actual.is_ok(), success, "old={old} cap={cap}: {actual:?}");
                drop(actual);
            }).1
        },
    );
    assert!(!record.invalid && record.live == 0);
    assert_eq!(record.allocated, record.freed);
    assert!(record.allocated <= paid);
    assert_eq!(exhausted, !success);
    assert_eq!(refusal.is_some(), !success);
    eprintln!("INVOCATION_FACT_BACKING old={old} cap={cap} paid={paid} {record:?}");
    paid
}

#[test]
fn complete_program_old_current_alternating_large_small_and_full_drop_keep_real_net() {
    let program = fixture();
    let old = run(&program, true, usize::MAX, true);
    let current = run(&program, false, usize::MAX, true);
    assert!(
        current < old,
        "all invocation ownership/reset/transport fees must earn a whole-caller net"
    );
    for (old, exact) in [(false, current), (true, old)] {
        assert_eq!(run(&program, old, exact, true), exact);
        run(&program, old, exact - 1, false);
        run(&program, old, 0, false);
    }
}

#[test]
fn every_rebuilt_fact_and_empty_return_match_the_complete_immediate_producer() {
    let program = fixture();
    let originals = program
        .functions
        .iter()
        .map(|function| {
            let mut budget = ReplayBudget::unobserved();
            let mut physical = physical_storage::PhysicalReservation::new(usize::MAX);
            super::super::build::<errors::Legacy>(
                function,
                capacity::Payer::Selected(&mut physical),
                &mut budget,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let (_, exhausted, _, refusal) =
        crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            budget.prepare_builder(&program.functions[0]).unwrap();
            budget
                .reserve_bytes(
                    &program.functions[0],
                    Backing::ACQUISITION_BYTES,
                    "invocation test exact acquisition",
                )
                .unwrap();
            let mut owner = Backing::acquire(&program, &program.functions[0], &mut budget).unwrap();
            for (function, original) in program.functions.iter().zip(&originals) {
                budget
                    .reserve_bytes(
                        function,
                        Backing::CALL_BYTES,
                        "invocation test receiving tuple",
                    )
                    .unwrap();
                let current = with_backing(function, &program, &mut budget, &mut owner).unwrap();
                let Facts::Arena(arena) = &*current else {
                    panic!("ordinary arena owner")
                };
                assert!(arena.balanced());
                assert_eq!(arena.len(), original.len());
                for ((key, fact), (old_key, old_fact)) in arena.entries().zip(original.entries()) {
                    assert_eq!(key, old_key);
                    match (fact, old_fact) {
                        (Some(fact), Some(old)) => {
                            assert_eq!(fact.callee, old.callee);
                            assert_eq!(fact.instance, old.instance);
                            assert_eq!(fact.arguments, old.arguments);
                            assert_eq!(fact.type_arguments, old.type_arguments);
                        }
                        (None, None) => (),
                        _ => panic!("complete fact presence changed"),
                    }
                }
                drop(current);
                let (arena, frames) = owner.empty.as_ref().unwrap();
                assert!(arena.nodes.is_empty() && arena.root == NIL && arena.first == NIL);
                assert!(frames.iter().all(Option::is_none));
            }
            drop(owner);
        });
    assert!(!exhausted && refusal.is_none());
}

#[test]
fn later_duplicate_id_is_rebuilt_and_foreign_budget_cannot_take_empty_backing() {
    let mut program = fixture();
    let duplicate = program.functions[1].body.clone();
    program.functions[1].ensures.push(duplicate);
    let function = &program.functions[1];
    let old = {
        let mut physical = physical_storage::PhysicalReservation::new(usize::MAX);
        let mut budget = ReplayBudget::unobserved();
        super::super::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical),
            &mut budget,
        )
        .err()
        .unwrap()
    };
    let (_, exhausted, _, refusal) =
        crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            budget.prepare_builder(&program.functions[0]).unwrap();
            budget
                .reserve_bytes(
                    &program.functions[0],
                    Backing::ACQUISITION_BYTES,
                    "invocation test exact acquisition",
                )
                .unwrap();
            let mut owner = Backing::acquire(&program, &program.functions[0], &mut budget).unwrap();
            drop(with_backing(&program.functions[0], &program, &mut budget, &mut owner).unwrap());
            let actual = with_backing(function, &program, &mut budget, &mut owner)
                .err()
                .unwrap();
            assert_eq!(actual, old);
            assert!(
                owner.empty.is_none(),
                "failed rebuild drops all acquired backing"
            );
            drop(actual);
            let mut foreign = ReplayBudget::new();
            assert!(
                with_backing(&program.functions[0], &program, &mut foreign, &mut owner).is_err()
            );
            drop(owner);
        });
    // Wrong-budget authentication keeps its own sticky replay failure without
    // spending or marking the held ordinary owner as a byte refusal.
    assert!(!exhausted && refusal.is_none());
}

#[test]
fn exact_short_sticky_reset_acquisition_never_returns_partially_cleared_storage() {
    let program = fixture();
    let function = &program.functions[0];
    let observe = |cap| {
        crate::bounded_output::with_limit_usage_refusal(cap, || {
            let mut budget = ReplayBudget::new();
            let acquired = budget.prepare_builder(function).and_then(|()| {
                budget.reserve_bytes(
                    function,
                    Backing::ACQUISITION_BYTES,
                    "invocation test exact acquisition",
                )?;
                Backing::acquire(&program, function, &mut budget)
            });
            if let Ok(mut owner) = acquired {
                let actual = with_backing(function, &program, &mut budget, &mut owner);
                match actual {
                    Ok(facts) => drop(facts),
                    Err(first) => {
                        let before = owner.ledger.first_refusal();
                        let retry = with_backing(function, &program, &mut budget, &mut owner);
                        assert!(retry.is_err());
                        drop(retry);
                        assert_eq!(owner.ledger.first_refusal(), before);
                        drop(first);
                        assert!(owner.empty.is_none());
                    }
                }
            }
        })
    };
    let ample = observe(usize::MAX);
    assert!(!ample.1 && ample.3.is_none());
    assert!(!observe(ample.2).1);
    assert!(observe(ample.2 - 1).1);
    assert!(observe(0).1);
}

#[test]
fn complete_immediate_sources_old_program_caller_and_new_children_are_source_bound() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let packet = include_str!("invocation_source_inverse.json");
    let data: serde_json::Value = serde_json::from_str(packet).unwrap();
    let read = |path: &str, expected: &str| {
        assert!(expected.len() <= 2 * 1024 * 1024);
        let mut raw = Vec::new();
        std::fs::File::open(root.join(path))
            .unwrap()
            .take(u64::try_from(expected.len() + 1).unwrap())
            .read_to_end(&mut raw)
            .unwrap();
        assert_eq!(raw, expected.as_bytes());
        raw
    };
    let digest = |raw: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(raw)));
    let packet_path =
        "src/cleanup_plan/replay/expression_facts/arena/invocation_source_inverse.json";
    read(packet_path, packet);
    let existing = [
        (
            "src/cleanup_plan/replay.rs",
            include_str!("../../../replay.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts.rs",
            include_str!("../../expression_facts.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/arena.rs",
            include_str!("../arena.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/capacity.rs",
            include_str!("../capacity.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving.rs",
            include_str!("../receiving.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/retained.rs",
            include_str!("../receiving/retained.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/retained/last_use.rs",
            include_str!("../receiving/retained/last_use.rs"),
        ),
    ];
    assert_eq!(data["owners"].as_array().unwrap().len(), existing.len());
    for (path, compiled) in existing {
        read(path, compiled);
        let rows = data["owners"].as_array().unwrap();
        assert_eq!(rows.iter().filter(|row| row["path"] == path).count(), 1);
        let row = rows.iter().find(|row| row["path"] == path).unwrap();
        assert_eq!(row["current"]["length"], compiled.len());
        assert_eq!(row["current"]["sha256"], digest(compiled.as_bytes()));
        let mut restored = compiled.to_owned();
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
            let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
            assert_eq!(&restored[start..end], edit["current"].as_str().unwrap());
            restored.replace_range(start..end, edit["before"].as_str().unwrap());
        }
        assert_eq!(row["before"]["length"], restored.len());
        assert_eq!(row["before"]["sha256"], digest(restored.as_bytes()));
        if path == "src/cleanup_plan/replay.rs" {
            let old = &restored[restored
                .find("pub(in crate::cleanup_plan) fn validate_program_context(")
                .unwrap()..];
            let end = old.find("\nfn reserve_program_skeleton_work").unwrap();
            let body = &old[old.find('{').unwrap()..end];
            assert_eq!(
                include_str!("../../fact_preflight.rs")
                    .matches(body)
                    .count(),
                1,
                "entire immediate caller body retained independently"
            );
        }
        read(path, compiled);
    }
    for (path, compiled) in [
        (
            "src/cleanup_plan/replay/fact_preflight.rs",
            include_str!("../../fact_preflight.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/arena/invocation.rs",
            include_str!("invocation.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/arena/invocation_tests.rs",
            include_str!("invocation_tests.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_invocation.rs",
            include_str!("../receiving/retained/ordinary_invocation.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_status_invocation.rs",
            include_str!("../receiving/retained/ordinary_status_invocation.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/directory/invocation.rs",
            include_str!("../receiving/directory/invocation.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/directory/invocation_source_inverse.json",
            include_str!("../receiving/directory/invocation_source_inverse.json"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/directory/invocation_tests.rs",
            include_str!("../receiving/directory/invocation_tests.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase.rs",
            include_str!("../receiving/retained/ordinary_cross_phase.rs"),
        ),
    ] {
        read(path, compiled);
    }
    read(packet_path, packet);
}

#[test]
fn lexical_functions_skip_fact_construction_without_skipping_program_sequence() {
    let program = crate::hir::resolve(
        &crate::check(
            r#"module invocation.lexical;
@id("first") fn first()->i64{1}
@id("exit") fn exit(flag:bool)->i64{while flag {break;} return 7;}
@id("last") fn last()->i64{2}
"#,
            "invocation-lexical.spx",
        )
        .unwrap(),
    )
    .unwrap();
    assert!(program.functions.iter().any(lexical_cert::requires_lexical));
    run(&program, true, usize::MAX, true);
    run(&program, false, usize::MAX, true);
}

#[test]
fn foreign_invocation_and_callback_refusal_never_buy_new_receiving_bank() {
    use crate::cleanup::ReplayContext;
    struct Callback<'a> {
        program: &'a ResolvedProgram,
        witness: &'a ClassifierLedger,
        fired: std::cell::Cell<bool>,
    }
    impl ReplayContext for Callback<'_> {
        fn declarations(&self) -> &crate::hir::DeclarationIndex {
            &self.program.declarations
        }
        fn types(&self) -> crate::hir::items::View<'_, crate::hir::ResolvedTypeDeclaration> {
            crate::hir::items::View::slice(&self.program.types)
        }
        fn interfaces(&self) -> &[crate::hir::ResolvedInterface] {
            &self.program.interfaces
        }
        fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction> {
            crate::hir::items::View::slice(&self.program.functions)
        }
        fn function_templates(&self) -> &[crate::hir::ResolvedFunctionTemplate] {
            &self.program.function_templates
        }
        fn function_instances(&self) -> &[crate::hir::ResolvedFunctionInstance] {
            &self.program.function_instances
        }
        fn full_program(&self) -> Option<&ResolvedProgram> {
            assert!(!self.fired.replace(true), "one actual source callback");
            let (same, exhausted, used) =
                crate::bounded_output::with_limit_usage(1 << 20, || self.witness.current());
            assert!(!same && !exhausted);
            assert_eq!(used, 0, "foreign scope pays no new owner control");
            Some(self.program)
        }
    }
    let program = fixture();
    let function = &program.functions[0];
    for callback in [false, true] {
        let (_, exhausted, _, refusal) =
            crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                let mut budget = ReplayBudget::new();
                budget.prepare_builder(function).unwrap();
                budget
                    .reserve_bytes(
                        function,
                        Backing::ACQUISITION_BYTES
                            + 4 * std::mem::size_of::<ClassifierLedger>()
                            + ClassifierLedger::capture_into_controls()
                            + ClassifierLedger::current_receiving_controls(),
                        "invocation test complete caller and callback witness",
                    )
                    .unwrap();
                let mut owner = Backing::acquire(&program, function, &mut budget).unwrap();
                let witness = ClassifierLedger::capture();
                if callback {
                    let context = Callback {
                        program: &program,
                        witness: &witness,
                        fired: std::cell::Cell::new(false),
                    };
                    let result = with_backing(function, &context, &mut budget, &mut owner);
                    assert!(result.is_err());
                    drop(result);
                    assert!(context.fired.get());
                } else {
                    let (failed, exhausted, used) =
                        crate::bounded_output::with_limit_usage(1 << 20, || {
                            with_backing(function, &program, &mut budget, &mut owner).is_err()
                        });
                    assert!(failed && !exhausted);
                    assert_eq!(used, 0, "affinity precedes every new positive debit");
                }
                assert!(owner.empty.is_none());
            });
        assert!(exhausted && refusal.is_some());
    }
}
