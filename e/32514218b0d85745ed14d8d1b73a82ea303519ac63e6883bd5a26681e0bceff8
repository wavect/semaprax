use super::*;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module retained.lookup; @id(\"lookup.main\") fn main()->i64 {0}",
            "retained-lookup.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn arena<'hir>(function: &'hir ResolvedFunction, keys: &[ExpressionId]) -> Arena<'hir> {
    let mut physical = physical_storage::PhysicalReservation::new(usize::MAX);
    let mut capacity =
        capacity::FactCapacity::with_payer(function, capacity::Payer::Selected(&mut physical))
            .unwrap();
    let mut arena = Arena::new(&mut capacity).unwrap();
    let mut budget = ReplayBudget::unobserved();
    for key in keys {
        arena.prepare_growth_work(function, &mut budget).unwrap();
        arena.reserve_next(&mut capacity).unwrap();
        arena
            .prepare_insert_work(function, key, &mut budget)
            .unwrap();
        assert!(arena.insert(key.clone(), None, &mut capacity).unwrap());
    }
    assert!(arena.balanced());
    arena
}
fn acquire<'a, 'hir, 'b>(
    arena: &'a Arena<'hir>,
    function: &'a ResolvedFunction,
    budget: &'b mut ReplayBudget,
) -> Result<Session<'a, 'hir, 'b>, Diagnostic> {
    budget.reserve_bytes(
        function,
        Session::OLD_SCRATCH + Session::acquisition_bytes(),
        "retained expression-fact owner acquisition",
    )?;
    budget.charge(
        function,
        12,
        "retained expression-fact owner initialization",
    )?;
    Ok(Session::new_prepaid(arena, function, budget))
}

#[test]
fn alternating_queries_preserve_complete_borrowed_result_and_old_work_on_all_shapes() {
    let program = program();
    let function = &program.functions[0];
    let mut keys = vec!["", "\0", "a", "a\0", "aa", "λ", "識別", "🙂"]
        .into_iter()
        .map(|s| ExpressionId::from_owned(s.into()))
        .collect::<Vec<_>>();
    for n in 0..67 {
        keys.push(ExpressionId::from_owned(format!("shared.λ\0.{n:03}")));
    }
    for count in [0, 1, 2, 15, 16, keys.len()] {
        for reverse in [false, true] {
            let mut order = keys[..count].to_vec();
            if reverse {
                order.reverse();
            }
            let arena = arena(function, &order);
            let queries = keys
                .iter()
                .chain(keys.iter().rev())
                .map(|key| ExpressionId::from_owned(key.as_str().to_owned()))
                .collect::<Vec<_>>();
            let mut budget = ReplayBudget::unobserved();
            let mut session = acquire(&arena, function, &mut budget).unwrap();
            for query in &queries {
                let mut old_budget = ReplayBudget::unobserved();
                let before = session.budget.remaining;
                let old = prefix::retained_before::find_receiving(
                    &arena,
                    function,
                    query,
                    &mut old_budget,
                )
                .unwrap();
                let current = session.fact(query).unwrap();
                assert_eq!(current.is_some(), old.is_some());
                if let (Some(current), Some(old)) = (current, old) {
                    assert!(
                        std::ptr::eq(current, old),
                        "complete payload and present noncall are unchanged"
                    );
                }
                assert_eq!(
                    before - session.budget.remaining,
                    MAX_REPLAY_WORK_UNITS - old_budget.remaining
                );
            }
        }
    }
}

#[test]
fn every_old_query_work_cut_keeps_all_six_diagnostic_fields() {
    let program = program();
    let function = &program.functions[0];
    let keys = (0..33)
        .map(|n| ExpressionId::from_owned(format!("prefix.{n:03}")))
        .collect::<Vec<_>>();
    let arena = arena(function, &keys);
    for query in [&keys[0], &keys[17], &keys[32]] {
        let mut complete = ReplayBudget::unobserved();
        prefix::retained_before::find_receiving(&arena, function, query, &mut complete).unwrap();
        for work in 0..=MAX_REPLAY_WORK_UNITS - complete.remaining {
            let mut old_budget = ReplayBudget::unobserved();
            old_budget.remaining = work;
            let old =
                prefix::retained_before::find_receiving(&arena, function, query, &mut old_budget);
            let mut budget = ReplayBudget::unobserved();
            let mut session = acquire(&arena, function, &mut budget).unwrap();
            session.budget.remaining = work;
            let current = session.fact(query);
            assert_eq!(
                current.as_ref().err(),
                old.as_ref().err(),
                "all six fields at work {work}"
            );
            assert_eq!(current.is_ok(), old.is_ok());
            assert_eq!(session.budget.remaining, old_budget.remaining);
        }
    }
}

