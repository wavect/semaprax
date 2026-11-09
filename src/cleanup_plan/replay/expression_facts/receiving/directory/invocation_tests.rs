use super::*;
use crate::test_allocation_observer::measure;

fn fixture() -> ResolvedProgram {
    let mut source = String::from("module invocation.statuses; ");
    for index in 0..12 {
        source.push_str(&format!(
            "@id(\"large{index}\") fn large{index}(x:i64)->i64 {{"
        ));
        for local in 0..128 {
            let input = if local == 0 {
                "x".to_owned()
            } else {
                format!("value{}", local - 1)
            };
            source.push_str(&format!("let value{local}={input}+1;"));
        }
        source.push_str("value127} ");
        source.push_str(&format!(
            "@id(\"small{index}\") fn small{index}(x:i64)->i64 {{x+1}} "
        ));
    }
    crate::hir::resolve(&crate::check(&source, "invocation-statuses.spx").unwrap()).unwrap()
}
fn call(program: &ResolvedProgram, old: bool) -> Result<(), Diagnostic> {
    if old {
        crate::cleanup_plan::replay::fact_preflight::before_status_backing_validate_program_context(
            program,
        )
    } else {
        crate::cleanup_plan::replay::validate_program_context(program)
    }
}
fn run(program: &ResolvedProgram, old: bool, cap: usize, floor: usize, success: bool) -> usize {
    let (record, exhausted, paid, refusal) =
        crate::bounded_output::with_limit_usage_refusal(cap, || {
            assert!(crate::bounded_output::set_active_floor(floor));
            measure(|| {
                let actual = call(program, old);
                assert_eq!(
                    actual.is_ok(),
                    success,
                    "old={old} cap={cap} floor={floor}: {actual:?}"
                );
                drop(actual);
            })
            .1
        });
    assert!(!record.invalid && record.live == 0 && record.allocated == record.freed);
    assert!(record.allocated <= paid);
    assert_eq!(exhausted, !success);
    assert_eq!(refusal.is_some(), !success);
    eprintln!("INVOCATION_STATUS_BACKING old={old} cap={cap} floor={floor} paid={paid} {record:?}");
    paid
}
#[test]
fn full_program_status_heavy_old_current_net_exact_short_floor_and_drop() {
    let program = fixture();
    assert!(program
        .functions
        .iter()
        .any(|f| f.cleanup_plan.status_sources.len() >= 128));
    let old = run(&program, true, usize::MAX, 0, true);
    let current = run(&program, false, usize::MAX, 0, true);
    assert!(
        current < old,
        "complete additional owner/caller/reset fees must earn a real whole-caller net"
    );
    for (before, exact) in [(false, current), (true, old)] {
        assert_eq!(run(&program, before, exact, 0, true), exact);
        run(&program, before, exact - 1, 0, false);
        run(&program, before, exact, 1, false);
        run(&program, before, 0, 0, false);
    }
}
#[test]
fn tiny_cold_owner_cost_is_reported_without_claiming_a_gain() {
    let program = crate::hir::resolve(
        &crate::check(
            "module status.cold; @id(\"main\") fn main()->i64{1}",
            "status-cold.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let old = run(&program, true, usize::MAX, 0, true);
    let current = run(&program, false, usize::MAX, 0, true);
    assert!(
        current > old,
        "new complete invocation transport is a real cold cost"
    );
}
#[test]
fn every_directory_rebuild_matches_original_and_returns_no_keys_or_frozen_index() {
    let program = fixture();
    let (_, exhausted, _, refusal) =
        crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            budget.prepare_builder(&program.functions[0]).unwrap();
            budget
                .reserve_bytes(
                    &program.functions[0],
                    Backing::ACQUISITION_BYTES,
                    "status test acquisition",
                )
                .unwrap();
            let mut owner = Backing::acquire(&program, &program.functions[0], &mut budget).unwrap();
            let mut capacity = 0;
            for function in &program.functions {
                let mut directory = owner.directory(&program, function, &mut budget).unwrap();
                assert!(
                    directory.nodes.is_empty()
                        && directory.root == NIL
                        && directory.first == NIL
                        && directory.lookup.is_none()
                );
                assert!(directory.nodes.capacity() >= capacity);
                {
                    let mut prefix = directory.held_prefix(function, &mut budget).unwrap();
                    for source in &function.cleanup_plan.status_sources {
                        assert!(prefix.insert(function, &source.id, &mut budget).unwrap());
                    }
                    prefix.freeze(function, &mut budget).unwrap();
                }
                assert!(directory.lookup.is_some());
                let mut ordinary = Directory::new(
                    function,
                    &mut budget,
                    "complete independent original status directory",
                )
                .unwrap();
                for source in &function.cleanup_plan.status_sources {
                    assert!(ordinary.insert(function, &source.id, &mut budget).unwrap());
                }
                assert!(directory.balanced() && ordinary.balanced());
                assert!(directory.iter().eq(ordinary.iter()));
                capacity = directory.nodes.capacity();
                drop(ordinary);
                owner.recycle(directory);
                assert!(owner.empty.as_ref().unwrap().is_empty());
                assert_eq!(owner.empty.as_ref().unwrap().capacity(), capacity);
                owner.complete(&program, function, &mut budget).unwrap();
            }
            drop(owner);
        });
    assert!(!exhausted && refusal.is_none());
}
#[test]
fn later_duplicate_status_is_rebuilt_with_complete_old_diagnostic() {
    let mut program = fixture();
    let sources = &mut program.functions[2].cleanup_plan.status_sources;
    assert!(sources.len() >= 2);
    sources[1].id = sources[0].id.clone();
    let old =
        crate::bounded_output::with_limit_usage(usize::MAX, || call(&program, true).unwrap_err()).0;
    let current =
        crate::bounded_output::with_limit_usage(usize::MAX, || call(&program, false).unwrap_err())
            .0;
    assert_eq!(current, old);
}
#[test]
fn foreign_budget_and_scope_cannot_consume_held_empty_backing_or_buy_new_fees() {
    let program = fixture();
    let function = &program.functions[0];
    let (_, exhausted, _, refusal) =
        crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            budget.prepare_builder(function).unwrap();
            budget
                .reserve_bytes(
                    function,
                    Backing::ACQUISITION_BYTES,
                    "status test acquisition",
                )
                .unwrap();
            let mut owner = Backing::acquire(&program, function, &mut budget).unwrap();
            let mut directory = owner.directory(&program, function, &mut budget).unwrap();
            for source in &function.cleanup_plan.status_sources {
                assert!(directory.insert(function, &source.id, &mut budget).unwrap());
            }
            owner.recycle(directory);
            owner.complete(&program, function, &mut budget).unwrap();
            let function = &program.functions[1];
            let capacity = owner.empty.as_ref().unwrap().capacity();
            let pointer = owner.empty.as_ref().unwrap().as_ptr();
            assert!(capacity > 0);
            let mut foreign = ReplayBudget::new();
            assert!(owner.directory(&program, function, &mut foreign).is_err());
            let (failed, foreign_exhausted, used) =
                crate::bounded_output::with_limit_usage(1 << 20, || {
                    owner.directory(&program, function, &mut budget).is_err()
                });
            assert!(failed && !foreign_exhausted && used == 0);
            let held = owner.ledger.first_refusal();
            assert!(owner.directory(&program, function, &mut budget).is_err());
            assert_eq!(owner.ledger.first_refusal(), held);
            assert!(owner.empty.as_ref().unwrap().is_empty());
            assert_eq!(owner.empty.as_ref().unwrap().capacity(), capacity);
            assert_eq!(owner.empty.as_ref().unwrap().as_ptr(), pointer);
        });
    assert!(exhausted && refusal.is_some());
}

#[test]
fn source_callback_rechecks_affinity_before_any_new_directory_payment() {
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
                    let result = owner.directory(&context, function, &mut budget);
                    assert!(result.is_err());
                    drop(result);
                    assert!(context.fired.get());
                } else {
                    let (failed, exhausted, used) =
                        crate::bounded_output::with_limit_usage(1 << 20, || {
                            owner.directory(&program, function, &mut budget).is_err()
                        });
                    assert!(failed && !exhausted);
                    assert_eq!(used, 0, "affinity precedes every new positive debit");
                }
                assert!(owner.empty.is_none());
            });
        assert!(exhausted && refusal.is_some());
    }
}

#[test]
fn directory_constructor_exact_short_floor_and_sticky_keep_backing_unconsumed() {
    let program = fixture();
    let function = &program.functions[0];
    let observe = |cap, floor| {
        crate::bounded_output::with_limit_usage_refusal(cap, || {
            assert!(crate::bounded_output::set_active_floor(floor));
            let mut budget = ReplayBudget::new();
            let acquired = budget.prepare_builder(function).and_then(|()| {
                budget.reserve_bytes(
                    function,
                    Backing::ACQUISITION_BYTES,
                    "status exact constructor gateway",
                )?;
                Backing::acquire(&program, function, &mut budget)
            });
            if let Ok(mut owner) = acquired {
                match owner.directory(&program, function, &mut budget) {
                    Ok(directory) => drop(directory),
                    Err(first) => {
                        let held = owner.ledger.first_refusal();
                        assert!(owner.directory(&program, function, &mut budget).is_err());
                        assert_eq!(owner.ledger.first_refusal(), held);
                        assert!(owner.empty.is_none());
                        drop(first);
                    }
                }
            }
        })
    };
    let ample = observe(usize::MAX, 0);
    assert!(!ample.1 && ample.3.is_none());
    assert!(!observe(ample.2, 0).1);
    assert!(observe(ample.2 - 1, 0).1);
    assert!(observe(ample.2, 1).1);
    assert!(observe(0, 0).1);
}