#[test]
fn exact_short_first_and_later_query_payments_are_sticky_without_allocation_or_refund() {
    let program = program();
    let function = &program.functions[0];
    let arena = arena(function, &[function.body.id.clone()]);
    let observe = |bytes, queries| {
        let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
        let result = (|| {
            let mut session = acquire(&arena, function, &mut budget)?;
            for _ in 0..queries {
                session.fact(&function.body.id)?;
            }
            Ok::<_, Diagnostic>(())
        })();
        let paid = budget.physical.as_ref().unwrap().charged();
        if result.is_err() {
            let remaining = budget.remaining;
            assert!(budget.reserve_bytes(function, 0, "sticky retry").is_err());
            assert_eq!(remaining, budget.remaining);
            assert_eq!(paid, budget.physical.as_ref().unwrap().charged());
        }
        (result.is_ok(), paid)
    };
    for count in [0, 1, 2, 7] {
        let (ok, paid) = observe(usize::MAX, count);
        assert!(ok);
        assert_eq!(observe(paid, count), (true, paid));
        assert!(!observe(paid - 1, count).0);
    }
    let mut budget = ReplayBudget::unobserved();
    let (_, metrics) = measure(|| {
        let mut session = acquire(&arena, function, &mut budget).unwrap();
        for _ in 0..10 {
            session.fact(&function.body.id).unwrap();
        }
    });
    assert!(!metrics.invalid);
    assert_eq!(
        (metrics.allocated, metrics.reallocations, metrics.live),
        (0, 0, 0)
    );
}

#[test]
fn unwind_releases_the_exclusive_budget_loan_without_any_scratch_backing() {
    let program = program();
    let function = &program.functions[0];
    let arena = arena(function, &[function.body.id.clone()]);
    let mut budget = ReplayBudget::unobserved();
    let (_, metrics) = measure(|| {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut session = acquire(&arena, function, &mut budget).unwrap();
            session.fact(&function.body.id).unwrap();
            std::panic::resume_unwind(Box::new(()));
        }));
        assert!(result.is_err());
        drop(result);
        budget
            .reserve_bytes(function, 0, "after unwound owner")
            .unwrap();
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
}

#[test]
fn zero_budget_and_reserved_floor_refuse_without_later_retry_authority() {
    let program = program();
    let function = &program.functions[0];
    let arena = arena(function, &[function.body.id.clone()]);
    assert_eq!(
        ReplayBudget::with_physical(function, 0).err().unwrap().code,
        "SPX-H006"
    );
    crate::bounded_output::with_limit_usage(1 << 20, || {
        let mut budget = ReplayBudget::new();
        budget.prepare_builder(function).unwrap();
        let remaining = crate::bounded_output::active_remaining().unwrap();
        assert!(crate::bounded_output::set_active_floor(remaining));
        let error = acquire(&arena, function, &mut budget)
            .err()
            .expect("reserved floor refuses owner acquisition");
        assert_eq!(error.code, "SPX-H006");
        crate::bounded_output::clear_active_floor();
        assert!(budget
            .reserve_bytes(function, 0, "retry after floor restoration")
            .is_err());
    });
}

// Fixed, finite OPT885 source composition BEFORE each unchanged historical
// authority. Every inverse is unique and the complete restored predecessor is
// authenticated. There is no substring-only acceptance or historical fallback.
pub(in crate::cleanup_plan::replay::expression_facts) fn before_885_source(
    source: &str,
    current: &str,
) -> String {
    use sha2::{Digest, Sha256};
    fn undo(current: &mut String, new: &str, old: &str) {
        assert!(!new.is_empty());
        assert_eq!(
            current.matches(new).count(),
            1,
            "unique complete OPT885 preimage"
        );
        *current = current.replacen(new, old, 1);
    }
    let acquired = crate::source_reflows_merge::main_e177_final_inputs::combined_side_value(
        source, "whole885", current,
    );
    let current = acquired.as_str();
    let mut restored = current.to_owned();
    let expected = match source {
        "src/cleanup_plan/replay/expression_facts/arena.rs" => {
            undo(
                &mut restored,
                r######"mod prefix;
pub(super) mod retained;"######,
                r######"mod prefix;"######,
            );
            "2eae0ecc943b9114099a7e87c7b2acf565c0f6454cb02f396ee30e981a109488"
        }
        "src/cleanup_plan/replay/expression_facts/receiving/directory.rs" => {
            undo(
                &mut restored,
                r######"pub(super) use status_prefix::{HeldPrefix, Owner as StatusInsertionOwner};"######,
                r######"pub(super) use status_prefix::Owner as StatusInsertionOwner;"######,
            );
            "71bddf2e91cd7b6768523035ce49c111e41686dc5104adbcdb8dd7165675e464"
        }
        "tests/hir_module_boundaries.rs" => {
            undo(
                &mut restored,
                r######"            include_str!("../src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/arena/prefix/retained_before.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/arena/retained.rs"),
            include_str!("../src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs")),"######,
                r######"            include_str!("../src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs")),"######,
            );
            undo(
                &mut restored,
                r######"        include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs"),
        include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs"),
        include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_before.rs"),
        include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_caller_before.rs"),
        include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs"),"######,
                r######"        include_str!("../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs"),"######,
            );
            "d3eb0f843f53b525b4a0dbd388b79a865721b02968fa2419af678bb7670ae249"
        }
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => {
            undo(&mut restored, r######"audit_source_join!(include_str!("../../../../../src/cleanup_plan/replay/expression_facts/arena.rs"), include_str!("../../../../../src/cleanup_plan/replay/expression_facts/arena/prefix/retained_before.rs"), include_str!("../../../../../src/cleanup_plan/replay/expression_facts/arena/retained.rs"), include_str!("../../../../../src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs"))"######, r######"include_str!("../../../../../src/cleanup_plan/replay/expression_facts/arena.rs")"######);
            undo(&mut restored, r######"audit_source_join!(include_str!("../../../../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs"), include_str!("../../../../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs"), include_str!("../../../../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_before.rs"), include_str!("../../../../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_caller_before.rs"), include_str!("../../../../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs"))"######, r######"include_str!("../../../../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs")"######);
            "ac45caecfb6e4d8484f304092c9d6a1d4db23e254480949f1619b7cc9c7a9f0d"
        }
        "tests/workspace/semantic_graph/source_inventory.rs" => {
            undo(&mut restored, r######"include_str!("../../src/cleanup_plan/replay/expression_facts/arena.rs"),
include_str!("../../src/cleanup_plan/replay/expression_facts/arena/prefix/retained_before.rs"),
include_str!("../../src/cleanup_plan/replay/expression_facts/arena/retained.rs"),
include_str!("../../src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs"),"######, r######"include_str!("../../src/cleanup_plan/replay/expression_facts/arena.rs"),"######);
            undo(&mut restored, r######"include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs"),
include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs"),
include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_before.rs"),
include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_caller_before.rs"),
include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs"),"######, r######"include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs"),"######);
            "a33bdf50c97ddec9ce29ca3dfa1abec37c13afcadf6b707c4539b9af6f9f13aa"
        }
        _ => return restored,
    };
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        expected,
        "complete fixed pre-885 input for {source}"
    );
    restored
}

// Preserve the exact caller/path and existing CI/semantic-reflow authority.
// The only leaked bytes are bounded, fixed test fixtures, never production.
pub(in crate::cleanup_plan::replay::expression_facts) fn before_885_ci(
    caller: &'static str,
    included: &'static str,
    current: &'static str,
) -> &'static str {
    let source = included
        .strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .and_then(|path| path.strip_prefix('/'))
        .expect("fixed absolute repository source");
    let restored = before_885_source(source, current);
    let input = if restored == current {
        current
    } else {
        Box::leak(restored.into_boxed_str())
    };
    crate::source_reflows::source(caller, included, input)
}

#[test]
fn source_composition_restores_full_fixed_prior_and_refuses_drift_or_already_old_input() {
    let sources = [
        (
            "src/cleanup_plan/replay/expression_facts/arena.rs",
            include_str!("../../arena.rs"),
        ),
        (
            "src/cleanup_plan/replay/expression_facts/receiving/directory.rs",
            include_str!("../../receiving/directory.rs"),
        ),
        (
            "tests/hir_module_boundaries.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/hir_module_boundaries.rs"
            )),
        ),
        ("tests/workspace/semantic_graph/source_inventory.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs"))),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"))),
    ];
    for (source, current) in sources {
        let old = before_885_source(source, current);
        assert_ne!(old, current);
        for wrong in [
            format!("{current} "),
            current[..current.len() - 1].to_owned(),
            old,
        ] {
            assert!(
                std::panic::catch_unwind(|| before_885_source(source, &wrong)).is_err(),
                "complete named preimage must reject drift and prior bytes: {source}"
            );
        }
    }
    assert_eq!(
        before_885_source("unlisted", "exact passthrough"),
        "exact passthrough"
    );
}