#[test]
fn complete_immediate_sources_and_new_children_are_bound_without_reducing_old_coverage() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |path: &str, expected: &str| {
        assert!(expected.len() <= 2_097_152);
        let mut actual = Vec::new();
        std::fs::File::open(root.join(path))
            .unwrap()
            .take(u64::try_from(expected.len() + 1).unwrap())
            .read_to_end(&mut actual)
            .unwrap();
        assert_eq!(actual, expected.as_bytes());
    };
    let digest = |raw: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(raw)));
    let packet = include_str!("invocation_source_inverse.json");
    let packet_path = "src/cleanup_plan/replay/expression_facts/receiving/directory/invocation_source_inverse.json";
    read(packet_path, packet);
    let data: serde_json::Value = serde_json::from_str(packet).unwrap();
    let independent = include_str!("../retained/ordinary_invocation.rs");
    assert_eq!(
        data["independent_fact_only_driver"]["length"],
        independent.len()
    );
    assert_eq!(
        data["independent_fact_only_driver"]["sha256"],
        digest(independent.as_bytes())
    );
    let existing = [
        (
            "src/cleanup_plan/replay/fact_preflight.rs",
            include_str!("../../../fact_preflight.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving.rs",
            include_str!("../../receiving.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/retained.rs",
            include_str!("../retained.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/directory.rs",
            include_str!("../directory.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/arena/invocation_tests.rs",
            include_str!("../../arena/invocation_tests.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/arena/invocation_source_inverse.json",
            include_str!("../../arena/invocation_source_inverse.json"),
        ),
        (
            "docs/RETAINED-EXPRESSION-FACT-LOOKUP-V1.md",
            include_str!("../../../../../../docs/RETAINED-EXPRESSION-FACT-LOOKUP-V1.md"),
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
            let a = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
            let b = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
            assert_eq!(&restored[a..b], edit["current"].as_str().unwrap());
            restored.replace_range(a..b, edit["before"].as_str().unwrap());
        }
        assert_eq!(row["before"]["length"], restored.len());
        assert_eq!(row["before"]["sha256"], digest(restored.as_bytes()));
        if path == "src/cleanup_plan/replay/fact_preflight.rs" {
            let begin = restored
                .find("pub(in crate::cleanup_plan) fn validate_program_context(")
                .unwrap();
            let body = &restored[begin..];
            let body = &body[body.find('{').unwrap()..body.find("\n#[cfg(test)]").unwrap()];
            assert_eq!(
                compiled.matches(body).count(),
                1,
                "complete immediate fact-only program caller"
            );
        }
        read(path, compiled);
    }
    for (path, compiled) in [
        (packet_path, packet),
        ("src/cleanup_plan/replay/expression_facts/receiving/directory/invocation.rs", include_str!("invocation.rs")),
        ("src/cleanup_plan/replay/expression_facts/receiving/directory/invocation_tests.rs", include_str!("invocation_tests.rs")),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_status_invocation.rs", include_str!("../retained/ordinary_status_invocation.rs")),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_invocation.rs", include_str!("../retained/ordinary_invocation.rs")),
    ] { read(path, compiled); }
    read(packet_path, packet);
}