#[test]
fn all_seven_retained_lookup_modules_have_exact_role_edges_and_complete_raw_joins() {
    let arena = include_str!("../../arena.rs");
    let prefix = include_str!("../prefix.rs");
    let session = include_str!("../retained.rs");
    let certified = include_str!("../../receiving/retained/case_certified.rs");
    let edges = [
        ("src/cleanup_plan/replay/expression_facts/arena/retained.rs", "production", arena,
            "pub(super) mod retained;\n"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs", "production", certified,
            "mod retained_lookup;\n"),
        ("src/cleanup_plan/replay/expression_facts/arena/prefix/retained_before.rs", "test-only", prefix,
            "#[cfg(test)]\n#[path = \"prefix/retained_before.rs\"]\n#[allow(dead_code, reason = \"complete independently compiled immediate-old lookup\")]\npub(super) mod retained_before;\n"),
        ("src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs", "test-only", session,
            "#[cfg(test)]\npub(in crate::cleanup_plan::replay::expression_facts) mod tests;\n"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_before.rs", "test-only", certified,
            "#[cfg(test)]\npub(in crate::cleanup_plan::replay) mod lookup_before;\n"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_caller_before.rs", "test-only", certified,
            "#[cfg(test)]\nmod lookup_caller_before;\n"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs", "test-only", certified,
            "#[cfg(test)]\nmod lookup_tests;\n"),
    ];
    assert_eq!(
        edges
            .iter()
            .filter(|(_, role, _, _)| *role == "production")
            .count(),
        2
    );
    assert_eq!(
        edges
            .iter()
            .filter(|(_, role, _, _)| *role == "test-only")
            .count(),
        5
    );
    let hir = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/hir_module_boundaries.rs"
    ));
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/workspace/semantic_graph/source_inventory.rs"
    ));
    let native = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"));
    assert_eq!(
        native.lines().count(),
        1500,
        "finite joins preserve the standing native file budget"
    );
    for (path, role, parent, registration) in edges {
        assert_eq!(
            parent.matches(registration).count(),
            1,
            "exact owning module registration: {path}"
        );
        assert_eq!(registration.contains("#[cfg(test)]"), role == "test-only");
        if role == "production" {
            let at = parent.find(registration).unwrap();
            assert_ne!(parent[..at].trim_end().lines().last(), Some("#[cfg(test)]"));
        }
        assert_eq!(
            hir.matches(&format!("include_str!(\"../{path}\")")).count(),
            1,
            "one complete new source in the existing HIR raw owner: {path}"
        );
        assert_eq!(
            graph
                .matches(&format!("include_str!(\"../../{path}\")"))
                .count(),
            1,
            "one complete new source in the existing graph raw owner: {path}"
        );
        assert_eq!(
            native
                .matches(&format!("include_str!(\"../../../../../{path}\")"))
                .count(),
            1,
            "one complete new source in the existing native raw owner: {path}"
        );
    }
}
